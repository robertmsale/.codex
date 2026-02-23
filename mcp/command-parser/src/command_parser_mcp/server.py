from __future__ import annotations

import json
import os
import re
import shlex
import subprocess
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

from fastmcp import FastMCP

SANDBOX_FAILURE_PATTERNS = [
    re.compile(r"forbidden-sandbox-reinit", re.IGNORECASE),
    re.compile(r"operation not permitted", re.IGNORECASE),
    re.compile(r"killed by sandbox", re.IGNORECASE),
    re.compile(r"sandbox(?:-exec)?[: ].*denied", re.IGNORECASE),
]

DEFAULT_PROFILE = os.getenv("COMMAND_PARSER_PROFILE", "command-parser")
ROBDEX_STATE_FILE = Path(
    os.getenv(
        "COMMAND_PARSER_ROBDEX_STATE_FILE",
        str(Path.home() / ".codex" / "robdex.json"),
    )
)
CODEX_CONFIG_FILE = Path(
    os.getenv(
        "COMMAND_PARSER_CODEX_CONFIG_FILE",
        str(Path.home() / ".codex" / "config.toml"),
    )
)

mcp = FastMCP("command-parser-mcp")


class CommandParserError(RuntimeError):
    pass


@dataclass
class ExecutionOutcome:
    argv: list[str]
    cwd: str
    exit_code: int
    stdout: str
    stderr: str

    @property
    def combined_output(self) -> str:
        return f"{self.stdout}{self.stderr}"


@dataclass
class SandboxState:
    mode: Literal["read-only", "workspace-write", "danger-full-access"]
    network_access: bool
    cwd: str
    thread_id: str | None
    source: str


# This state can be updated later if Codex python SDK gains direct support for
# codex/sandbox-state/update custom request handling.
CURRENT_SANDBOX_STATE = SandboxState(
    mode=os.getenv("ROBDEX_SANDBOX_MODE", "danger-full-access"),
    network_access=(os.getenv("ROBDEX_NETWORK_ACCESS", "false").strip().lower() == "true"),
    cwd=os.getenv("ROBDEX_SANDBOX_CWD", os.getcwd()),
    thread_id=os.getenv("CODEX_THREAD_ID"),
    source="env-defaults",
)


@dataclass
class ConfigFallbackState:
    mode: Literal["read-only", "workspace-write", "danger-full-access"] | None
    network_access: bool | None


def _load_config_fallback_state() -> ConfigFallbackState:
    if not CODEX_CONFIG_FILE.exists():
        return ConfigFallbackState(mode=None, network_access=None)
    try:
        payload = tomllib.loads(CODEX_CONFIG_FILE.read_text(encoding="utf-8", errors="replace"))
    except Exception:  # noqa: BLE001
        return ConfigFallbackState(mode=None, network_access=None)
    if not isinstance(payload, dict):
        return ConfigFallbackState(mode=None, network_access=None)

    mode = _normalize_robdex_mode(payload.get("sandbox_mode"))
    raw_network = payload.get("network_access")
    network_access = raw_network if isinstance(raw_network, bool) else None
    return ConfigFallbackState(mode=mode, network_access=network_access)


def _normalize_mode(value: str) -> Literal["read-only", "workspace-write", "danger-full-access"]:
    normalized = value.strip().lower()
    if normalized not in {"read-only", "workspace-write", "danger-full-access"}:
        raise CommandParserError(f"Unsupported sandbox mode: {value}")
    return normalized  # type: ignore[return-value]


