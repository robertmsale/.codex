from __future__ import annotations

import argparse
import os
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path


class GitOpsOpsError(RuntimeError):
    pass


@dataclass(frozen=True)
class GitResult:
    stdout: str
    stderr: str
    returncode: int


def _run_git(cwd: Path, args: list[str], check: bool = True) -> GitResult:
    process = subprocess.run(
        ["git", *args],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    result = GitResult(
        stdout=(process.stdout or "").strip(),
        stderr=(process.stderr or "").strip(),
        returncode=process.returncode,
    )
    if check and result.returncode != 0:
        detail = result.stderr or result.stdout or "unknown git error"
        raise GitOpsOpsError(f"git {' '.join(args)} failed: {detail}")
    return result


def _run_command(args: list[str], cwd: Path, check: bool = True) -> GitResult:
    process = subprocess.run(
        args,
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    result = GitResult(
        stdout=(process.stdout or "").strip(),
        stderr=(process.stderr or "").strip(),
        returncode=process.returncode,
    )
    if check and result.returncode != 0:
        detail = result.stderr or result.stdout or "unknown command error"
        raise GitOpsOpsError(f"{' '.join(args)} failed: {detail}")
    return result


def _resolve_repo_root(repo_path: str | None = None) -> Path:
    start = Path(repo_path).expanduser() if repo_path else Path.cwd()
    if not start.exists():
        raise GitOpsOpsError(f"Path does not exist: {start}")
    cwd = start if start.is_dir() else start.parent
    result = _run_git(cwd, ["rev-parse", "--show-toplevel"], check=False)
    if result.returncode != 0 or not result.stdout:
        raise GitOpsOpsError(f"Not inside a git repository: {cwd}")
    return Path(result.stdout).expanduser().resolve()


def _resolve_repo_and_git_cwd(repo_path: str | None = None) -> tuple[Path, Path]:
    start = Path(repo_path).expanduser() if repo_path else Path.cwd()
    if not start.exists():
        raise GitOpsOpsError(f"Path does not exist: {start}")
    git_cwd = start.resolve()
    if git_cwd.is_file():
        git_cwd = git_cwd.parent
    repo_root = _resolve_repo_root(str(git_cwd))
    try:
        git_cwd.relative_to(repo_root)
    except ValueError:
        git_cwd = repo_root
    return repo_root, git_cwd


def _parse_bool_env(key: str, default: bool = False) -> bool:
    raw = os.getenv(key)
    if raw is None:
        return default
    normalized = raw.strip().lower()
    return normalized not in {"", "0", "false", "no", "off"}


def _protected_branches() -> set[str]:
    raw = os.getenv("REQUEST_REVIEW_INTEGRATION_BRANCHES", "main master staging prod production")
    return {item.strip() for item in re.split(r"[\s,]+", raw) if item.strip()}


def _branch_name_at(git_cwd: Path) -> str:
    return _run_git(git_cwd, ["rev-parse", "--abbrev-ref", "HEAD"]).stdout


def _ensure_branch_is_mutable_at(git_cwd: Path) -> str:
    branch = _branch_name_at(git_cwd)
    if branch in _protected_branches():
        raise GitOpsOpsError(
            "Command Rejected: You seem to be working in a restricted integration branch. "
            "Please move your file changes into a worktree and notify the user/orchestrator that the integration branch is dirty."
        )
    return branch


def _slug(text: str) -> str:
    value = text.strip().lower()
    value = re.sub(r"[^a-z0-9._-]+", "-", value)
    value = re.sub(r"-+", "-", value).strip("-")
    return value or "worktree"


def _worktrees_root(repo_root: Path) -> Path:
    relative = os.getenv("GITOPS_WORKTREE_DIR", ".worktrees").strip() or ".worktrees"
    root = (repo_root / relative).resolve()
    try:
        root.relative_to(repo_root.resolve())
    except ValueError as exc:
        raise GitOpsOpsError(
            f"GITOPS_WORKTREE_DIR must resolve under repo root. repo={repo_root} configured={relative} resolved={root}"
        ) from exc
    return root


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


def _base_repo_root_from_entries(repo_probe: Path, entries: list[dict[str, str]]) -> Path:
    for entry in entries:
        value = entry.get("worktree")
        if not value:
            continue
        entry_path = Path(value).expanduser().resolve()
        if (entry_path / ".git").is_dir():
            return entry_path
    return repo_probe


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
        raise GitOpsOpsError("No worktrees found for repository.")

    requested_path = Path(worktree_path).expanduser().resolve(strict=False) if worktree_path else None
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
        raise GitOpsOpsError(f"Worktree path is not registered: {requested_path}")
    if requested_branch is not None:
        raise GitOpsOpsError(f"Branch is not attached to any worktree: {requested_branch}")
    raise GitOpsOpsError("Unable to resolve worktree target.")


def _infer_repo_root_from_worktree_path(worktree_path: Path) -> Path:
    if worktree_path.exists():
        return _resolve_repo_root(str(worktree_path))

    configured = os.getenv("GITOPS_WORKTREE_DIR", ".worktrees").strip() or ".worktrees"
    component = Path(configured).name
    for candidate in [worktree_path, *worktree_path.parents]:
        if candidate.name != component:
            continue
        possible_repo = candidate.parent
        if not possible_repo.exists():
            continue
        try:
            return _resolve_repo_root(str(possible_repo))
        except GitOpsOpsError:
            continue
    raise GitOpsOpsError(
        f"Unable to infer repository root for missing worktree path: {worktree_path}. "
        "Expected path under repo/.worktrees/<name>."
    )


def _has_staged_changes(git_cwd: Path) -> bool:
    process = subprocess.run(
        ["git", "diff", "--cached", "--quiet"],
        cwd=git_cwd,
        capture_output=True,
        text=True,
    )
    return process.returncode == 1


def _integration_branch_name() -> str:
    value = (os.getenv("GITOPS_INTEGRATION_BRANCH", "integration") or "").strip()
    return value or "integration"


def git_worktree_create_op(
    branch_name: str,
    worktree_name: str | None = None,
    base_branch: str | None = None,
    repo_path: str | None = None,
) -> str:
    if not branch_name.strip():
        raise GitOpsOpsError("branch_name is required.")
    repo_root = _resolve_repo_root(repo_path)
    base = (base_branch or _integration_branch_name()).strip()
    if not base:
        raise GitOpsOpsError("base_branch resolved to an empty string")

    target_parent = _worktrees_root(repo_root)
    target_parent.mkdir(parents=True, exist_ok=True)
    name = worktree_name.strip() if worktree_name else _slug(branch_name.replace("/", "-"))
    target_dir = target_parent / name
    if target_dir.exists():
        raise GitOpsOpsError(f"Worktree target already exists: {target_dir}")

    _run_git(repo_root, ["fetch", "--quiet", "origin", base], check=False)

    branch_exists = (
        _run_git(repo_root, ["show-ref", "--verify", "--quiet", f"refs/heads/{branch_name}"], check=False).returncode
        == 0
    )
    if branch_exists:
        _run_git(repo_root, ["worktree", "add", "--quiet", str(target_dir), branch_name], check=True)
        start_point = branch_name
    else:
        local_base_exists = _run_git(
            repo_root,
            ["show-ref", "--verify", "--quiet", f"refs/heads/{base}"],
            check=False,
        ).returncode == 0
        start_point = base if local_base_exists else f"origin/{base}"
        _run_git(
            repo_root,
            ["worktree", "add", "--quiet", "-b", branch_name, str(target_dir), start_point],
            check=True,
        )

    return f"Created worktree: {target_dir}\nbranch: {branch_name}\nbase: {start_point}"


def git_worktree_cleanup_op(worktree_path: str) -> str:
    if not worktree_path.strip():
        raise GitOpsOpsError("worktree_path is required.")
    target_path = Path(worktree_path).expanduser().resolve(strict=False)
    normalized_target = str(target_path)
    repo_probe = _infer_repo_root_from_worktree_path(target_path)

    entries = _worktree_entries(repo_probe)
    registered_entry: dict[str, str] | None = None
    base_repo_root: Path | None = None
    for entry in entries:
        value = entry.get("worktree")
        if not value:
            continue
        entry_path = Path(value).expanduser().resolve()
        if (entry_path / ".git").is_dir() and base_repo_root is None:
            base_repo_root = entry_path
        if str(entry_path) == normalized_target:
            registered_entry = entry

    repo_root = (base_repo_root or repo_probe).resolve()
    if target_path == repo_root:
        raise GitOpsOpsError("Refusing to remove repository root worktree.")
    if target_path.exists() and registered_entry is None:
        raise GitOpsOpsError(f"Worktree path is not registered: {target_path}")

    lines: list[str] = []
    if target_path.exists():
        rm_result = _run_command(["rm", "-rf", "--", str(target_path)], cwd=repo_root, check=False)
        if rm_result.returncode != 0:
            detail = rm_result.stderr or rm_result.stdout or "unknown error"
            raise GitOpsOpsError(f"Failed to remove worktree directory '{target_path}': {detail}")
        lines.append(f"Removed worktree directory: {target_path}")
    else:
        lines.append(f"Worktree directory already missing: {target_path}")

    prune_result = _run_git(repo_root, ["worktree", "prune"], check=False)
    if prune_result.returncode != 0:
        detail = prune_result.stderr or prune_result.stdout or "unknown error"
        raise GitOpsOpsError(f"git worktree prune failed after cleanup of '{target_path}': {detail}")
    lines.append("Pruned stale worktree metadata.")

    if registered_entry is not None:
        remaining_entries = _worktree_entries(repo_root)
        for entry in remaining_entries:
            value = entry.get("worktree")
            if not value:
                continue
            if str(Path(value).expanduser().resolve()) == normalized_target:
                raise GitOpsOpsError(
                    f"Worktree directory was removed but remains registered after prune: {target_path}"
                )

    return "\n".join(lines)


def git_publish_worktree_op(worktree_path: str) -> str:
    if not worktree_path.strip():
        raise GitOpsOpsError("worktree_path is required.")
    repo_probe = _infer_repo_root_from_worktree_path(Path(worktree_path).expanduser().resolve(strict=False))
    repo_root = _base_repo_root_from_entries(repo_probe, _worktree_entries(repo_probe))
    target_path, _ = _resolve_worktree_target(repo_root, worktree_path=worktree_path, branch_name=None)
    if target_path == repo_root.resolve():
        raise GitOpsOpsError("Refusing operation on repository root; provide a linked worktree path.")
    branch = _ensure_branch_is_mutable_at(target_path)

    push_result = _run_git(
        target_path,
        ["push", "--force-with-lease", "-u", "origin", "HEAD"],
        check=False,
    )
    if push_result.returncode != 0:
        detail = push_result.stderr or push_result.stdout or "unknown push error"
        raise GitOpsOpsError(
            f"git push --force-with-lease -u origin HEAD failed on branch '{branch}': {detail}"
        )

    upstream_result = _run_git(
        target_path,
        ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
        check=False,
    )
    upstream = upstream_result.stdout.strip() if upstream_result.returncode == 0 else f"origin/{branch}"
    return f"Published {branch} -> {upstream}"


def git_sync_worktree_op(
    worktree_path: str,
    upstream: str | None = None,
    remote: str = "origin",
    fetch_first: bool = True,
    autostash: bool = True,
) -> str:
    if not worktree_path.strip():
        raise GitOpsOpsError("worktree_path is required.")
    repo_probe = _infer_repo_root_from_worktree_path(Path(worktree_path).expanduser().resolve(strict=False))
    repo_root = _base_repo_root_from_entries(repo_probe, _worktree_entries(repo_probe))
    target_path, _ = _resolve_worktree_target(repo_root, worktree_path=worktree_path, branch_name=None)
    if target_path == repo_root.resolve():
        raise GitOpsOpsError("Refusing operation on repository root; provide a linked worktree path.")

    branch = _ensure_branch_is_mutable_at(target_path)
    normalized_remote = (remote or "").strip() or "origin"
    normalized_upstream = (upstream or "").strip() or f"{normalized_remote}/{_integration_branch_name()}"

    if fetch_first:
        fetch_result = _run_git(target_path, ["fetch", "--quiet", "--prune", normalized_remote], check=False)
        if fetch_result.returncode != 0:
            detail = fetch_result.stderr or fetch_result.stdout or "unknown fetch error"
            raise GitOpsOpsError(f"git fetch --prune {normalized_remote} failed: {detail}")

    args = ["rebase"]
    if autostash:
        args.append("--autostash")
    args.append(normalized_upstream)
    result = _run_git(target_path, args, check=False)
    if result.returncode != 0:
        detail = result.stderr or result.stdout or "unknown rebase error"
        raise GitOpsOpsError(f"git {' '.join(args)} failed on branch '{branch}': {detail}")
    return f"Rebased {branch} onto {normalized_upstream}"


def git_commit_op(
    message: str,
    repo_path: str | None = None,
    add_all: bool = True,
    allow_empty: bool = False,
) -> str:
    if not message.strip():
        raise GitOpsOpsError("Commit message cannot be empty.")
    _, git_cwd = _resolve_repo_and_git_cwd(repo_path)
    _ensure_branch_is_mutable_at(git_cwd)
    if add_all:
        _run_git(git_cwd, ["add", "-A"], check=True)
    if not allow_empty and not _has_staged_changes(git_cwd):
        raise GitOpsOpsError("No staged changes to commit.")

    args = ["commit", "-m", message]
    if allow_empty:
        args.append("--allow-empty")
    _run_git(git_cwd, args, check=True)
    sha = _run_git(git_cwd, ["rev-parse", "--short", "HEAD"]).stdout
    subject = _run_git(git_cwd, ["show", "-s", "--format=%s", "HEAD"]).stdout
    return f"{sha} {subject}".strip()


def git_fetch_op(
    repo_path: str | None = None,
    remote: str = "origin",
    refspec: str | None = None,
    prune: bool = False,
    tags: bool = False,
) -> str:
    _, git_cwd = _resolve_repo_and_git_cwd(repo_path)
    normalized_remote = (remote or "").strip() or "origin"
    normalized_refspec = (refspec or "").strip()

    args = ["fetch", "--quiet"]
    if prune:
        args.append("--prune")
    if tags:
        args.append("--tags")
    args.append(normalized_remote)
    if normalized_refspec:
        args.append(normalized_refspec)
    result = _run_git(git_cwd, args, check=False)
    if result.returncode != 0:
        detail = result.stderr or result.stdout or "unknown fetch error"
        raise GitOpsOpsError(f"git {' '.join(args)} failed: {detail}")
    return f"Fetched {normalized_remote}" + (f" {normalized_refspec}" if normalized_refspec else "")


def git_rebase_op(
    repo_path: str | None = None,
    upstream: str | None = None,
    remote: str = "origin",
    fetch_first: bool = True,
    autostash: bool = True,
) -> str:
    _, git_cwd = _resolve_repo_and_git_cwd(repo_path)
    branch = _ensure_branch_is_mutable_at(git_cwd)
    normalized_remote = (remote or "").strip() or "origin"
    normalized_upstream = (upstream or "").strip() or f"{normalized_remote}/{_integration_branch_name()}"

    if fetch_first:
        fetch_result = _run_git(git_cwd, ["fetch", "--quiet", "--prune", normalized_remote], check=False)
        if fetch_result.returncode != 0:
            detail = fetch_result.stderr or fetch_result.stdout or "unknown fetch error"
            raise GitOpsOpsError(f"git fetch --prune {normalized_remote} failed: {detail}")

    args = ["rebase"]
    if autostash:
        args.append("--autostash")
    args.append(normalized_upstream)
    result = _run_git(git_cwd, args, check=False)
    if result.returncode != 0:
        detail = result.stderr or result.stdout or "unknown rebase error"
        raise GitOpsOpsError(f"git {' '.join(args)} failed on branch '{branch}': {detail}")
    return f"Rebased {branch} onto {normalized_upstream}"


def _cli() -> int:
    parser = argparse.ArgumentParser(description="Run gitops workflow operations outside MCP")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_create = sub.add_parser("worktree-create")
    p_create.add_argument("--repo-path")
    p_create.add_argument("--base-branch")
    p_create.add_argument("--worktree-name")
    p_create.add_argument("--branch-name", required=True)

    p_cleanup = sub.add_parser("worktree-cleanup")
    p_cleanup.add_argument("--worktree-path", required=True)

    p_publish = sub.add_parser("worktree-publish")
    p_publish.add_argument("--worktree-path", required=True)

    p_sync = sub.add_parser("worktree-sync")
    p_sync.add_argument("--worktree-path", required=True)
    p_sync.add_argument("--upstream")
    p_sync.add_argument("--remote", default="origin")
    p_sync.add_argument("--no-fetch-first", action="store_true")
    p_sync.add_argument("--no-autostash", action="store_true")

    p_commit = sub.add_parser("commit")
    p_commit.add_argument("--repo-path")
    p_commit.add_argument("--message", required=True)
    p_commit.add_argument("--no-add-all", action="store_true")
    p_commit.add_argument("--allow-empty", action="store_true")

    p_fetch = sub.add_parser("fetch")
    p_fetch.add_argument("--repo-path")
    p_fetch.add_argument("--remote", default="origin")
    p_fetch.add_argument("--refspec")
    p_fetch.add_argument("--prune", action="store_true")
    p_fetch.add_argument("--tags", action="store_true")

    p_rebase = sub.add_parser("rebase")
    p_rebase.add_argument("--repo-path")
    p_rebase.add_argument("--upstream")
    p_rebase.add_argument("--remote", default="origin")
    p_rebase.add_argument("--no-fetch-first", action="store_true")
    p_rebase.add_argument("--no-autostash", action="store_true")

    args = parser.parse_args()
    try:
        if args.cmd == "worktree-create":
            print(
                git_worktree_create_op(
                    branch_name=args.branch_name,
                    worktree_name=args.worktree_name,
                    base_branch=args.base_branch,
                    repo_path=args.repo_path,
                )
            )
        elif args.cmd == "worktree-cleanup":
            print(git_worktree_cleanup_op(worktree_path=args.worktree_path))
        elif args.cmd == "worktree-publish":
            print(git_publish_worktree_op(worktree_path=args.worktree_path))
        elif args.cmd == "worktree-sync":
            print(
                git_sync_worktree_op(
                    worktree_path=args.worktree_path,
                    upstream=args.upstream,
                    remote=args.remote,
                    fetch_first=not args.no_fetch_first,
                    autostash=not args.no_autostash,
                )
            )
        elif args.cmd == "commit":
            print(
                git_commit_op(
                    message=args.message,
                    repo_path=args.repo_path,
                    add_all=not args.no_add_all,
                    allow_empty=args.allow_empty,
                )
            )
        elif args.cmd == "fetch":
            print(
                git_fetch_op(
                    repo_path=args.repo_path,
                    remote=args.remote,
                    refspec=args.refspec,
                    prune=args.prune,
                    tags=args.tags,
                )
            )
        elif args.cmd == "rebase":
            print(
                git_rebase_op(
                    repo_path=args.repo_path,
                    upstream=args.upstream,
                    remote=args.remote,
                    fetch_first=not args.no_fetch_first,
                    autostash=not args.no_autostash,
                )
            )
        else:
            raise GitOpsOpsError(f"Unsupported command: {args.cmd}")
    except GitOpsOpsError as exc:
        print(f"ERROR: {exc}")
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(_cli())
