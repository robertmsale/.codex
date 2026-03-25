from __future__ import annotations

from datetime import datetime, timezone
import json
import os
import re
import shlex
import shutil
import subprocess
import tempfile
import time
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

from fastmcp import FastMCP
from fastmcp.tools.tool import ToolResult
from mcp.types import TextContent

SANDBOX_FAILURE_PATTERNS = [
    re.compile(r"forbidden-sandbox-reinit", re.IGNORECASE),
    re.compile(r"operation not permitted", re.IGNORECASE),
    re.compile(r"killed by sandbox", re.IGNORECASE),
    re.compile(r"sandbox(?:-exec)?[: ].*denied", re.IGNORECASE),
]

COMMAND_PARSER_DEFAULT_PROFILE = "command-parser"
COMMAND_PARSER_SKILL_ENV_FILE = Path(
    os.getenv(
        "COMMAND_PARSER_SKILL_ENV_FILE",
        str(Path.home() / ".codex" / "skills" / "command-parser" / ".env"),
    )
)
ROBDEX_STATE_FILE = Path(
    os.getenv(
        "COMMAND_PARSER_ROBDEX_STATE_FILE",
        str(Path.home() / ".codex" / "robdex" / "robdex.json"),
    )
)
CODEX_CONFIG_FILE = Path(
    os.getenv(
        "COMMAND_PARSER_CODEX_CONFIG_FILE",
        str(Path.home() / ".codex" / "config.toml"),
    )
)
COMMAND_PARSER_USAGE_LOG_FILE = Path(
    os.getenv(
        "COMMAND_PARSER_USAGE_LOG_FILE",
        str(Path.home() / ".codex" / "command-parser-usage.log"),
    )
)
COMMAND_PARSER_RULE_FILE = Path(
    os.getenv(
        "COMMAND_PARSER_RULE_FILE",
        str(COMMAND_PARSER_SKILL_ENV_FILE.parent / "command-parser.rule"),
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


def _parse_dotenv_value(raw: str) -> str:
    value = raw.strip()
    if not value:
        return ""
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
        return value[1:-1]
    return value


def _refresh_command_parser_environment() -> None:
    # Re-read command-parser skill env on every tool call so operator edits
    # (for example COMMAND_PARSER_PROFILE) apply immediately without restart.
    for key in [k for k in os.environ if k.startswith("COMMAND_PARSER_")]:
        os.environ.pop(key, None)

    if not COMMAND_PARSER_SKILL_ENV_FILE.exists():
        return

    for raw_line in COMMAND_PARSER_SKILL_ENV_FILE.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        env_key = key.strip()
        if not env_key.startswith("COMMAND_PARSER_"):
            continue
        os.environ[env_key] = _parse_dotenv_value(value)


def _default_profile() -> str:
    return os.getenv("COMMAND_PARSER_PROFILE", COMMAND_PARSER_DEFAULT_PROFILE).strip() or COMMAND_PARSER_DEFAULT_PROFILE


def _normalize_mode(value: str) -> Literal["read-only", "workspace-write", "danger-full-access"]:
    normalized = value.strip().lower()
    if normalized not in {"read-only", "workspace-write", "danger-full-access"}:
        raise CommandParserError(f"Unsupported sandbox mode: {value}")
    return normalized  # type: ignore[return-value]


def _resolve_sandbox_state(
    cwd: str | None,
) -> SandboxState:
    resolved_thread_id = _resolve_thread_id()
    robdex_metadata = _load_robdex_thread_metadata(resolved_thread_id)
    config_fallback = _load_config_fallback_state()

    metadata_mode = _normalize_robdex_mode(robdex_metadata.get("sandboxMode"))
    config_mode = config_fallback.mode
    # Global full-access mode must win over potentially stale per-thread
    # metadata so command-parser matches the active Codex runtime policy.
    if config_mode == "danger-full-access":
        effective_mode: Literal["read-only", "workspace-write", "danger-full-access"] = "danger-full-access"
    elif metadata_mode is not None:
        effective_mode = metadata_mode
    elif config_mode is not None:
        effective_mode = config_mode
    else:
        effective_mode = _normalize_mode(CURRENT_SANDBOX_STATE.mode)

    metadata_network = robdex_metadata.get("networkAccess")
    if isinstance(metadata_network, bool):
        effective_network = metadata_network
    else:
        effective_network = config_fallback.network_access
        if effective_network is None:
            effective_network = CURRENT_SANDBOX_STATE.network_access

    command_cwd = cwd or CURRENT_SANDBOX_STATE.cwd or os.getcwd()
    command_cwd = str(Path(command_cwd).expanduser().resolve())

    if robdex_metadata:
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


def _resolve_thread_id() -> str | None:
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
    env = os.environ.copy()
    env["IS_USING_COMMAND_PARSER"] = "true"
    proc = subprocess.run(
        argv,
        cwd=cwd,
        text=True,
        capture_output=True,
        env=env,
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


def _extract_sandbox_failure_text(outcome: ExecutionOutcome) -> str | None:
    for text in (outcome.stderr, outcome.stdout, outcome.combined_output):
        if not text:
            continue

        first_match_index: int | None = None
        for pattern in SANDBOX_FAILURE_PATTERNS:
            match = pattern.search(text)
            if not match:
                continue
            if first_match_index is None or match.start() < first_match_index:
                first_match_index = match.start()

        if first_match_index is None:
            continue

        line_start = text.rfind("\n", 0, first_match_index)
        if line_start == -1:
            line_start = 0
        else:
            line_start += 1
        trimmed = text[line_start:].strip()
        if trimmed:
            return trimmed

    fallback = (outcome.stderr or outcome.stdout or outcome.combined_output).strip()
    return fallback or None


def _plaintext_result(message: str) -> ToolResult:
    text = message.strip() or "No output."
    return ToolResult(content=[TextContent(type="text", text=text)])


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
    raw_command: list[str],
    outcome: ExecutionOutcome,
    include_warnings: bool,
    additional_request: str | None,
    profile: str,
) -> str:
    _assert_profile_exists(profile)

    with tempfile.TemporaryDirectory(prefix="command-parser-mcp.") as temp_dir:
        temp_path = Path(temp_dir)
        codex_home = _stage_parser_codex_home(temp_path)
        output_log = temp_path / "output.log"
        response_log = temp_path / "response.log"
        command_txt = temp_path / "command.txt"

        output_log.write_text(outcome.combined_output, encoding="utf-8", errors="replace")
        command_txt.write_text(" ".join(shlex.quote(arg) for arg in raw_command) + "\n", encoding="utf-8")

        prompt = (
            f"Parse ./output.log from this raw command:\n{command_txt.read_text(encoding='utf-8')}\n"
            f"Include warnings: {'yes' if include_warnings else 'no'}\n\n"
            f"Additional request: {additional_request or '<none>'}\n\n"
            "Read the provided files and return only the extraction result."
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
            "-c",
            "mcp_servers.commandParser.enabled=false",
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

        parser_env = os.environ.copy()
        parser_env["CODEX_HOME"] = str(codex_home)
        parser_env["HOME"] = str(temp_path / "home")

        result = subprocess.run(cmd, text=True, capture_output=True, env=parser_env)
        if result.returncode != 0:
            raise CommandParserError(
                "codex exec parser failed: " + (result.stderr.strip() or result.stdout.strip() or f"exit {result.returncode}")
            )

        if not response_log.exists() or not response_log.read_text(encoding="utf-8", errors="replace").strip():
            raise CommandParserError("Missing parser response")

        return response_log.read_text(encoding="utf-8", errors="replace").strip()


def _stage_parser_codex_home(temp_path: Path) -> Path:
    source_codex_home = CODEX_CONFIG_FILE.expanduser().resolve().parent
    source_role_file = source_codex_home / "roles" / "command-parser.md"
    source_config_file = CODEX_CONFIG_FILE.expanduser().resolve()
    if not source_config_file.exists():
        raise CommandParserError(f"Missing Codex config for parser profile: {source_config_file}")
    if not source_role_file.exists():
        raise CommandParserError(f"Missing command-parser role instructions: {source_role_file}")

    staged_codex_home = temp_path / "codex-home"
    staged_role_file = staged_codex_home / "roles" / "command-parser.md"
    staged_role_file.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source_config_file, staged_codex_home / "config.toml")
    shutil.copy2(source_role_file, staged_role_file)
    return staged_codex_home


def _raw_command_text(command: list[str]) -> str:
    return " ".join(shlex.quote(arg) for arg in command)


def _resolved_usage_log_file() -> Path:
    raw = os.getenv("COMMAND_PARSER_USAGE_LOG_FILE", "").strip()
    if raw:
        return Path(raw).expanduser()
    return COMMAND_PARSER_USAGE_LOG_FILE


def _resolved_rule_file() -> Path:
    raw = os.getenv("COMMAND_PARSER_RULE_FILE", "").strip()
    if raw:
        return Path(raw).expanduser()
    return COMMAND_PARSER_RULE_FILE


def _append_usage_log(command: list[str], cwd: str) -> None:
    log_file = _resolved_usage_log_file()
    timestamp = datetime.now(timezone.utc).isoformat(timespec="seconds")
    line = f"{timestamp} | {_raw_command_text(command)} | cwd={cwd}\n"
    try:
        log_file.parent.mkdir(parents=True, exist_ok=True)
        with log_file.open("a", encoding="utf-8") as handle:
            handle.write(line)
    except OSError as exc:
        reason = exc.strerror or str(exc)
        raise CommandParserError(
            f"Failed to append command usage log at {log_file}: {reason}"
        ) from exc


def _delay_seconds() -> float:
    raw = os.getenv("COMMAND_PARSER_DELAY", "0").strip()
    if not raw:
        return 0.0
    try:
        value = float(raw)
    except ValueError as exc:
        raise CommandParserError(f"Invalid COMMAND_PARSER_DELAY='{raw}'. Use a numeric seconds value.") from exc
    if value < 0:
        raise CommandParserError(f"Invalid COMMAND_PARSER_DELAY='{raw}'. Value must be >= 0.")
    return value


def _execpolicy_forbidden_message(payload: dict[str, Any]) -> str | None:
    decision = str(payload.get("decision", "")).strip().lower()
    if decision == "forbidden":
        matched_rules = payload.get("matchedRules")
        if isinstance(matched_rules, list):
            for entry in matched_rules:
                if not isinstance(entry, dict):
                    continue
                prefix_match = entry.get("prefixRuleMatch")
                if not isinstance(prefix_match, dict):
                    continue
                justification = str(prefix_match.get("justification", "")).strip()
                if justification:
                    return justification
        return "Command is forbidden by command-parser.rule."

    matched_rules = payload.get("matchedRules")
    if not isinstance(matched_rules, list):
        return None

    for entry in matched_rules:
        if not isinstance(entry, dict):
            continue
        prefix_match = entry.get("prefixRuleMatch")
        if not isinstance(prefix_match, dict):
            continue
        rule_decision = str(prefix_match.get("decision", "")).strip().lower()
        if rule_decision != "forbidden":
            continue
        justification = str(prefix_match.get("justification", "")).strip()
        return justification or "Command is forbidden by command-parser.rule."
    return None


def _check_command_policy(command: list[str], cwd: str) -> str | None:
    rule_file = _resolved_rule_file()
    if not rule_file.exists():
        return None

    check_cmd = [
        "codex",
        "execpolicy",
        "check",
        "--rules",
        str(rule_file),
        "--",
        *command,
    ]
    result = subprocess.run(check_cmd, cwd=cwd, text=True, capture_output=True)
    if result.returncode != 0:
        detail = (result.stderr or result.stdout or f"exit {result.returncode}").strip()
        raise CommandParserError(
            f"execpolicy check failed for command-parser.rule at {rule_file}: {detail}"
        )

    stdout = (result.stdout or "").strip()
    if not stdout:
        return None

    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as exc:
        raise CommandParserError(
            f"execpolicy output was not valid JSON: {stdout}"
        ) from exc

    if not isinstance(payload, dict):
        return None

    return _execpolicy_forbidden_message(payload)


@mcp.tool
def command_parser_run(
    command: list[str],
    cwd: str | None = None,
    include_warnings: bool = False,
    additional_request: str | None = None,
) -> ToolResult:
    """Run command once with sandbox routing, skip parser on sandbox failures, parse output otherwise."""
    _refresh_command_parser_environment()
    state = _resolve_sandbox_state(cwd)
    _append_usage_log(command=command, cwd=state.cwd)

    policy_message = _check_command_policy(command=command, cwd=state.cwd)
    if policy_message:
        return _plaintext_result(f"Command blocked by command-parser.rule: {policy_message}")

    delay = _delay_seconds()
    if delay > 0:
        time.sleep(delay)

    argv = _build_execution_argv(command, state)
    outcome = _run_command(argv=argv, cwd=state.cwd)

    if _is_sandbox_failure(outcome, state):
        text = _extract_sandbox_failure_text(outcome)
        return _plaintext_result(text or "Sandbox blocked command execution.")

    parser_profile = _default_profile()
    parsed = _parse_output_with_codex(
        raw_command=command,
        outcome=outcome,
        include_warnings=include_warnings,
        additional_request=additional_request,
        profile=parser_profile,
    )
    return _plaintext_result(parsed)


def main() -> None:
    # NOTE: FastMCP/mcp Python SDK currently does not expose Codex custom request
    # handling for `codex/sandbox-state/update` with dynamic ClientRequest unions.
    # This server therefore uses env/config/robdex-derived sandbox state.
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
