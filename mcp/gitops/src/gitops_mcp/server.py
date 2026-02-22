from __future__ import annotations

import json
import os
import re
import subprocess
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from dotenv import load_dotenv
from fastmcp import FastMCP
from github import Auth
from github import Github
from github.Repository import Repository

PROJECT_ROOT = Path(__file__).resolve().parents[2]
REQUEST_REVIEW_ENV_PATH = Path.home() / ".codex" / "skills" / "request-review" / ".env"


def _load_environment() -> None:
    # Local MCP env is highest priority for this server process.
    load_dotenv(PROJECT_ROOT / ".env", override=True)
    # Keep request-review knobs consistent with the existing skill defaults.
    load_dotenv(REQUEST_REVIEW_ENV_PATH, override=False)


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


def _parse_bool_env(key: str, default: bool = False) -> bool:
    raw = os.getenv(key)
    if raw is None:
        return default
    normalized = raw.strip().lower()
    return normalized not in {"", "0", "false", "no", "off"}


def _review_settings() -> ReviewSettings:
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


@mcp.tool
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
        return text
    return str(target_dir)


@mcp.tool
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
    return _run_git(repo_root, ["log", "--oneline", "-n", "1"]).stdout


@mcp.tool
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
        return str(local_payload.get("message", "")).strip() or "Local review completed."

    if settings.mode != "remote":
        raise GitOpsError(f"Unsupported review mode: {settings.mode}")

    if not using_existing_commit:
        _run_git(repo_root, ["push", "-u", "origin", "HEAD"])

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
            return "\n".join(lines)

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
            return f"approved\nPR: {pull.html_url}"

        time.sleep(settings.poll_interval_seconds)


def _issue_to_text(issue: Any) -> str:
    return f"#{issue.number} [{issue.state}] {issue.title}\n{issue.html_url}"


def _pull_to_text(pull: Any) -> str:
    draft = " draft" if pull.draft else ""
    return f"PR #{pull.number} [{pull.state}{draft}] {pull.title}\n{pull.html_url}"


@mcp.tool
def github_get_issue(issue_number: int, repo_full_name: str | None = None, repo_path: str | None = None) -> str:
    """Fetch a single GitHub issue."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    issue = repo.get_issue(number=issue_number)
    return _issue_to_text(issue)


@mcp.tool
def github_list_issues(
    state: str = "open",
    limit: int = 50,
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
    return "\n\n".join(items) if items else "(no issues)"


@mcp.tool
def github_create_issue(
    title: str,
    body: str = "",
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
        body=body,
        labels=labels or [],
        assignees=assignees or [],
    )
    return _issue_to_text(issue)


@mcp.tool
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
        title=title,
        body=body,
        state=state,
        labels=labels,
        assignees=assignees,
    )
    refreshed = repo.get_issue(number=issue_number)
    return _issue_to_text(refreshed)


@mcp.tool
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
    return f"{comment.html_url}"


@mcp.tool
def github_get_pull_request(
    pull_number: int,
    repo_full_name: str | None = None,
    repo_path: str | None = None,
) -> str:
    """Fetch a single GitHub pull request."""
    repo_root = _resolve_repo_root(repo_path)
    repo, _ = _repo(repo_root, repo_full_name)
    pull = repo.get_pull(number=pull_number)
    return _pull_to_text(pull)


@mcp.tool
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
    return f"{comment.html_url}"


@mcp.tool
def github_list_pull_request_review_comments(
    pull_number: int,
    limit: int = 100,
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
        body_text = str(comment.body or "").replace("\n", " ").strip()
        items.append(f"{coord} {body_text}\n{comment.html_url}".strip())
        if len(items) >= max(1, limit):
            break
    return "\n\n".join(items) if items else "(no review comments)"


@mcp.tool
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
    return f"{comment.html_url}"


def main() -> None:
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
