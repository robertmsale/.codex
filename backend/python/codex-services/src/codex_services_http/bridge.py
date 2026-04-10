from __future__ import annotations

import os
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml


class BridgeError(RuntimeError):
    """Raised when a request cannot be handled safely."""


@dataclass(frozen=True)
class BridgePaths:
    host_home: Path
    virtual_home: Path
    allowed_roots: tuple[Path, ...]


def _codex_home() -> Path:
    raw = os.environ.get("CODEX_HOME")
    if raw:
        return Path(raw).expanduser().resolve()
    return (Path.home() / ".codex").resolve()


def _mutagen_file() -> Path:
    raw = os.environ.get("CODEX_MUTAGEN_FILE")
    if raw:
        return Path(raw).expanduser().resolve()
    return (Path.cwd() / "mutagen.yml").resolve()


def _host_home() -> Path:
    raw = os.environ.get("CODEX_HOST_HOME")
    if raw:
        return Path(raw).expanduser().resolve()
    return Path.home().resolve()


def _virtual_home(host_home: Path) -> Path:
    raw = os.environ.get("CODEX_VIRTUAL_HOME")
    if raw:
        return Path(raw).expanduser()
    return Path("/home") / host_home.name


def _load_allowed_roots(mutagen_file: Path) -> tuple[Path, ...]:
    if not mutagen_file.exists():
        raise BridgeError(f"Mutagen file not found: {mutagen_file}")

    payload = yaml.safe_load(mutagen_file.read_text(encoding="utf-8")) or {}
    sync = payload.get("sync") or {}
    roots: set[Path] = set()
    for name, config in sync.items():
        if not isinstance(config, dict):
            continue
        for endpoint_name in ("alpha", "beta"):
            endpoint = config.get(endpoint_name)
            if not isinstance(endpoint, str):
                continue
            if _looks_like_remote_mutagen_endpoint(endpoint):
                continue
            endpoint_path = Path(endpoint).expanduser().resolve()
            roots.add(endpoint_path)
    return tuple(sorted(roots, key=lambda p: (len(str(p)), str(p))))


def _looks_like_remote_mutagen_endpoint(value: str) -> bool:
    if not value or value.startswith("/"):
        return False
    if value.startswith("~/") or value.startswith("./") or value.startswith("../"):
        return False
    if ":" not in value:
        return False
    prefix, _suffix = value.split(":", 1)
    return "@" in prefix or prefix not in {"", ".", ".."}


def load_paths() -> BridgePaths:
    host_home = _host_home()
    virtual_home = _virtual_home(host_home)
    allowed_roots = _load_allowed_roots(_mutagen_file())
    return BridgePaths(host_home=host_home, virtual_home=virtual_home, allowed_roots=allowed_roots)


def _translate_virtual_to_host(path: Path, paths: BridgePaths) -> Path:
    if path == paths.virtual_home:
        return paths.host_home
    try:
        relative = path.relative_to(paths.virtual_home)
    except ValueError:
        return path
    return (paths.host_home / relative).resolve(strict=False)


def _translate_host_to_virtual(path: Path, paths: BridgePaths) -> Path:
    if path == paths.host_home:
        return paths.virtual_home
    try:
        relative = path.relative_to(paths.host_home)
    except ValueError:
        return path
    return paths.virtual_home / relative


def _under_allowed_root(path: Path, paths: BridgePaths) -> bool:
    normalized = path.resolve(strict=False)
    for root in paths.allowed_roots:
        normalized_root = root.resolve(strict=False)
        try:
            normalized.relative_to(normalized_root)
            return True
        except ValueError:
            continue
    return False


def require_allowed_path(raw_path: str, paths: BridgePaths) -> Path:
    if not raw_path.strip():
        raise BridgeError("Path must not be empty.")
    candidate = Path(raw_path).expanduser()
    translated = _translate_virtual_to_host(candidate, paths).resolve(strict=False)
    if not _under_allowed_root(translated, paths):
        roots = ", ".join(str(_translate_host_to_virtual(root, paths)) for root in paths.allowed_roots)
        raise BridgeError(f"Path is outside allowed synced roots: {raw_path}. Allowed roots: {roots}")
    return translated


def translate_text_to_virtual(text: str, paths: BridgePaths) -> str:
    if not text:
        return text
    return text.replace(str(paths.host_home), str(paths.virtual_home))


def sanitize_for_response(value: Any, paths: BridgePaths) -> Any:
    if isinstance(value, str):
        return translate_text_to_virtual(value, paths)
    if isinstance(value, list):
        return [sanitize_for_response(item, paths) for item in value]
    if isinstance(value, dict):
        return {key: sanitize_for_response(item, paths) for key, item in value.items()}
    return value


def run_git_visibility(args: list[str], cwd: Path) -> str:
    process = subprocess.run(
        ["git", *args],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    stdout = (process.stdout or "").strip()
    stderr = (process.stderr or "").strip()
    if process.returncode != 0:
        detail = stderr or stdout or "unknown git error"
        raise BridgeError(f"git {' '.join(args)} failed: {detail}")
    return stdout or stderr or "(no output)"


def run_git(args: list[str], cwd: Path, check: bool = True) -> str:
    process = subprocess.run(
        ["git", *args],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    stdout = (process.stdout or "").strip()
    stderr = (process.stderr or "").strip()
    if check and process.returncode != 0:
        detail = stderr or stdout or "unknown git error"
        raise BridgeError(f"git {' '.join(args)} failed: {detail}")
    return stdout or stderr or "(no output)"


def protected_branches() -> set[str]:
    raw = os.environ.get("REQUEST_REVIEW_INTEGRATION_BRANCHES", "main master staging prod production")
    return {item.strip() for item in re.split(r"[\s,]+", raw) if item.strip()}


def current_branch(cwd: Path) -> str:
    return run_git(["rev-parse", "--abbrev-ref", "HEAD"], cwd=cwd)


def ensure_branch_allows_destructive_mutation(cwd: Path) -> str:
    branch = current_branch(cwd)
    if branch in protected_branches():
        raise BridgeError(
            f"Refusing destructive mutation on protected integration branch '{branch}'. "
            "Only additive mutations and abort flows are allowed."
        )
    return branch