def _resolve_sandbox_state(
    cwd: str | None,
    sandbox_mode: Literal["read-only", "workspace-write", "danger-full-access"] | None,
    network_access: bool | None,
    thread_id: str | None,
) -> SandboxState:
    resolved_thread_id = _resolve_thread_id(thread_id)
    robdex_metadata = _load_robdex_thread_metadata(resolved_thread_id)
    config_fallback = _load_config_fallback_state()

    effective_mode: Literal["read-only", "workspace-write", "danger-full-access"] | None = None
    if sandbox_mode is not None:
        effective_mode = _normalize_mode(sandbox_mode)
    else:
        effective_mode = _normalize_robdex_mode(robdex_metadata.get("sandboxMode"))
        if effective_mode is None:
            effective_mode = config_fallback.mode
        if effective_mode is None:
            effective_mode = _normalize_mode(CURRENT_SANDBOX_STATE.mode)

    if network_access is not None:
        effective_network = bool(network_access)
    else:
        metadata_network = robdex_metadata.get("networkAccess")
        if isinstance(metadata_network, bool):
            effective_network = metadata_network
        else:
            effective_network = config_fallback.network_access
            if effective_network is None:
                effective_network = CURRENT_SANDBOX_STATE.network_access

    command_cwd = cwd or CURRENT_SANDBOX_STATE.cwd or os.getcwd()
    command_cwd = str(Path(command_cwd).expanduser().resolve())

    if sandbox_mode is not None or network_access is not None:
        source = "tool-override"
    elif robdex_metadata:
        source = "robdex-state"
    elif config_fallback.mode is not None or config_fallback.network_access is not None:
        source = "codex-config"
    else:
        source = "env-defaults"

    return SandboxState(
        mode=effective_mode,
        network_access=bool(effective_network),
        cwd=command_cwd,
        thread_id=resolved_thread_id,
        source=source,
    )


def _resolve_thread_id(explicit_thread_id: str | None) -> str | None:
    if explicit_thread_id is not None:
        trimmed = explicit_thread_id.strip()
        return trimmed or None
    env_value = os.getenv("CODEX_THREAD_ID", "").strip()
    return env_value or None


def _normalize_robdex_mode(
    value: Any,
) -> Literal["read-only", "workspace-write", "danger-full-access"] | None:
    if not isinstance(value, str):
        return None
    normalized = value.strip().lower()
    if normalized in {"read-only", "workspace-write", "danger-full-access"}:
        return normalized
    # external sandbox cannot be represented here, so fail closed.
    if normalized == "external-sandbox":
        return "read-only"
    return None


def _load_robdex_thread_metadata(thread_id: str | None) -> dict[str, Any]:
    if not thread_id:
        return {}
    try:
        payload = json.loads(ROBDEX_STATE_FILE.read_text(encoding="utf-8", errors="replace"))
    except Exception:  # noqa: BLE001
        return {}
    if not isinstance(payload, dict):
        return {}
    metadata = payload.get("threadMetadataByID", {})
    if not isinstance(metadata, dict):
        return {}
    entry = metadata.get(thread_id, {})
    if isinstance(entry, dict):
        return entry
    return {}


def _build_execution_argv(command: list[str], state: SandboxState) -> list[str]:
    if not command:
        raise CommandParserError("command cannot be empty")

    if state.mode == "danger-full-access":
        return command

    argv: list[str] = ["codex", "sandbox", "macos"]
    if state.mode == "workspace-write":
        argv.extend(["--full-auto", "-c", f"sandbox_workspace_write.network_access={'true' if state.network_access else 'false'}"])
    argv.extend(["--log-denials", *command])
    return argv


def _run_command(argv: list[str], cwd: str) -> ExecutionOutcome:
    proc = subprocess.run(
        argv,
        cwd=cwd,
        text=True,
        capture_output=True,
    )
    return ExecutionOutcome(
        argv=argv,
        cwd=cwd,
        exit_code=proc.returncode,
        stdout=proc.stdout,
        stderr=proc.stderr,
    )


def _is_sandbox_failure(outcome: ExecutionOutcome, state: SandboxState) -> bool:
    if state.mode == "danger-full-access":
        return False

    haystack = (outcome.stderr or "") + "\n" + (outcome.stdout or "")
    return any(pattern.search(haystack) for pattern in SANDBOX_FAILURE_PATTERNS)


