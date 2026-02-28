from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from dotenv import dotenv_values
from dotenv import load_dotenv
from fastmcp import FastMCP
from github import Auth
from github import Github
from github.GithubObject import NotSet
from github.Repository import Repository

PROJECT_ROOT = Path(__file__).resolve().parents[2]
REQUEST_REVIEW_ENV_PATH = Path.home() / ".codex" / "skills" / "request-review" / ".env"
MAX_TOOL_OUTPUT_CHARS = max(256, int(os.getenv("GITOPS_MAX_OUTPUT_CHARS", "4000")))


def _load_environment() -> None:
    # Local MCP env is highest priority for this server process.
    load_dotenv(PROJECT_ROOT / ".env", override=True)
    # Request-review env is operator-authoritative for review behavior.
    # It must override MCP-local defaults for shared review knobs.
    load_dotenv(REQUEST_REVIEW_ENV_PATH, override=True)


def _refresh_review_environment() -> None:
    # Re-read on every review tool call so operator edits take effect without MCP restart.
    load_dotenv(PROJECT_ROOT / ".env", override=True)

    # Reset review-scoped knobs so removals in request-review/.env are respected.
    for key in [k for k in os.environ if k.startswith("REQUEST_REVIEW_")]:
        os.environ.pop(key, None)

    values = dotenv_values(REQUEST_REVIEW_ENV_PATH)
    for key, value in values.items():
        if key is None or not key.startswith("REQUEST_REVIEW_"):
            continue
        if value is None:
            continue
        os.environ[key] = value


_load_environment()

mcp = FastMCP("gitops-mcp")


class GitOpsError(RuntimeError):
    pass


@dataclass(frozen=True)
class GitResult:
    stdout: str
    stderr: str
    returncode: int


@dataclass(frozen=True)
class ReviewSettings:
    mode: str
    disable: bool
    bot_login: str
    trigger_comment: str
    poll_interval_seconds: int
    local_profile: str
    local_output_file: str
    local_error_file: str
    local_keep_debug_logs: bool


