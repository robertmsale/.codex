from __future__ import annotations

from datetime import datetime, timezone
import json
import os
import re
import subprocess
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

from fastmcp import FastMCP

mcp = FastMCP("safe-delete-mcp")

SANDBOX_FAILURE_PATTERNS = [
    re.compile(r"forbidden-sandbox-reinit", re.IGNORECASE),
    re.compile(r"operation not permitted", re.IGNORECASE),
    re.compile(r"killed by sandbox", re.IGNORECASE),
    re.compile(r"sandbox(?:-exec)?[: ].*denied", re.IGNORECASE),
]
ROBDEX_STATE_FILE = Path(
    os.getenv(
        "SAFE_DELETE_ROBDEX_STATE_FILE",
        str(Path.home() / ".codex" / "robdex.json"),
    )
)
CODEX_CONFIG_FILE = Path(
    os.getenv(
        "SAFE_DELETE_CODEX_CONFIG_FILE",
        str(Path.home() / ".codex" / "config.toml"),
    )
)
SAFE_DELETE_STAGING_DIR = Path("/tmp/safe-delete")


class SafeDeleteError(RuntimeError):
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


@dataclass
class ConfigFallbackState:
    mode: Literal["read-only", "workspace-write", "danger-full-access"] | None
    network_access: bool | None


# Mirrors command-parser defaults so both tools resolve sandbox policy identically.
CURRENT_SANDBOX_STATE = SandboxState(
    mode=os.getenv("ROBDEX_SANDBOX_MODE", "danger-full-access"),
    network_access=(os.getenv("ROBDEX_NETWORK_ACCESS", "false").strip().lower() == "true"),
    cwd=os.getenv("ROBDEX_SANDBOX_CWD", os.getcwd()),
    thread_id=os.getenv("CODEX_THREAD_ID"),
)


def _normalize_mode(value: str) -> Literal["read-only", "workspace-write", "danger-full-access"]:
    normalized = value.strip().lower()
    if normalized not in {"read-only", "workspace-write", "danger-full-access"}:
        raise SafeDeleteError(f"Unsupported sandbox mode: {value}")
    return normalized  # type: ignore[return-value]


def _normalize_robdex_mode(
    value: Any,
) -> Literal["read-only", "workspace-write", "danger-full-access"] | None:
    if not isinstance(value, str):
        return None
    normalized = value.strip().lower()
    if normalized in {"read-only", "workspace-write", "danger-full-access"}:
        return normalized
    if normalized == "external-sandbox":
        return "read-only"
    return None


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


def _resolve_thread_id() -> str | None:
    env_value = os.getenv("CODEX_THREAD_ID", "").strip()
    return env_value or None


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


def _resolve_sandbox_state(cwd: str | None) -> SandboxState:
    resolved_thread_id = _resolve_thread_id()
    robdex_metadata = _load_robdex_thread_metadata(resolved_thread_id)
    config_fallback = _load_config_fallback_state()

    metadata_mode = _normalize_robdex_mode(robdex_metadata.get("sandboxMode"))
    config_mode = config_fallback.mode
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

    return SandboxState(
        mode=effective_mode,
        network_access=bool(effective_network),
        cwd=command_cwd,
        thread_id=resolved_thread_id,
    )


def _build_execution_argv(command: list[str], state: SandboxState) -> list[str]:
    if not command:
        raise SafeDeleteError("command cannot be empty")
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


def _normalize_paths(paths: list[str], cwd: Path) -> list[Path]:
    normalized: list[Path] = []
    for raw in paths:
        candidate = (raw or "").strip()
        if not candidate:
            continue

        path = Path(candidate).expanduser()
        if not path.is_absolute():
            path = cwd / path

        normalized.append(path.resolve(strict=False))

    if not normalized:
        raise SafeDeleteError("Provide at least one non-empty path.")

    return normalized


def _staging_dir() -> Path:
    return SAFE_DELETE_STAGING_DIR.expanduser().resolve()


def _timestamp_token() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def _safe_name(path: Path) -> str:
    name = path.name.strip()
    return name or "item"


def _unique_destination(root: Path, src: Path, token: str) -> Path:
    base = f"{_safe_name(src)}-{token}"
    candidate = root / base
    index = 1
    while candidate.exists():
        candidate = root / f"{base}-{index}"
        index += 1
    return candidate


def _run_wrapped(command: list[str], state: SandboxState) -> ExecutionOutcome:
    argv = _build_execution_argv(command, state)
    return _run_command(argv=argv, cwd=state.cwd)


def _raise_on_failure(outcome: ExecutionOutcome, state: SandboxState, context: str) -> None:
    if outcome.exit_code == 0:
        return
    if _is_sandbox_failure(outcome, state):
        detail = _extract_sandbox_failure_text(outcome) or "Sandbox blocked safe-delete operation."
        raise SafeDeleteError(f"sandbox blocked delete: {detail}")
    detail = (outcome.stderr or outcome.stdout or "unknown error").strip()
    raise SafeDeleteError(f"{context}: {detail}")


@mcp.tool(output_schema=None)
def safe_delete(
    paths: list[str],
    cwd: str | None = None,
) -> str:
    """Move one or more files/directories into /tmp safe-delete staging."""
    base_cwd = Path(cwd or os.getcwd()).expanduser().resolve()
    targets = _normalize_paths(paths, base_cwd)
    state = _resolve_sandbox_state(str(base_cwd))

    staging = _staging_dir()
    mkdir_outcome = _run_wrapped(["/bin/mkdir", "-p", str(staging)], state)
    _raise_on_failure(mkdir_outcome, state, f"Failed to prepare safe-delete staging directory '{staging}'")

    token = _timestamp_token()
    moved: list[tuple[Path, Path]] = []
    for source in targets:
        destination = _unique_destination(staging, source, token)
        move_outcome = _run_wrapped(["/bin/mv", str(source), str(destination)], state)
        _raise_on_failure(move_outcome, state, f"Failed to stage delete '{source}'")
        moved.append((source, destination))

    if len(moved) == 1:
        src, dst = moved[0]
        return f"Staged delete: {src} -> {dst}"

    lines = [f"Staged {len(moved)} paths under {staging}:"]
    for src, dst in moved:
        lines.append(f"- {src} -> {dst}")
    return "\n".join(lines)


def main() -> None:
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