def _assert_profile_exists(profile: str) -> None:
    config_file = CODEX_CONFIG_FILE
    if not config_file.exists():
        raise CommandParserError(f"Missing config file: {config_file}")
    marker = f"[profiles.{profile}]"
    if marker not in config_file.read_text(encoding="utf-8", errors="replace"):
        raise CommandParserError(f"Profile '{profile}' not found in {config_file}")


def _load_config_payload() -> dict[str, Any]:
    if not CODEX_CONFIG_FILE.exists():
        return {}
    try:
        payload = tomllib.loads(CODEX_CONFIG_FILE.read_text(encoding="utf-8", errors="replace"))
    except Exception:  # noqa: BLE001
        return {}
    if isinstance(payload, dict):
        return payload
    return {}


def _load_mcp_server_names() -> list[str]:
    payload = _load_config_payload()
    servers = payload.get("mcp_servers")
    if not isinstance(servers, dict):
        return []
    names = [name for name in servers.keys() if isinstance(name, str) and name]
    names.sort()
    return names


def _mcp_disable_override(name: str) -> str:
    escaped = name.replace("\\", "\\\\").replace('"', '\\"')
    return f'mcp_servers."{escaped}".enabled=false'


def _parse_output_with_codex(
    outcome: ExecutionOutcome,
    include_warnings: bool,
    additional_request: str | None,
    profile: str,
) -> str:
    _assert_profile_exists(profile)

    with tempfile.TemporaryDirectory(prefix="command-parser-mcp.") as temp_dir:
        temp_path = Path(temp_dir)
        output_log = temp_path / "output.log"
        response_log = temp_path / "response.log"
        command_txt = temp_path / "command.txt"
        agents_md = temp_path / "AGENTS.md"

        output_log.write_text(outcome.combined_output, encoding="utf-8", errors="replace")
        command_txt.write_text(" ".join(shlex.quote(arg) for arg in outcome.argv) + "\n", encoding="utf-8")
        agents_md.write_text(
            """You are command-parser, a CLI output extraction agent.

Task:
- Read ./output.log and extract errors (and warnings only if requested).
- Prefer targeted search (`rg`, `grep`) before broad reads for huge files.

Output rules:
- If there are no errors at all:
  - and no additional request: output exactly: No errors!
  - and additional request exists: output `No errors!` first, then `## Requested Information`
- Otherwise output:
  - ## Errors
  - one bullet per distinct error as: - <brief message> — <file:line(:col) when present>
- Special case — unit test failures:
  - Include failing test names and concise assertion/panic/trace lines that explain why a test failed.
  - Include expected vs actual snippets when present.
  - Do not include passing tests or non-error test noise.
- If warnings are requested and present, add:
  - ## Warnings
  - one bullet per distinct warning as: - <brief message> — <file:line(:col) when present>
- Additional request (optional):
  - Only if an additional request is provided, append:
    - ## Requested Information
    - concise bullets answering only that request, anchored to log lines/files when present
  - If requested information is not present, output: - Not found in output.
- Preserve file paths and coordinates exactly as shown.
- Do not include advice, fixes, commands, or extra headings.
""",
            encoding="utf-8",
        )

        prompt = (
            f"Parse ./output.log from this command:\n{command_txt.read_text(encoding='utf-8')}\n"
            f"Include warnings: {'yes' if include_warnings else 'no'}\n\n"
            f"Additional request: {additional_request or '<none>'}\n\n"
            "Return only the structured extraction format from AGENTS.md."
        )

        cmd = [
            "codex",
            "exec",
            "--skip-git-repo-check",
            "--ephemeral",
            "-s",
            "read-only",
            "-C",
            temp_dir,
            "-p",
            profile,
            "-c",
            "web_search=\"disabled\"",
            # "-c",
            # "tools.web_search=false",
            # "-c",
            # "tools.view_image=false",
            # "-c",
            # "features.shell_tool=false",
            "-c",
            "features.unified_exec=false",
            # "-c",
            # "features.shell_zsh_fork=false",
            # "-c",
            # "features.shell_snapshot=false",
            # "-c",
            # "features.js_repl=false",
            # "-c",
            # "features.js_repl_tools_only=false",
            # "-c",
            # "features.web_search_request=false",
            # "-c",
            # "features.web_search_cached=false",
            # "-c",
            # "features.search_tool=false",
            # "-c",
            # "features.codex_git_commit=false",
            # "-c",
            # "features.runtime_metrics=false",
            # "-c",
            # "features.sqlite=false",
            # "-c",
            # "features.memory_tool=false",
            # "-c",
            # "features.child_agents_md=false",
            # "-c",
            # "features.apply_patch_freeform=false",
            # "-c",
            # "features.use_linux_sandbox_bwrap=false",
            # "-c",
            # "features.request_rule=false",
            # "-c",
            # "features.experimental_windows_sandbox=false",
            # "-c",
            # "features.elevated_windows_sandbox=false",
            # "-c",
            # "features.remote_models=false",
            # "-c",
            # "features.powershell_utf8=false",
            # "-c",
            # "features.enable_request_compression=false",
            "-c",
            "features.multi_agent=false",
            # "-c",
            # "features.apps=false",
            # "-c",
            # "features.apps_mcp_gateway=false",
            # "-c",
            # "features.skill_mcp_dependency_install=false",
            # "-c",
            # "features.skill_env_var_dependency_prompt=false",
            "-c",
            "features.steer=false",
            # "-c",
            # "features.collaboration_modes=false",
            # "-c",
            # "features.personality=false",
            # "-c",
            # "features.prevent_idle_sleep=false",
            # "-c",
            # "features.responses_websockets=false",
            # "-c",
            # "features.responses_websockets_v2=false",
            # "-c",
            # "features.undo=false",
            "-c",
            "features.skills=false",
            # "-c",
            # "skills=false",
        ]
        # for server_name in _load_mcp_server_names():
        #     cmd.extend(["-c", _mcp_disable_override(server_name)])
        cmd.extend(
            [
                "-o",
                str(response_log),
                prompt,
            ]
        )

        result = subprocess.run(cmd, text=True, capture_output=True)
        if result.returncode != 0:
            raise CommandParserError(
                "codex exec parser failed: " + (result.stderr.strip() or result.stdout.strip() or f"exit {result.returncode}")
            )

        if not response_log.exists() or not response_log.read_text(encoding="utf-8", errors="replace").strip():
            raise CommandParserError("Missing parser response")

        return response_log.read_text(encoding="utf-8", errors="replace").strip()