def _run_git(repo_root: Path, args: list[str], check: bool = True) -> GitResult:
    process = subprocess.run(
        ["git", *args],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    result = GitResult(
        stdout=process.stdout.strip(),
        stderr=process.stderr.strip(),
        returncode=process.returncode,
    )
    if check and process.returncode != 0:
        raise GitOpsError(f"git {' '.join(args)} failed: {result.stderr or result.stdout}")
    return result


def _run_command(args: list[str], cwd: Path, check: bool = True) -> GitResult:
    process = subprocess.run(
        args,
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    result = GitResult(
        stdout=process.stdout,
        stderr=process.stderr,
        returncode=process.returncode,
    )
    if check and process.returncode != 0:
        raise GitOpsError(
            f"{' '.join(args)} failed (exit {process.returncode}): {(result.stderr or result.stdout).strip()}"
        )
    return result


def _resolve_repo_root(repo_path: str | None = None) -> Path:
    start = Path(repo_path).expanduser() if repo_path else Path.cwd()
    process = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        cwd=start,
        capture_output=True,
        text=True,
    )
    if process.returncode != 0:
        raise GitOpsError("Not inside a git repository.")
    return Path(process.stdout.strip())


def _resolve_repo_and_git_cwd(repo_path: str | None = None) -> tuple[Path, Path]:
    start = Path(repo_path).expanduser() if repo_path else Path.cwd()
    if not start.exists():
        raise GitOpsError(f"Path does not exist: {start}")

    git_cwd = start.resolve()
    if git_cwd.is_file():
        git_cwd = git_cwd.parent

    repo_root = _resolve_repo_root(str(git_cwd))
    try:
        git_cwd.relative_to(repo_root)
    except ValueError:
        git_cwd = repo_root
    return repo_root, git_cwd


def _parse_origin_repo_full_name(origin_url: str) -> str:
    value = origin_url.strip()
    patterns = [
        r"^git@github\.com:(?P<repo>[^\s]+?)(?:\.git)?$",
        r"^https://github\.com/(?P<repo>[^\s]+?)(?:\.git)?$",
        r"^ssh://git@github\.com/(?P<repo>[^\s]+?)(?:\.git)?$",
    ]
    for pattern in patterns:
        match = re.match(pattern, value)
        if match:
            return match.group("repo")
    raise GitOpsError(f"Could not parse GitHub repo from origin URL: {origin_url}")


def _resolve_repo_full_name(repo_root: Path, repo_full_name: str | None = None) -> str:
    if repo_full_name:
        return repo_full_name
    origin = _run_git(repo_root, ["remote", "get-url", "origin"]).stdout
    if not origin:
        raise GitOpsError("Git remote 'origin' is not configured.")
    return _parse_origin_repo_full_name(origin)


def _github_client() -> Github:
    token = os.getenv("GITHUB_TOKEN", "").strip()
    if not token:
        raise GitOpsError("Missing GITHUB_TOKEN. Add it to ~/.codex/mcp/gitops/.env")
    return Github(auth=Auth.Token(token))


def _to_iso8601(value: datetime | None) -> str | None:
    if value is None:
        return None
    if value.tzinfo is None:
        value = value.replace(tzinfo=timezone.utc)
    return value.astimezone(timezone.utc).isoformat()


def _branch_name(repo_root: Path) -> str:
    return _run_git(repo_root, ["rev-parse", "--abbrev-ref", "HEAD"]).stdout


def _branch_name_at(git_cwd: Path) -> str:
    return _run_git(git_cwd, ["rev-parse", "--abbrev-ref", "HEAD"]).stdout


def _parse_bool_env(key: str, default: bool = False) -> bool:
    raw = os.getenv(key)
    if raw is None:
        return default
    normalized = raw.strip().lower()
    return normalized not in {"", "0", "false", "no", "off"}


def _review_settings() -> ReviewSettings:
    _refresh_review_environment()

    mode = os.getenv("REQUEST_REVIEW_MODE", "local").strip().lower()
    if mode not in {"local", "remote"}:
        raise GitOpsError(
            f"Unsupported REQUEST_REVIEW_MODE='{mode}'. Expected 'local' or 'remote'."
        )

    poll_raw = os.getenv("REQUEST_REVIEW_POLL_INTERVAL_SECONDS", "20").strip()
    try:
        poll_seconds = max(1, int(poll_raw))
    except ValueError as exc:
        raise GitOpsError(
            f"Invalid REQUEST_REVIEW_POLL_INTERVAL_SECONDS='{poll_raw}'"
        ) from exc

    return ReviewSettings(
        mode=mode,
        disable=_parse_bool_env("REQUEST_REVIEW_DISABLE", False),
        bot_login=os.getenv("REQUEST_REVIEW_BOT_LOGIN", "chatgpt-codex-connector[bot]").strip(),
        trigger_comment=os.getenv("REQUEST_REVIEW_TRIGGER_COMMENT", "@codex review").strip(),
        poll_interval_seconds=poll_seconds,
        local_profile=os.getenv("REQUEST_REVIEW_LOCAL_PROFILE", "local-review").strip(),
        local_output_file=os.getenv("REQUEST_REVIEW_LOCAL_OUTPUT_FILE", "review.log").strip(),
        local_error_file=os.getenv("REQUEST_REVIEW_LOCAL_ERROR_FILE", "review.err.log").strip(),
        local_keep_debug_logs=_parse_bool_env("REQUEST_REVIEW_LOCAL_KEEP_DEBUG_LOGS", False),
    )


def _protected_branches() -> set[str]:
    raw = os.getenv("REQUEST_REVIEW_INTEGRATION_BRANCHES", "main master staging prod production")
    return {item.strip() for item in re.split(r"[\s,]+", raw) if item.strip()}


def _ensure_branch_is_mutable(repo_root: Path) -> str:
    branch = _branch_name(repo_root)
    if branch in _protected_branches():
        raise GitOpsError(
            f"Refusing operation on protected integration branch '{branch}'. "
            "Switch to an issue/feature branch first."
        )
    return branch


def _ensure_branch_is_mutable_at(git_cwd: Path) -> str:
    branch = _branch_name_at(git_cwd)
    if branch in _protected_branches():
        raise GitOpsError(
            f"Refusing operation on protected integration branch '{branch}'. "
            "Switch to an issue/feature branch first."
        )
    return branch


def _ensure_branch_published(repo_root: Path, branch: str) -> None:
    push_result = _run_git(repo_root, ["push", "-u", "origin", "HEAD"], check=False)
    if push_result.returncode == 0:
        return
    detail = (push_result.stderr or push_result.stdout or "").strip() or "unknown push error"
    raise GitOpsError(
        f"Failed to publish branch '{branch}' to origin. "
        f"Cannot request remote review until push succeeds. Details: {detail}"
    )


def _sync_local_branch_after_remote_merge(repo_root: Path, branch: str) -> str:
    normalized_branch = (branch or "").strip()
    if not normalized_branch:
        raise GitOpsError("Cannot sync local branch: base branch is empty.")

    fetch_result = _run_git(repo_root, ["fetch", "origin", normalized_branch], check=False)
    if fetch_result.returncode != 0:
        detail = (fetch_result.stderr or fetch_result.stdout or "").strip() or "unknown fetch error"
        raise GitOpsError(
            f"Failed to fetch origin/{normalized_branch} before local sync: {detail}"
        )

    remote_tracking_ref = f"origin/{normalized_branch}"
    remote_full_ref = f"refs/remotes/origin/{normalized_branch}"
    remote_sha_result = _run_git(repo_root, ["rev-parse", "--verify", remote_full_ref], check=False)
    if remote_sha_result.returncode != 0:
        detail = (remote_sha_result.stderr or remote_sha_result.stdout or "").strip() or "unknown ref error"
        raise GitOpsError(
            f"Remote tracking ref {remote_full_ref} is unavailable: {detail}"
        )
    remote_sha = remote_sha_result.stdout.strip()

    try:
        worktree_path, _ = _resolve_worktree_target(repo_root, worktree_path=None, branch_name=normalized_branch)
    except GitOpsError:
        worktree_path = None

    if worktree_path is not None:
        status_result = _run_git(worktree_path, ["status", "--porcelain"], check=False)
        if status_result.returncode != 0:
            detail = (status_result.stderr or status_result.stdout or "").strip() or "unknown status error"
            raise GitOpsError(
                f"Failed to inspect working tree for '{normalized_branch}' at {worktree_path}: {detail}"
            )

        stash_created = False
        if status_result.stdout.strip():
            stash_marker = f"gitops-autostash-{normalized_branch}-{int(time.time())}"
            stash_result = _run_git(
                worktree_path,
                ["stash", "push", "-u", "-m", stash_marker],
                check=False,
            )
            if stash_result.returncode != 0:
                detail = (stash_result.stderr or stash_result.stdout or "").strip() or "unknown stash error"
                raise GitOpsError(
                    f"Failed to stash local changes before fast-forward in {worktree_path}: {detail}"
                )
            stash_text = (stash_result.stdout or stash_result.stderr or "").strip()
            stash_created = "No local changes to save" not in stash_text

        ff_result = _run_git(worktree_path, ["merge", "--ff-only", remote_tracking_ref], check=False)
        if ff_result.returncode != 0:
            detail = (ff_result.stderr or ff_result.stdout or "").strip() or "unknown merge error"
            raise GitOpsError(
                f"Failed to fast-forward '{normalized_branch}' in {worktree_path}: {detail}"
            )

        if stash_created:
            stash_pop = _run_git(worktree_path, ["stash", "pop"], check=False)
            if stash_pop.returncode != 0:
                detail = (stash_pop.stderr or stash_pop.stdout or "").strip() or "unknown stash pop error"
                raise GitOpsError(
                    f"Fast-forward succeeded for '{normalized_branch}' but restoring stashed changes failed in "
                    f"{worktree_path}: {detail}"
                )
            return _compact_inline_text(
                f"synced_local_branch: {normalized_branch} (worktree ff with stash restore) [{worktree_path}]"
            )

        return _compact_inline_text(
            f"synced_local_branch: {normalized_branch} (worktree ff) [{worktree_path}]"
        )

    local_ref = f"refs/heads/{normalized_branch}"
    local_exists = _run_git(repo_root, ["show-ref", "--verify", "--quiet", local_ref], check=False).returncode == 0
    if not local_exists:
        create_result = _run_git(repo_root, ["branch", normalized_branch, remote_tracking_ref], check=False)
        if create_result.returncode != 0:
            detail = (create_result.stderr or create_result.stdout or "").strip() or "unknown branch create error"
            raise GitOpsError(
                f"Unable to create local branch '{normalized_branch}' from {remote_tracking_ref}: {detail}"
            )
        return _compact_inline_text(
            f"synced_local_branch: {normalized_branch} (created from {remote_tracking_ref})"
        )

    local_sha = _run_git(repo_root, ["rev-parse", "--verify", local_ref]).stdout.strip()
    if local_sha == remote_sha:
        return _compact_inline_text(f"synced_local_branch: {normalized_branch} (already up to date)")

    merge_base = _run_git(repo_root, ["merge-base", local_sha, remote_sha]).stdout.strip()
    if merge_base != local_sha:
        raise GitOpsError(
            f"Local branch '{normalized_branch}' cannot be fast-forwarded safely (local={local_sha}, remote={remote_sha})."
        )

    update_result = _run_git(
        repo_root,
        ["update-ref", local_ref, remote_sha, local_sha],
        check=False,
    )
    if update_result.returncode != 0:
        detail = (update_result.stderr or update_result.stdout or "").strip() or "unknown update-ref error"
        raise GitOpsError(
            f"Failed to fast-forward local ref '{local_ref}' to '{remote_sha}': {detail}"
        )
    return _compact_inline_text(f"synced_local_branch: {normalized_branch} (ref ff to {remote_sha[:12]})")


def _has_staged_changes(repo_root: Path) -> bool:
    result = _run_git(repo_root, ["diff", "--cached", "--quiet"], check=False)
    if result.returncode == 0:
        return False
    if result.returncode == 1:
        return True
    raise GitOpsError(result.stderr or "Unable to determine staged changes.")


def _repo(repo_root: Path, repo_full_name: str | None = None) -> tuple[Repository, str]:
    full_name = _resolve_repo_full_name(repo_root, repo_full_name)
    gh = _github_client()
    return gh.get_repo(full_name), full_name


def _normalize_line(text: str) -> str:
    return text.replace("\r\n", "\n").replace("\r", "\n")


def _compact_text(text: str, max_chars: int = MAX_TOOL_OUTPUT_CHARS) -> str:
    normalized = text.strip()
    if len(normalized) <= max_chars:
        return normalized
    omitted = len(normalized) - max_chars
    head = normalized[: max_chars - 48].rstrip()
    return f"{head}\n... [truncated {omitted} chars]"


def _compact_inline_text(text: str, max_chars: int = 220) -> str:
    normalized = str(text or "").replace("\n", " ").strip()
    if len(normalized) <= max_chars:
        return normalized
    omitted = len(normalized) - max_chars
    head = normalized[: max_chars - 32].rstrip()
    return f"{head}... (+{omitted} chars)"


def _notset_if_none(value: Any) -> Any:
    return NotSet if value is None else value


def _slug(text: str) -> str:
    value = re.sub(r"[^a-zA-Z0-9._-]+", "-", text.strip())
    value = re.sub(r"-+", "-", value).strip("-")
    return value or "worktree"


def _worktrees_root(repo_root: Path) -> Path:
    relative = os.getenv("GITOPS_WORKTREE_DIR", ".worktrees").strip()
    if not relative:
        relative = ".worktrees"
    root = (repo_root / relative).resolve()
    try:
        root.relative_to(repo_root.resolve())
    except ValueError as exc:
        raise GitOpsError(
            f"GITOPS_WORKTREE_DIR must resolve under repo root. "
            f"repo={repo_root} configured={relative} resolved={root}"
        ) from exc
    return root


def _resolve_trash_bin() -> str:
    preferred = Path("/usr/local/bin/trash")
    if preferred.exists() and os.access(preferred, os.X_OK):
        return str(preferred)
    fallback = shutil.which("trash")
    if fallback:
        return fallback
    raise GitOpsError(
        "Missing trash CLI for worktree cleanup. Expected '/usr/local/bin/trash' "
        "or a 'trash' executable on PATH."
    )


def _worktree_entries(repo_root: Path) -> list[dict[str, str]]:
    listing = _run_git(repo_root, ["worktree", "list", "--porcelain"]).stdout
    entries: list[dict[str, str]] = []
    current: dict[str, str] = {}
    for line in listing.splitlines():
        raw = line.strip()
        if not raw:
            if current:
                entries.append(current)
                current = {}
            continue
        if " " in raw:
            key, value = raw.split(" ", 1)
            current[key] = value.strip()
        else:
            current[raw] = "true"
    if current:
        entries.append(current)
    return entries


def _normalize_branch_ref(value: str | None) -> str | None:
    if value is None:
        return None
    trimmed = value.strip()
    if not trimmed:
        return None
    if trimmed.startswith("refs/heads/"):
        return trimmed.removeprefix("refs/heads/")
    return trimmed


def _resolve_worktree_target(
    repo_root: Path,
    worktree_path: str | None,
    branch_name: str | None,
) -> tuple[Path, str | None]:
    entries = _worktree_entries(repo_root)
    if not entries:
        raise GitOpsError("No worktrees found for repository.")

    requested_path = Path(worktree_path).expanduser().resolve() if worktree_path else None
    requested_branch = _normalize_branch_ref(branch_name)

    for entry in entries:
        path_value = entry.get("worktree")
        if not path_value:
            continue
        entry_path = Path(path_value).expanduser().resolve()
        entry_branch = _normalize_branch_ref(entry.get("branch"))
        path_match = requested_path is not None and entry_path == requested_path
        branch_match = requested_branch is not None and entry_branch == requested_branch
        if path_match or branch_match:
            return entry_path, entry_branch

    if requested_path is not None:
        raise GitOpsError(f"Worktree path is not registered: {requested_path}")
    if requested_branch is not None:
        raise GitOpsError(f"Branch is not attached to any worktree: {requested_branch}")
    raise GitOpsError("Unable to resolve worktree target.")


def _extract_local_review_message(raw_jsonl: str) -> str:
    message = ""
    for line in raw_jsonl.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue

        if event.get("type") != "item.completed":
            continue
        item = event.get("item") or {}
        if item.get("type") != "agent_message":
            continue

        candidate = str(item.get("text") or "").strip()
        if candidate:
            message = candidate

    if message:
        return message
    return raw_jsonl.strip()


def _ensure_local_review_profile_exists(profile_name: str) -> None:
    config_path = Path.home() / ".codex" / "config.toml"
    if not config_path.exists():
        raise GitOpsError(f"Missing config file: {config_path}")

    content = config_path.read_text(encoding="utf-8", errors="replace")
    pattern = rf"^\[profiles\.{re.escape(profile_name)}\]$"
    if re.search(pattern, content, flags=re.MULTILINE) is None:
        raise GitOpsError(
            f"Profile '{profile_name}' not found in {config_path}."
        )


def _run_local_review(
    repo_root: Path,
    review_sha: str,
    commit_message: str,
    settings: ReviewSettings,
) -> dict[str, Any]:
    if not settings.local_profile:
        raise GitOpsError("REQUEST_REVIEW_LOCAL_PROFILE cannot be empty.")

    _ensure_local_review_profile_exists(settings.local_profile)

    output_file = settings.local_output_file or "review.log"
    error_file = settings.local_error_file or "review.err.log"

    output_path = repo_root / output_file
    error_path = repo_root / error_file
    raw_jsonl_path = repo_root / ".request-review.events.jsonl"

    cmd = [
        "codex",
        "exec",
        "-C",
        str(repo_root),
        "-s",
        "read-only",
        "-p",
        settings.local_profile,
        "--json",
        "review",
        "--commit",
        review_sha,
        "--title",
        commit_message,
    ]

    result = _run_command(cmd, cwd=repo_root, check=False)

    raw_stdout = result.stdout or ""
    raw_stderr = result.stderr or ""
    raw_jsonl_path.write_text(raw_stdout, encoding="utf-8")
    error_path.write_text(raw_stderr, encoding="utf-8")

    message = _extract_local_review_message(raw_stdout)
    if not message:
        message = f"Local review produced no parsable output. See {error_path}."
    output_path.write_text(message + "\n", encoding="utf-8")

    if not settings.local_keep_debug_logs and raw_jsonl_path.exists():
        raw_jsonl_path.unlink()

    payload = {
        "status": "local_review_complete" if result.returncode == 0 else "local_review_failed",
        "reviewSha": review_sha,
        "profile": settings.local_profile,
        "message": message,
        "outputFile": str(output_path),
        "errorFile": str(error_path),
        "exitCode": result.returncode,
    }

    if result.returncode != 0:
        raise GitOpsError(
            f"Local review command failed with exit {result.returncode}. "
            f"See {error_path}. Summary: {message}"
        )

    return payload


@mcp.tool(output_schema=None)
def git_worktree_create(
    branch_name: str,
    worktree_name: str | None = None,
    base_branch: str | None = None,
    repo_path: str | None = None,
) -> str:
    """Create a worktree under repo-local .worktrees/ for a branch."""
    repo_root = _resolve_repo_root(repo_path)
    base = (base_branch or os.getenv("GITOPS_INTEGRATION_BRANCH", "integration")).strip()
    if not base:
        raise GitOpsError("base_branch resolved to an empty string")

    target_parent = _worktrees_root(repo_root)
    target_parent.mkdir(parents=True, exist_ok=True)
    name = worktree_name.strip() if worktree_name else _slug(branch_name.replace("/", "-"))
    target_dir = target_parent / name
    if target_dir.exists():
        raise GitOpsError(f"Worktree target already exists: {target_dir}")

    _run_git(repo_root, ["fetch", "origin", base], check=False)

    branch_exists = (
        _run_git(repo_root, ["show-ref", "--verify", "--quiet", f"refs/heads/{branch_name}"], check=False).returncode
        == 0
    )

    if branch_exists:
        add_result = _run_git(repo_root, ["worktree", "add", str(target_dir), branch_name])
        start_point = branch_name
    else:
        local_base_exists = _run_git(
            repo_root,
            ["show-ref", "--verify", "--quiet", f"refs/heads/{base}"],
            check=False,
        ).returncode == 0
        start_point = base if local_base_exists else f"origin/{base}"
        add_result = _run_git(repo_root, ["worktree", "add", "-b", branch_name, str(target_dir), start_point])

    text = (add_result.stderr or add_result.stdout).strip()
    if text:
        return _compact_text(text)
    return _compact_text(str(target_dir))


@mcp.tool(output_schema=None)
def git_worktree_cleanup(
    worktree_path: str,
) -> str:
    """Trash a worktree directory and prune stale worktree metadata."""
    if not worktree_path.strip():
        raise GitOpsError("worktree_path is required.")

    target_path = Path(worktree_path).expanduser().resolve()
    repo_root = _resolve_repo_root(str(target_path))
    root_path = repo_root.resolve()
    normalized_target = str(target_path)

    entries = _worktree_entries(repo_root)
    registered_entry: dict[str, str] | None = None
    for entry in entries:
        value = entry.get("worktree")
        if not value:
            continue
        if str(Path(value).expanduser().resolve()) == normalized_target:
            registered_entry = entry
            break

    if registered_entry is None:
        raise GitOpsError(f"Worktree path is not registered: {target_path}")

    if target_path == root_path:
        raise GitOpsError("Refusing to trash repository root worktree.")

    trash_bin = _resolve_trash_bin()
    lines: list[str] = []
    if target_path.exists():
        trash_result = _run_command(
            [trash_bin, "--", str(target_path)],
            cwd=repo_root,
            check=False,
        )
        if trash_result.returncode != 0:
            detail = (trash_result.stderr or trash_result.stdout or "unknown error").strip()
            raise GitOpsError(f"Failed to trash worktree directory '{target_path}': {detail}")
        lines.append(f"Trashed worktree directory: {target_path}")
    else:
        lines.append(f"Worktree directory already missing: {target_path}")

    prune_result = _run_git(repo_root, ["worktree", "prune"], check=False)
    if prune_result.returncode != 0:
        detail = (prune_result.stderr or prune_result.stdout or "unknown error").strip()
        raise GitOpsError(f"git worktree prune failed after trashing '{target_path}': {detail}")
    prune_text = (prune_result.stderr or prune_result.stdout).strip()
    if prune_text:
        lines.append(prune_text)
    else:
        lines.append("Pruned stale worktree metadata.")

    remaining_entries = _worktree_entries(repo_root)
    still_registered = False
    for entry in remaining_entries:
        value = entry.get("worktree")
        if not value:
            continue
        if str(Path(value).expanduser().resolve()) == normalized_target:
            still_registered = True
            break
    if still_registered:
        raise GitOpsError(
            f"Worktree directory was trashed but remains registered after prune: {target_path}"
        )

    return _compact_text("\n".join(lines))


@mcp.tool(output_schema=None)
def git_commit(
    message: str,
    repo_path: str | None = None,
    add_all: bool = True,
    allow_empty: bool = False,
) -> str:
    """Create a guarded commit. Refuses to commit on protected integration branches."""
    if not message.strip():
        raise GitOpsError("Commit message cannot be empty.")

    repo_root = _resolve_repo_root(repo_path)
    branch = _ensure_branch_is_mutable(repo_root)

    if add_all:
        _run_git(repo_root, ["add", "-A"])

    if not allow_empty and not _has_staged_changes(repo_root):
        raise GitOpsError("No staged changes to commit.")

    commit_args = ["commit", "-m", message]
    if allow_empty:
        commit_args.append("--allow-empty")
    _run_git(repo_root, commit_args)
    return _compact_text(_run_git(repo_root, ["log", "--oneline", "-n", "1"]).stdout)


@mcp.tool(output_schema=None)
def git_fetch(
    repo_path: str | None = None,
    remote: str = "origin",
    refspec: str | None = None,
    prune: bool = False,
    tags: bool = False,
) -> str:
    """Fetch from remote with optional refspec."""
    _, git_cwd = _resolve_repo_and_git_cwd(repo_path)
    normalized_remote = (remote or "").strip() or "origin"
    normalized_refspec = (refspec or "").strip()

    args = ["fetch"]
    if prune:
        args.append("--prune")
    if tags:
        args.append("--tags")
    args.append(normalized_remote)
    if normalized_refspec:
        args.append(normalized_refspec)

    result = _run_git(git_cwd, args, check=False)
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip() or "unknown fetch error"
        raise GitOpsError(f"git {' '.join(args)} failed: {detail}")

    text = (result.stderr or result.stdout).strip()
    if text:
        return _compact_text(text)
    if normalized_refspec:
        return _compact_text(f"Fetched {normalized_remote} {normalized_refspec}")
    return _compact_text(f"Fetched {normalized_remote}")


@mcp.tool(output_schema=None)
def git_rebase(
    repo_path: str | None = None,
    upstream: str | None = None,
    remote: str = "origin",
    fetch_first: bool = True,
    autostash: bool = True,
) -> str:
    """Rebase current branch in the selected worktree/repo context."""
    _, git_cwd = _resolve_repo_and_git_cwd(repo_path)
    branch = _ensure_branch_is_mutable_at(git_cwd)

    normalized_remote = (remote or "").strip() or "origin"
    normalized_upstream = (upstream or "").strip()
    if not normalized_upstream:
        integration = (os.getenv("GITOPS_INTEGRATION_BRANCH", "integration") or "").strip() or "integration"
        normalized_upstream = f"{normalized_remote}/{integration}"

    if fetch_first:
        fetch_result = _run_git(git_cwd, ["fetch", "--prune", normalized_remote], check=False)
        if fetch_result.returncode != 0:
            detail = (fetch_result.stderr or fetch_result.stdout).strip() or "unknown fetch error"
            raise GitOpsError(f"git fetch --prune {normalized_remote} failed: {detail}")

    args = ["rebase"]
    if autostash:
        args.append("--autostash")
    args.append(normalized_upstream)
    result = _run_git(git_cwd, args, check=False)
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip() or "unknown rebase error"
        raise GitOpsError(f"git {' '.join(args)} failed on branch '{branch}': {detail}")

    text = (result.stdout or result.stderr).strip()
    if text:
        return _compact_text(text)
    return _compact_text(f"Rebased {branch} onto {normalized_upstream}")


@mcp.tool(output_schema=None)
def git_request_review_and_wait(
    commit_message: str,
    repo_path: str | None = None,
    use_existing_commit: bool = False,
    existing_commit_sha: str | None = None,
    create_pr_if_missing: bool = True,
    pr_title: str | None = None,
    pr_body: str | None = None,
) -> str:
    """Request review using env-controlled mode (local/remote). Agents cannot select mode directly."""
    if not commit_message.strip():
        raise GitOpsError("commit_message cannot be empty.")

    settings = _review_settings()

    repo_root = _resolve_repo_root(repo_path)
    branch = _ensure_branch_is_mutable(repo_root)

    using_existing_commit = use_existing_commit or bool(existing_commit_sha and existing_commit_sha.strip())
    if using_existing_commit:
        ref = (existing_commit_sha or "HEAD").strip()
        review_sha = _run_git(repo_root, ["rev-parse", "--verify", f"{ref}^{{commit}}"], check=True).stdout
    else:
        _run_git(repo_root, ["add", "-A"])
        if not _has_staged_changes(repo_root):
            raise GitOpsError("No staged changes to commit for review.")
        _run_git(repo_root, ["commit", "-m", commit_message])
        review_sha = _run_git(repo_root, ["rev-parse", "HEAD"]).stdout

    if settings.disable:
        if settings.mode == "remote" and not using_existing_commit:
            _run_git(repo_root, ["push", "-u", "origin", "HEAD"])
        return "all clear!"

    if settings.mode == "local":
        local_payload = _run_local_review(repo_root, review_sha, commit_message, settings)
        message = str(local_payload.get("message", "")).strip() or "Local review completed."
        return _compact_text(message)

    if settings.mode != "remote":
        raise GitOpsError(f"Unsupported review mode: {settings.mode}")

    # Remote review requires a published branch for PR discovery/creation.
    # Always ensure origin has this branch, including existing-commit flows.
    _ensure_branch_published(repo_root, branch)

    repo, full_name = _repo(repo_root)
    owner_login = repo.owner.login
    pulls = list(repo.get_pulls(state="open", head=f"{owner_login}:{branch}", sort="updated", direction="desc"))
    pull = pulls[0] if pulls else None

    if pull is None:
        if not create_pr_if_missing:
            raise GitOpsError(f"No open PR found for branch '{branch}' in {full_name}.")
        base = (os.getenv("REQUEST_REVIEW_BASE_BRANCH") or repo.default_branch).strip()
        title = (pr_title or commit_message).strip()
        body = _normalize_line(pr_body or os.getenv("REQUEST_REVIEW_PR_BODY", "Automated review request from gitops-mcp."))
        pull = repo.create_pull(title=title, body=body, head=branch, base=base)

    if not settings.trigger_comment:
        raise GitOpsError("REQUEST_REVIEW_TRIGGER_COMMENT cannot be empty.")
    pull.create_issue_comment(settings.trigger_comment)

    if not settings.bot_login:
        raise GitOpsError("REQUEST_REVIEW_BOT_LOGIN cannot be empty.")

    commit_time_text = _run_git(repo_root, ["show", "-s", "--format=%cI", review_sha]).stdout
    commit_time = datetime.fromisoformat(commit_time_text.replace("Z", "+00:00")).astimezone(timezone.utc)

    saw_eyes = False
    while True:
        inline_comments: list[dict[str, Any]] = []
        for comment in pull.get_comments():
            created = comment.created_at
            if created.tzinfo is None:
                created = created.replace(tzinfo=timezone.utc)
            matches_commit = comment.commit_id == review_sha or getattr(comment, "original_commit_id", None) == review_sha
            if (
                comment.user
                and comment.user.login == settings.bot_login
                and matches_commit
                and created.astimezone(timezone.utc) > commit_time
            ):
                inline_comments.append(
                    {
                        "id": comment.id,
                        "url": comment.html_url,
                        "path": comment.path,
                        "line": comment.line,
                        "body": comment.body,
                        "createdAt": _to_iso8601(created),
                    }
                )

        if inline_comments:
            lines = [f"changes_requested", f"PR: {pull.html_url}"]
            for comment in inline_comments:
                path = comment.get("path") or "<unknown>"
                line = comment.get("line")
                body = str(comment.get("body") or "").strip()
                coord = f"{path}:{line}" if line is not None else str(path)
                if body:
                    lines.append(f"- {coord} {body}")
                else:
                    lines.append(f"- {coord}")
            return _compact_text("\n".join(lines))

        thumbs_up = False
        for reaction in pull.as_issue().get_reactions():
            created = reaction.created_at
            if created.tzinfo is None:
                created = created.replace(tzinfo=timezone.utc)
            if not reaction.user or reaction.user.login != settings.bot_login:
                continue
            if created.astimezone(timezone.utc) <= commit_time:
                continue
            if reaction.content == "eyes":
                saw_eyes = True
            if reaction.content == "+1":
                thumbs_up = True

        if thumbs_up:
            return _compact_text(f"approved\nPR: {pull.html_url}")

        time.sleep(settings.poll_interval_seconds)


def _issue_to_text(issue: Any) -> str:
    return _compact_text(f"#{issue.number} [{issue.state}] {issue.title}\n{issue.html_url}")


def _pull_to_text(pull: Any) -> str:
    draft = " draft" if pull.draft else ""
    return _compact_text(f"PR #{pull.number} [{pull.state}{draft}] {pull.title}\n{pull.html_url}")


def _pull_detail_text(pull: Any, include_body: bool = True) -> str:
    draft = " draft" if pull.draft else ""
    lines: list[str] = [
        f"PR #{pull.number} [{pull.state}{draft}] {pull.title}",
        str(pull.html_url),
        f"base: {getattr(getattr(pull, 'base', None), 'ref', None) or 'unknown'}",
        f"head: {getattr(getattr(pull, 'head', None), 'ref', None) or 'unknown'}",
    ]
    mergeable_state = getattr(pull, "mergeable_state", None)
    if mergeable_state is not None:
        lines.append(f"mergeable_state: {mergeable_state}")
    if include_body:
        body = str(getattr(pull, "body", "") or "").strip()
        if body:
            lines.append("body:")
            lines.append(body)
        else:
            lines.append("body: (empty)")
    return _compact_text("\n".join(lines))


def _issue_detail_text(issue: Any, include_body: bool = True, include_comments: bool = True, comment_limit: int = 30) -> str:
    lines: list[str] = [
        f"#{issue.number} [{issue.state}] {issue.title}",
        str(issue.html_url),
        f"comments: {int(getattr(issue, 'comments', 0) or 0)}",
    ]

    if include_body:
        body = str(issue.body or "").strip()
        if body:
            lines.append("body:")
            lines.append(body)
        else:
            lines.append("body: (empty)")

    if include_comments:
        comments = issue.get_comments()
        rendered: list[str] = []
        max_comments = max(1, comment_limit)
        for idx, comment in enumerate(comments):
            if idx >= max_comments:
                break
            login = getattr(getattr(comment, "user", None), "login", None) or "unknown"
            created_at = _to_iso8601(getattr(comment, "created_at", None)) or "unknown-time"
            body = str(getattr(comment, "body", "") or "").strip()
            if not body:
                body = "(empty)"
            rendered.append(f"- {login} @ {created_at}\n{body}")

        if rendered:
            lines.append("comments_detail:")
            lines.extend(rendered)
        else:
            lines.append("comments_detail: (none)")

    return _compact_text("\n".join(lines))


@mcp.tool(output_schema=None)
def github_get_issue(
    issue_number: int,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
    include_body: bool = True,
    include_comments: bool = True,
    comment_limit: int = 30,
) -> str:
    """Fetch a single GitHub issue."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    issue = repo.get_issue(number=issue_number)
    return _issue_detail_text(
        issue,
        include_body=include_body,
        include_comments=include_comments,
        comment_limit=comment_limit,
    )


@mcp.tool(output_schema=None)
def github_list_issue_comments(
    issue_number: int,
    limit: int = 50,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """List comments on a GitHub issue."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    issue = repo.get_issue(number=issue_number)
    items: list[str] = []
    for idx, comment in enumerate(issue.get_comments()):
        if idx >= max(1, limit):
            break
        login = getattr(getattr(comment, "user", None), "login", None) or "unknown"
        created_at = _to_iso8601(getattr(comment, "created_at", None)) or "unknown-time"
        body = str(getattr(comment, "body", "") or "").strip() or "(empty)"
        items.append(f"- {login} @ {created_at}\n{body}")
    return _compact_text("\n".join(items) if items else "(no comments)")


@mcp.tool(output_schema=None)
def github_list_issues(
    state: str = "open",
    limit: int = 25,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """List GitHub issues."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    items: list[str] = []
    for issue in repo.get_issues(state=state):
        if issue.pull_request is not None:
            continue
        items.append(_issue_to_text(issue))
        if len(items) >= max(1, limit):
            break
    return _compact_text("\n\n".join(items) if items else "(no issues)")


@mcp.tool(output_schema=None)
def github_create_issue(
    title: str,
    body: str | None = None,
    labels: list[str] | None = None,
    assignees: list[str] | None = None,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """Create a GitHub issue."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    issue = repo.create_issue(
        title=title,
        body=_notset_if_none(body),
        labels=_notset_if_none(labels),
        assignees=_notset_if_none(assignees),
    )
    return _issue_to_text(issue)


@mcp.tool(output_schema=None)
def github_update_issue(
    issue_number: int,
    title: str | None = None,
    body: str | None = None,
    state: str | None = None,
    labels: list[str] | None = None,
    assignees: list[str] | None = None,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """Update issue fields and return the refreshed issue."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    issue = repo.get_issue(number=issue_number)
    issue.edit(
        title=_notset_if_none(title),
        body=_notset_if_none(body),
        state=_notset_if_none(state),
        labels=_notset_if_none(labels),
        assignees=_notset_if_none(assignees),
    )
    refreshed = repo.get_issue(number=issue_number)
    return _issue_to_text(refreshed)


@mcp.tool(output_schema=None)
def github_add_issue_comment(
    issue_number: int,
    body: str,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """Add a comment on a GitHub issue."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    issue = repo.get_issue(number=issue_number)
    comment = issue.create_comment(body)
    return _compact_text(f"{comment.html_url}")


@mcp.tool(output_schema=None)
def github_get_pull_request(
    pull_number: int,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
    include_body: bool = True,
) -> str:
    """Fetch a single GitHub pull request."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    pull = repo.get_pull(number=pull_number)
    return _pull_detail_text(pull, include_body=include_body)


@mcp.tool(output_schema=None)
def github_list_pull_requests(
    state: str = "open",
    limit: int = 25,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """List pull requests."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    items: list[str] = []
    for pull in repo.get_pulls(state=state, sort="updated", direction="desc"):
        items.append(_pull_to_text(pull))
        if len(items) >= max(1, limit):
            break
    return _compact_text("\n\n".join(items) if items else "(no pull requests)")


@mcp.tool(output_schema=None)
def github_create_pull_request(
    title: str,
    head: str,
    base: str,
    body: str = "",
    draft: bool = False,
    maintainer_can_modify: bool = True,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """Create a pull request."""
    if not title.strip():
        raise GitOpsError("title cannot be empty.")
    if not head.strip():
        raise GitOpsError("head cannot be empty.")
    if not base.strip():
        raise GitOpsError("base cannot be empty.")

    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    pull = repo.create_pull(
        base=base.strip(),
        head=head.strip(),
        title=title.strip(),
        body=body,
        draft=draft,
        maintainer_can_modify=maintainer_can_modify,
    )
    return _pull_detail_text(pull, include_body=True)


@mcp.tool(output_schema=None)
def github_update_pull_request(
    pull_number: int,
    title: str | None = None,
    body: str | None = None,
    state: str | None = None,
    base: str | None = None,
    maintainer_can_modify: bool | None = None,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """Update pull request fields and return refreshed pull request."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    pull = repo.get_pull(number=pull_number)
    pull.edit(
        title=title if title is not None else NotSet,
        body=body if body is not None else NotSet,
        state=state if state is not None else NotSet,
        base=base if base is not None else NotSet,
        maintainer_can_modify=maintainer_can_modify if maintainer_can_modify is not None else NotSet,
    )
    refreshed = repo.get_pull(number=pull_number)
    return _pull_detail_text(refreshed, include_body=True)


@mcp.tool(output_schema=None)
def github_merge_pull_request(
    pull_number: int,
    merge_method: str = "squash",
    commit_message: str | None = None,
    expected_head_sha: str | None = None,
    delete_branch: bool = False,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """Merge a pull request (default: squash) and optionally delete its remote branch."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    pull = repo.get_pull(number=pull_number)

    method = merge_method.strip().lower()
    if method not in {"merge", "squash", "rebase"}:
        raise GitOpsError("merge_method must be one of: merge, squash, rebase")

    if pull.state != "open":
        raise GitOpsError(f"PR #{pull_number} is not open (state={pull.state}).")

    normalized_commit_message = (commit_message or "").strip()
    normalized_expected_sha = (expected_head_sha or "").strip()
    result = pull.merge(
        commit_message=normalized_commit_message or NotSet,
        sha=normalized_expected_sha or NotSet,
        merge_method=method,
    )
    if not result.merged:
        raise GitOpsError(result.message or f"PR #{pull_number} merge failed.")

    lines = [
        f"merged via {method}",
        f"PR: {pull.html_url}",
    ]
    if result.sha:
        lines.append(f"merge_sha: {result.sha}")

    base_branch = (pull.base.ref or "").strip()
    try:
        lines.append(_sync_local_branch_after_remote_merge(repo_root, base_branch))
    except GitOpsError as exc:
        raise GitOpsError(
            f"PR #{pull_number} merged remotely, but local base-branch sync failed for '{base_branch}': {exc}"
        ) from exc

    if delete_branch:
        head_ref = (pull.head.ref or "").strip()
        if not head_ref:
            raise GitOpsError("PR merged but head branch is unknown; remote branch delete skipped.")
        if head_ref in _protected_branches():
            raise GitOpsError(
                f"PR merged but refusing to delete protected branch '{head_ref}'."
            )
        if head_ref == repo.default_branch:
            raise GitOpsError(
                f"PR merged but refusing to delete repository default branch '{head_ref}'."
            )
        git_ref = repo.get_git_ref(f"heads/{head_ref}")
        git_ref.delete()
        lines.append(f"deleted_remote_branch: {head_ref}")

    return _compact_text("\n".join(lines))


@mcp.tool(output_schema=None)
def github_add_pull_request_comment(
    pull_number: int,
    body: str,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """Add an issue-style comment on a pull request."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    pull = repo.get_pull(number=pull_number)
    comment = pull.create_issue_comment(body)
    return _compact_text(f"{comment.html_url}")


@mcp.tool(output_schema=None)
def github_list_pull_request_comments(
    pull_number: int,
    limit: int = 50,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """List issue-style comments on a pull request."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    pull = repo.get_pull(number=pull_number)
    items: list[str] = []
    for idx, comment in enumerate(pull.as_issue().get_comments()):
        if idx >= max(1, limit):
            break
        login = getattr(getattr(comment, "user", None), "login", None) or "unknown"
        created_at = _to_iso8601(getattr(comment, "created_at", None)) or "unknown-time"
        body = str(getattr(comment, "body", "") or "").strip() or "(empty)"
        url = str(getattr(comment, "html_url", "") or "").strip()
        items.append(f"- {login} @ {created_at}\n{body}\n{url}".strip())
    return _compact_text("\n\n".join(items) if items else "(no pull request comments)")


@mcp.tool(output_schema=None)
def github_list_pull_request_reviews(
    pull_number: int,
    limit: int = 50,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """List review summaries on a pull request (APPROVED/CHANGES_REQUESTED/COMMENTED)."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    pull = repo.get_pull(number=pull_number)
    items: list[str] = []
    for idx, review in enumerate(pull.get_reviews()):
        if idx >= max(1, limit):
            break
        login = getattr(getattr(review, "user", None), "login", None) or "unknown"
        state = str(getattr(review, "state", None) or "UNKNOWN").upper()
        submitted = _to_iso8601(getattr(review, "submitted_at", None)) or "unknown-time"
        body = str(getattr(review, "body", "") or "").strip() or "(empty)"
        items.append(f"- {login} [{state}] @ {submitted}\n{body}")
    return _compact_text("\n\n".join(items) if items else "(no pull request reviews)")


@mcp.tool(output_schema=None)
def github_list_pull_request_review_comments(
    pull_number: int,
    limit: int = 50,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """List inline review comments on a pull request."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    pull = repo.get_pull(number=pull_number)

    items: list[str] = []
    for comment in pull.get_comments():
        coord = f"{comment.path}:{comment.line}" if comment.line is not None else str(comment.path)
        body_text = _compact_inline_text(comment.body or "")
        items.append(f"{coord} {body_text}\n{comment.html_url}".strip())
        if len(items) >= max(1, limit):
            break
    return _compact_text("\n\n".join(items) if items else "(no review comments)")


@mcp.tool(output_schema=None)
def github_add_pull_request_review_comment(
    pull_number: int,
    commit_id: str,
    path: str,
    line: int,
    body: str,
    side: str = "RIGHT",
    start_line: int | None = None,
    start_side: str | None = None,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """Add an inline code review comment on a pull request."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    pull = repo.get_pull(number=pull_number)

    kwargs: dict[str, Any] = {
        "body": body,
        "commit": commit_id,
        "path": path,
        "line": line,
        "side": side,
    }
    if start_line is not None:
        kwargs["start_line"] = start_line
    if start_side is not None:
        kwargs["start_side"] = start_side

    comment = pull.create_review_comment(**kwargs)
    return _compact_text(f"{comment.html_url}")


def main() -> None:
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