@mcp.tool
def command_parser_run(
    command: list[str],
    cwd: str | None = None,
    include_warnings: bool = False,
    additional_request: str | None = None,
    sandbox_mode: Literal["read-only", "workspace-write", "danger-full-access"] | None = None,
    network_access: bool | None = None,
    thread_id: str | None = None,
    profile: str | None = None,
) -> str:
    """Run command once with sandbox routing, skip parser on sandbox failures, parse output otherwise."""
    state = _resolve_sandbox_state(cwd, sandbox_mode, network_access, thread_id)
    argv = _build_execution_argv(command, state)
    outcome = _run_command(argv=argv, cwd=state.cwd)

    if _is_sandbox_failure(outcome, state):
        text = (outcome.stderr or outcome.stdout).strip()
        return text or "Sandbox blocked command execution."

    parser_profile = (profile or DEFAULT_PROFILE).strip() or DEFAULT_PROFILE
    return _parse_output_with_codex(
        outcome=outcome,
        include_warnings=include_warnings,
        additional_request=additional_request,
        profile=parser_profile,
    )


def main() -> None:
    # NOTE: FastMCP/mcp Python SDK currently does not expose Codex custom request
    # handling for `codex/sandbox-state/update` with dynamic ClientRequest unions.
    # This server therefore uses env/default-based sandbox state plus optional
    # per-call overrides until SDK support lands.
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
