from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any, Callable

import uvicorn
from fastapi import FastAPI
from fastapi import HTTPException
from pydantic import BaseModel
from pydantic import Field

from .bridge import BridgeError
from .bridge import BridgePaths
from .bridge import current_branch
from .bridge import ensure_branch_allows_destructive_mutation
from .bridge import import_gitops_modules
from .bridge import load_paths
from .bridge import require_allowed_path
from .bridge import run_git
from .bridge import run_git_visibility
from .bridge import sanitize_for_response


class OperationRequest(BaseModel):
    args: dict[str, Any] = Field(default_factory=dict)


class OperationResponse(BaseModel):
    ok: bool = True
    operation: str
    result: Any


app = FastAPI(title="sync-gitops-http", version="0.1.0")
PATHS = load_paths()
WORKFLOW_OPS, GITOPS_SERVER = import_gitops_modules()


def _codex_home() -> Path:
    raw = os.environ.get("CODEX_HOME")
    if raw:
        return Path(raw).expanduser().resolve()
    return (Path.home() / ".codex").resolve()


def _subprocess_env() -> dict[str, str]:
    env = os.environ.copy()
    path_entries = [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        env.get("PATH", ""),
    ]
    env["PATH"] = ":".join(entry for entry in path_entries if entry)
    return env


def _host_script_env() -> dict[str, str]:
    env = _subprocess_env()
    env["PARALLELS_SYNC_GITOPS_NO_BRIDGE"] = "1"
    return env


def _run_process(
    cmd: list[str],
    *,
    cwd: Path,
    paths: BridgePaths,
    error_label: str,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    process = subprocess.run(
        cmd,
        cwd=cwd,
        capture_output=True,
        text=True,
        env=_subprocess_env(),
    )
    if check and process.returncode != 0:
        detail = (process.stderr or process.stdout or f"{error_label} failed").strip()
        raise HTTPException(status_code=400, detail=sanitize_for_response(detail, paths))
    return process


def _worktree_repo_root(worktree_path: Path, paths: BridgePaths) -> Path:
    process = _run_process(
        ["git", "-C", str(worktree_path), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        cwd=worktree_path,
        paths=paths,
        error_label="resolve git common dir",
    )
    common_dir = Path(process.stdout.strip())
    return common_dir.parent.resolve()


def _resolve_integration_branch(repo_root: Path, explicit_branch: str | None, paths: BridgePaths) -> str:
    if explicit_branch and explicit_branch.strip():
        return explicit_branch.strip()

    for cmd in (
        ["git", "-C", str(repo_root), "rev-parse", "--abbrev-ref", "HEAD"],
        ["git", "-C", str(repo_root), "symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
        ["git", "-C", str(repo_root), "remote", "show", "origin"],
    ):
        process = subprocess.run(cmd, cwd=repo_root, capture_output=True, text=True, env=_subprocess_env())
        if process.returncode != 0:
            continue
        output = (process.stdout or "").strip()
        if not output:
            continue
        if cmd[-1] == "HEAD":
            return output.removeprefix("origin/")
        if cmd[-2:] == ["show", "origin"]:
            for line in output.splitlines():
                if "HEAD branch:" in line:
                    return line.split("HEAD branch:", 1)[1].strip()
        if output != "HEAD":
            return output
    return os.environ.get("GITOPS_INTEGRATION_BRANCH", "master").strip() or "master"


def _sync_local_integration_branch(repo_root: Path, integration_branch: str, paths: BridgePaths) -> str:
    remote_ref = f"origin/{integration_branch}"
    _run_process(
        ["git", "-C", str(repo_root), "fetch", "-q", "origin", integration_branch, "--prune"],
        cwd=repo_root,
        paths=paths,
        error_label="git fetch integration branch",
    )

    remote_sha = _run_process(
        ["git", "-C", str(repo_root), "rev-parse", remote_ref],
        cwd=repo_root,
        paths=paths,
        error_label="git rev-parse remote integration branch",
    ).stdout.strip()

    current = subprocess.run(
        ["git", "-C", str(repo_root), "rev-parse", "--abbrev-ref", "HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        env=_subprocess_env(),
    ).stdout.strip()

    local_branch_exists = subprocess.run(
        ["git", "-C", str(repo_root), "show-ref", "--verify", "--quiet", f"refs/heads/{integration_branch}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        env=_subprocess_env(),
    ).returncode == 0

    if current == integration_branch:
        had_stash = False
        stash_name = f"git-merge-sync-{integration_branch}-{int(time.time())}"
        status = _run_process(
            ["git", "-C", str(repo_root), "status", "--short"],
            cwd=repo_root,
            paths=paths,
            error_label="git status",
        ).stdout.strip()
        if status:
            _run_process(
                ["git", "-C", str(repo_root), "stash", "push", "-u", "-m", stash_name],
                cwd=repo_root,
                paths=paths,
                error_label="git stash push",
            )
            had_stash = True

        _run_process(
            ["git", "-C", str(repo_root), "merge", "--ff-only", remote_ref],
            cwd=repo_root,
            paths=paths,
            error_label="git merge --ff-only integration branch",
        )

        if had_stash:
            stash_pop = subprocess.run(
                ["git", "-C", str(repo_root), "stash", "pop"],
                cwd=repo_root,
                capture_output=True,
                text=True,
                env=_subprocess_env(),
            )
            if stash_pop.returncode != 0:
                raise HTTPException(
                    status_code=400,
                    detail=sanitize_for_response(
                        f"Local {integration_branch} was fast-forwarded, but stash restoration failed after sync.",
                        paths,
                    ),
                )
        return f"Fast-forwarded checked-out {integration_branch} to {remote_ref}"

    if local_branch_exists:
        local_sha = _run_process(
            ["git", "-C", str(repo_root), "rev-parse", integration_branch],
            cwd=repo_root,
            paths=paths,
            error_label="git rev-parse local integration branch",
        ).stdout.strip()
        merge_base = _run_process(
            ["git", "-C", str(repo_root), "merge-base", integration_branch, remote_ref],
            cwd=repo_root,
            paths=paths,
            error_label="git merge-base integration branch",
        ).stdout.strip()
        if merge_base != local_sha:
            raise HTTPException(
                status_code=400,
                detail=sanitize_for_response(
                    f"Local {integration_branch} is not an ancestor of {remote_ref}; refusing non-fast-forward update.",
                    paths,
                ),
            )

    _run_process(
        ["git", "-C", str(repo_root), "update-ref", f"refs/heads/{integration_branch}", remote_sha],
        cwd=repo_root,
        paths=paths,
        error_label="git update-ref integration branch",
    )
    return f"Updated local {integration_branch} to {remote_ref}"


def _request_review_disable_flag() -> bool:
    env_path = _codex_home() / "skills" / "request-review" / ".env"
    if not env_path.exists():
        return False
    for raw_line in env_path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        if key.strip() != "REQUEST_REVIEW_DISABLE":
            continue
        normalized = value.strip().strip("'\"")
        return normalized not in {"", "0", "false", "False", "FALSE"}
    return False


def _map_repo_arg(args: dict[str, Any], key: str, paths: BridgePaths) -> dict[str, Any]:
    mapped = dict(args)
    value = mapped.get(key)
    if value is None and key == "repo_path" and mapped.get("worktree_path") is not None:
        value = mapped.get("worktree_path")
        mapped["repo_path"] = value
        mapped.pop("worktree_path", None)
    if value is not None:
        mapped[key] = str(require_allowed_path(str(value), paths))
    return mapped


def _map_common_args(args: dict[str, Any], paths: BridgePaths) -> dict[str, Any]:
    mapped = dict(args)
    for key in ("repo_path", "worktree_path"):
        if key in mapped and mapped[key] is not None:
            mapped[key] = str(require_allowed_path(str(mapped[key]), paths))
    return mapped


def _require_local_git_repo(repo_path: Path) -> Path:
    probe = subprocess.run(
        ["git", "-C", str(repo_path), "rev-parse", "--git-dir"],
        capture_output=True,
        text=True,
        env=_subprocess_env(),
    )
    if probe.returncode != 0:
        detail = (probe.stderr or probe.stdout or "").strip() or f"Path is not a valid local git repository: {repo_path}"
        raise HTTPException(status_code=400, detail=detail)
    return repo_path


def _call(fn: Callable[..., Any], args: dict[str, Any], paths: BridgePaths) -> Any:
    try:
        return sanitize_for_response(fn(**args), paths)
    except (BridgeError, WORKFLOW_OPS.GitOpsOpsError, GITOPS_SERVER.GitOpsError) as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc


def _visibility_status(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = _require_local_git_repo(require_allowed_path(str(args.get("repo_path") or ""), paths))
    return sanitize_for_response(run_git_visibility(["status", "--short", "--branch"], repo_path), paths)


def _visibility_worktree_list(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = _require_local_git_repo(require_allowed_path(str(args.get("repo_path") or ""), paths))
    return sanitize_for_response(run_git_visibility(["worktree", "list", "--porcelain"], repo_path), paths)


def _visibility_branch(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = _require_local_git_repo(require_allowed_path(str(args.get("repo_path") or ""), paths))
    return sanitize_for_response(run_git_visibility(["branch", "--show-current"], repo_path), paths)


def _visibility_log(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = _require_local_git_repo(require_allowed_path(str(args.get("repo_path") or ""), paths))
    limit = max(1, int(args.get("limit", 20)))
    return sanitize_for_response(run_git_visibility(["log", "--oneline", f"-n{limit}"], repo_path), paths)


def _visibility_diff_stat(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = _require_local_git_repo(require_allowed_path(str(args.get("repo_path") or ""), paths))
    return sanitize_for_response(run_git_visibility(["diff", "--stat"], repo_path), paths)


def _visibility_diff(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = _require_local_git_repo(require_allowed_path(str(args.get("repo_path") or ""), paths))
    ref = str(args.get("ref") or "").strip()
    pathspec = str(args.get("pathspec") or "").strip()
    git_args = ["diff"]
    if ref:
        git_args.append(ref)
    if pathspec:
        git_args.extend(["--", pathspec])
    return sanitize_for_response(run_git_visibility(git_args, repo_path), paths)


def _visibility_show(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = _require_local_git_repo(require_allowed_path(str(args.get("repo_path") or ""), paths))
    obj = str(args.get("object") or "").strip()
    if not obj:
        raise HTTPException(status_code=400, detail="object is required")
    return sanitize_for_response(run_git_visibility(["show", "--stat", obj], repo_path), paths)


def _visibility_rev_parse(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = _require_local_git_repo(require_allowed_path(str(args.get("repo_path") or ""), paths))
    ref = str(args.get("ref") or "HEAD").strip()
    return sanitize_for_response(run_git_visibility(["rev-parse", "--verify", ref], repo_path), paths)


def _visibility_branch_list(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = _require_local_git_repo(require_allowed_path(str(args.get("repo_path") or ""), paths))
    all_branches = bool(args.get("all", True))
    git_args = ["branch"]
    if all_branches:
        git_args.append("--all")
    git_args.extend(["--verbose", "--verbose"])
    return sanitize_for_response(run_git_visibility(git_args, repo_path), paths)


def _visibility_merge_base(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = _require_local_git_repo(require_allowed_path(str(args.get("repo_path") or ""), paths))
    left = str(args.get("left") or "").strip()
    right = str(args.get("right") or "").strip()
    if not left or not right:
        raise HTTPException(status_code=400, detail="left and right are required")
    return sanitize_for_response(run_git_visibility(["merge-base", left, right], repo_path), paths)


def _mutating_stage_paths(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = require_allowed_path(str(args.get("repo_path") or ""), paths)
    items = args.get("paths")
    if not isinstance(items, list) or not items:
        raise HTTPException(status_code=400, detail="paths must be a non-empty list")
    git_args = ["add", "--"]
    git_args.extend(str(item) for item in items)
    return sanitize_for_response(run_git(git_args, repo_path), paths)


def _mutating_unstage_paths(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = require_allowed_path(str(args.get("repo_path") or ""), paths)
    ensure_branch_allows_destructive_mutation(repo_path)
    items = args.get("paths")
    if not isinstance(items, list) or not items:
        raise HTTPException(status_code=400, detail="paths must be a non-empty list")
    git_args = ["reset", "HEAD", "--"]
    git_args.extend(str(item) for item in items)
    return sanitize_for_response(run_git(git_args, repo_path), paths)


def _mutating_rebase_abort(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = require_allowed_path(str(args.get("repo_path") or ""), paths)
    current_branch(repo_path)
    return sanitize_for_response(run_git(["rebase", "--abort"], repo_path), paths)


def _mutating_rebase_continue(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = require_allowed_path(str(args.get("repo_path") or ""), paths)
    return _call(WORKFLOW_OPS.git_rebase_continue_op, {"repo_path": str(repo_path)}, paths)


def _request_host_review(args: dict[str, Any], paths: BridgePaths) -> dict[str, Any]:
    repo_path = require_allowed_path(str(args.get("repo_path") or ""), paths)
    profile = str(args.get("profile") or "local-review").strip() or "local-review"
    title = str(args.get("title") or "").strip()
    prompt = str(args.get("prompt") or "").strip()
    uncommitted = bool(args.get("uncommitted", False))
    commit_ref = str(args.get("commit_ref") or "").strip()

    if not uncommitted and not commit_ref:
        raise HTTPException(status_code=400, detail="Either uncommitted=true or commit_ref is required")

    if _request_review_disable_flag():
        message = "all clear!"
        (repo_path / "review.log").write_text(f"{message}\n", encoding="utf-8")
        return sanitize_for_response(
            {
                "exit_code": 0,
                "message": message,
                "stdout": message,
                "stderr": "",
            },
            paths,
        )

    codex_bin = shutil.which("codex") or "/opt/homebrew/bin/codex"
    if not Path(codex_bin).exists():
        raise HTTPException(status_code=500, detail=f"codex binary not found: {codex_bin}")

    resolved_commit = ""
    if not uncommitted:
        try:
            resolved_commit = run_git_visibility(["rev-parse", "--verify", f"{commit_ref}^{{commit}}"], repo_path)
        except BridgeError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

    with tempfile.NamedTemporaryFile(prefix="sync-review.", suffix=".txt", delete=False) as handle:
        output_path = Path(handle.name)

    try:
        cmd = [
            codex_bin,
            "exec",
            "-C",
            str(repo_path),
            "-s",
            "read-only",
            "-p",
            profile,
            "review",
            "--output-last-message",
            str(output_path),
        ]
        if title:
            cmd.extend(["--title", title])
        if uncommitted:
            cmd.append("--uncommitted")
        else:
            cmd.extend(["--commit", resolved_commit])
        if prompt:
            cmd.append(prompt)

        process = subprocess.run(cmd, capture_output=True, text=True, env=_subprocess_env(), cwd=repo_path)
        message = output_path.read_text(encoding="utf-8", errors="replace").strip() if output_path.exists() else ""
        if not message:
            message = (process.stdout or process.stderr or "").strip()

        if process.returncode == 0 and message:
            (repo_path / "review.log").write_text(f"{message.rstrip()}\n", encoding="utf-8")

        return sanitize_for_response(
            {
                "exit_code": process.returncode,
                "message": message,
                "stdout": (process.stdout or "").strip(),
                "stderr": (process.stderr or "").strip(),
            },
            paths,
        )
    finally:
        output_path.unlink(missing_ok=True)


def _git_merge_worktree(args: dict[str, Any], paths: BridgePaths) -> str:
    worktree_path = require_allowed_path(str(args.get("worktree_path") or ""), paths)
    integration_branch = str(args.get("integration_branch") or "").strip()
    repo_root = _worktree_repo_root(worktree_path, paths)
    resolved_integration_branch = _resolve_integration_branch(repo_root, integration_branch, paths)
    branch = ensure_branch_allows_destructive_mutation(worktree_path)
    completed_steps: list[str] = []
    status = _run_process(
        ["git", "-C", str(worktree_path), "status", "--short"],
        cwd=worktree_path,
        paths=paths,
        error_label="git status",
    ).stdout.strip()
    if status:
        raise HTTPException(status_code=400, detail=f"Refusing to merge a dirty worktree: {worktree_path}")

    pr_number = _run_process(
        ["gh", "pr", "view", "--json", "number", "--jq", ".number"],
        cwd=worktree_path,
        paths=paths,
        error_label="gh pr view",
    ).stdout.strip()
    if not pr_number:
        raise HTTPException(status_code=400, detail=f"No PR found for worktree branch: {branch}")

    _run_process(
        ["gh", "pr", "merge", pr_number, "--squash"],
        cwd=worktree_path,
        paths=paths,
        error_label="gh pr merge",
    )
    completed_steps.append(f"squash-merged PR #{pr_number}")
    try:
        ls_remote = subprocess.run(
            ["git", "-C", str(repo_root), "ls-remote", "--exit-code", "--heads", "origin", branch],
            cwd=repo_root,
            capture_output=True,
            text=True,
            env=_subprocess_env(),
        )
        if ls_remote.returncode == 0:
            _run_process(
                ["git", "-C", str(repo_root), "push", "-q", "origin", "--delete", branch],
                cwd=repo_root,
                paths=paths,
                error_label="delete remote branch",
            )
            completed_steps.append(f"deleted origin/{branch}")

        sync_text = _sync_local_integration_branch(repo_root, resolved_integration_branch, paths)
        completed_steps.append(sync_text)

        show_ref = subprocess.run(
            ["git", "-C", str(repo_root), "show-ref", "--verify", "--quiet", f"refs/heads/{branch}"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            env=_subprocess_env(),
        )
        if show_ref.returncode == 0:
            _run_process(
                ["git", "-C", str(repo_root), "branch", "-D", branch],
                cwd=repo_root,
                paths=paths,
                error_label="delete local branch",
            )
            completed_steps.append(f"deleted local branch {branch}")

        _run_process(
            ["git", "-C", str(repo_root), "fetch", "-q", "origin", "--prune"],
            cwd=repo_root,
            paths=paths,
            error_label="git fetch --prune",
        )
        completed_steps.append("pruned remote refs")
    except HTTPException as exc:
        detail = exc.detail if isinstance(exc.detail, str) else str(exc.detail)
        prefix = ". ".join(completed_steps)
        if prefix:
            detail = f"{prefix}. Failed during merge finalization: {detail}"
        raise HTTPException(status_code=400, detail=sanitize_for_response(detail, paths)) from exc

    cleanup_warning = ""
    try:
        cleanup_text = _call(WORKFLOW_OPS.git_worktree_cleanup_op, {"worktree_path": str(worktree_path)}, paths)
    except HTTPException as exc:
        detail = exc.detail if isinstance(exc.detail, str) else str(exc.detail)
        cleanup_warning = f" WARNING: worktree cleanup failed: {detail}"
    else:
        completed_steps.append(cleanup_text)

    return sanitize_for_response(". ".join(completed_steps) + "." + cleanup_warning, paths)


def _git_publish_worktree(args: dict[str, Any], paths: BridgePaths) -> str:
    worktree_path = require_allowed_path(str(args.get("worktree_path") or ""), paths)
    if not worktree_path.exists():
        raise HTTPException(status_code=400, detail=f"Worktree path does not exist: {worktree_path}")

    integration_branch = str(
        args.get("integration_branch")
        or args.get("base_branch")
        or ""
    ).strip()
    _ = integration_branch
    return _call(WORKFLOW_OPS.git_publish_worktree_op, {"worktree_path": str(worktree_path)}, paths)


def _git_worktree_create(args: dict[str, Any], paths: BridgePaths) -> str:
    repo_path = require_allowed_path(str(args.get("repo_path") or ""), paths)
    base_branch = str(args.get("base_branch") or "").strip()
    branch_name = str(args.get("branch_name") or "").strip()
    worktree_name = str(args.get("worktree_name") or "").strip()
    if not repo_path or not base_branch or not branch_name or not worktree_name:
        raise HTTPException(status_code=400, detail="repo_path, base_branch, branch_name, and worktree_name are required")

    return _call(
        WORKFLOW_OPS.git_worktree_create_op,
        {
            "repo_path": str(repo_path),
            "base_branch": base_branch,
            "branch_name": branch_name,
            "worktree_name": worktree_name,
        },
        paths,
    )


def _git_worktree_cleanup(args: dict[str, Any], paths: BridgePaths) -> str:
    worktree_path = require_allowed_path(str(args.get("worktree_path") or ""), paths)
    _ = str(args.get("integration_branch") or "").strip()
    return _call(WORKFLOW_OPS.git_worktree_cleanup_op, {"worktree_path": str(worktree_path)}, paths)


def _git_sync_worktree(args: dict[str, Any], paths: BridgePaths) -> str:
    worktree_path = require_allowed_path(str(args.get("worktree_path") or ""), paths)
    upstream = str(args.get("upstream") or "").strip()
    return _call(
        WORKFLOW_OPS.git_sync_worktree_op,
        {"worktree_path": str(worktree_path), "upstream": upstream or None},
        paths,
    )


def _qa_fastforward(args: dict[str, Any], paths: BridgePaths) -> str:
    worktree_path = require_allowed_path(str(args.get("worktree_path") or ""), paths)
    integration_branch = str(args.get("integration_branch") or "").strip()
    repo_root = _worktree_repo_root(worktree_path, paths)
    branch = current_branch(worktree_path)
    target_branch = _resolve_integration_branch(repo_root, integration_branch, paths)
    stash_name = f"qa-fastforward-{branch}-{int(time.time())}"

    status = _run_process(
        ["git", "-C", str(worktree_path), "status", "--short"],
        cwd=worktree_path,
        paths=paths,
        error_label="git status",
    ).stdout.strip()
    if status:
        _run_process(
            ["git", "-C", str(worktree_path), "stash", "push", "-u", "-m", stash_name],
            cwd=worktree_path,
            paths=paths,
            error_label="git stash push",
        )
    else:
        stash_name = ""

    _run_process(
        ["git", "-C", str(worktree_path), "fetch", "-q", "origin", "--prune"],
        cwd=worktree_path,
        paths=paths,
        error_label="git fetch --prune",
    )
    local_head = _run_process(
        ["git", "-C", str(worktree_path), "rev-parse", "HEAD"],
        cwd=worktree_path,
        paths=paths,
        error_label="git rev-parse HEAD",
    ).stdout.strip()
    remote_head = _run_process(
        ["git", "-C", str(worktree_path), "rev-parse", f"origin/{target_branch}"],
        cwd=worktree_path,
        paths=paths,
        error_label="git rev-parse origin head",
    ).stdout.strip()

    if local_head != remote_head:
        if branch in {"main", "master", "staging", "prod", "production"}:
            _run_process(
                ["git", "-C", str(worktree_path), "merge", "--ff-only", f"origin/{target_branch}"],
                cwd=worktree_path,
                paths=paths,
                error_label="git merge --ff-only",
            )
        else:
            _run_process(
                ["git", "-C", str(worktree_path), "rebase", f"origin/{target_branch}"],
                cwd=worktree_path,
                paths=paths,
                error_label="git rebase",
            )

    stash_preserved = False
    if stash_name:
        stash_list = _run_process(
            ["git", "-C", str(repo_root), "stash", "list", "--format=%gd %s"],
            cwd=repo_root,
            paths=paths,
            error_label="git stash list",
        ).stdout.splitlines()
        stash_ref = ""
        for line in stash_list:
            if stash_name in line:
                stash_ref = line.split(" ", 1)[0]
                break
        if stash_ref:
            apply_process = subprocess.run(
                ["git", "-C", str(repo_root), "stash", "apply", stash_ref],
                cwd=repo_root,
                capture_output=True,
                text=True,
                env=_subprocess_env(),
            )
            if apply_process.returncode == 0:
                _run_process(
                    ["git", "-C", str(repo_root), "stash", "drop", stash_ref],
                    cwd=repo_root,
                    paths=paths,
                    error_label="git stash drop",
                )
            else:
                stash_preserved = True
        else:
            stash_preserved = True

    if stash_name:
        if stash_preserved:
            return sanitize_for_response(
                f"Fast-forwarded {branch} onto origin/{target_branch}; stash {stash_name} was preserved for manual recovery",
                paths,
            )
        return sanitize_for_response(
            f"Fast-forwarded {branch} onto origin/{target_branch} and restored stash {stash_name}",
            paths,
        )
    return sanitize_for_response(f"Fast-forwarded {branch} onto origin/{target_branch}", paths)


def _git_commit(args: dict[str, Any], paths: BridgePaths) -> str:
    worktree_path = require_allowed_path(str(args.get("worktree_path") or args.get("repo_path") or ""), paths)
    message = str(args.get("message") or "").strip()
    allow_empty = bool(args.get("allow_empty", False))
    add_all = bool(args.get("add_all", True))
    if not message:
        raise HTTPException(status_code=400, detail="message is required")

    return _call(
        WORKFLOW_OPS.git_commit_op,
        {
            "repo_path": str(worktree_path),
            "message": message,
            "allow_empty": allow_empty,
            "add_all": add_all,
        },
        paths,
    )


OPERATIONS: dict[str, Callable[[dict[str, Any], BridgePaths], Any]] = {
    "git_worktree_create": _git_worktree_create,
    "git_worktree_cleanup": _git_worktree_cleanup,
    "git_merge_worktree": _git_merge_worktree,
    "git_publish_worktree": _git_publish_worktree,
    "git_sync_worktree": _git_sync_worktree,
    "qa_fastforward": _qa_fastforward,
    "git_commit": _git_commit,
    "git_fetch": lambda args, paths: _call(
        WORKFLOW_OPS.git_fetch_op,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "git_rebase": lambda args, paths: _call(
        WORKFLOW_OPS.git_rebase_op,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "git_request_review_and_wait": lambda args, paths: _call(
        GITOPS_SERVER.git_request_review_and_wait,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_get_issue": lambda args, paths: _call(
        GITOPS_SERVER.github_get_issue,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_list_issue_comments": lambda args, paths: _call(
        GITOPS_SERVER.github_list_issue_comments,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_list_issues": lambda args, paths: _call(
        GITOPS_SERVER.github_list_issues,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_create_issue": lambda args, paths: _call(
        GITOPS_SERVER.github_create_issue,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_update_issue": lambda args, paths: _call(
        GITOPS_SERVER.github_update_issue,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_add_issue_comment": lambda args, paths: _call(
        GITOPS_SERVER.github_add_issue_comment,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_get_pull_request": lambda args, paths: _call(
        GITOPS_SERVER.github_get_pull_request,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_list_pull_requests": lambda args, paths: _call(
        GITOPS_SERVER.github_list_pull_requests,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_create_pull_request": lambda args, paths: _call(
        GITOPS_SERVER.github_create_pull_request,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_update_pull_request": lambda args, paths: _call(
        GITOPS_SERVER.github_update_pull_request,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_merge_pull_request": lambda args, paths: _call(
        GITOPS_SERVER.github_merge_pull_request,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_add_pull_request_comment": lambda args, paths: _call(
        GITOPS_SERVER.github_add_pull_request_comment,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_list_pull_request_comments": lambda args, paths: _call(
        GITOPS_SERVER.github_list_pull_request_comments,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_list_pull_request_reviews": lambda args, paths: _call(
        GITOPS_SERVER.github_list_pull_request_reviews,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_list_pull_request_review_comments": lambda args, paths: _call(
        GITOPS_SERVER.github_list_pull_request_review_comments,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "github_add_pull_request_review_comment": lambda args, paths: _call(
        GITOPS_SERVER.github_add_pull_request_review_comment,
        _map_repo_arg(args, "repo_path", paths),
        paths,
    ),
    "visibility_git_status": _visibility_status,
    "visibility_git_worktree_list": _visibility_worktree_list,
    "visibility_git_branch": _visibility_branch,
    "visibility_git_log": _visibility_log,
    "visibility_git_diff_stat": _visibility_diff_stat,
    "visibility_git_diff": _visibility_diff,
    "visibility_git_show": _visibility_show,
    "visibility_git_rev_parse": _visibility_rev_parse,
    "visibility_git_branch_list": _visibility_branch_list,
    "visibility_git_merge_base": _visibility_merge_base,
    "git_stage_paths": _mutating_stage_paths,
    "git_unstage_paths": _mutating_unstage_paths,
    "git_rebase_abort": _mutating_rebase_abort,
    "git_rebase_continue": _mutating_rebase_continue,
    "request_host_review": _request_host_review,
}


@app.get("/healthz")
def healthz() -> dict[str, Any]:
    return {
        "ok": True,
        "virtual_home": str(PATHS.virtual_home),
        "allowed_roots": [str(PATHS.virtual_home / root.relative_to(PATHS.host_home)) if root.is_relative_to(PATHS.host_home) else str(root) for root in PATHS.allowed_roots],
        "operations": sorted(OPERATIONS),
    }


@app.post("/v1/ops/{operation}", response_model=OperationResponse)
def run_operation(operation: str, request: OperationRequest) -> OperationResponse:
    handler = OPERATIONS.get(operation)
    if handler is None:
        raise HTTPException(status_code=404, detail=f"Unknown operation: {operation}")
    try:
        result = handler(request.args, PATHS)
    except BridgeError as exc:
        raise HTTPException(status_code=400, detail=sanitize_for_response(str(exc), PATHS)) from exc
    except HTTPException:
        raise
    except Exception as exc:
        detail = str(exc).strip() or exc.__class__.__name__
        raise HTTPException(status_code=500, detail=sanitize_for_response(detail, PATHS)) from exc
    return OperationResponse(operation=operation, result=result)


def main() -> None:
    parser = argparse.ArgumentParser(description="Run the sync gitops HTTP bridge")
    parser.add_argument("--host", default=os.environ.get("PARALLELS_SYNC_GITOPS_BIND", "127.0.0.1"))
    parser.add_argument("--port", type=int, default=int(os.environ.get("PARALLELS_SYNC_GITOPS_PORT", "8765")))
    args = parser.parse_args()
    uvicorn.run(app, host=args.host, port=args.port, log_level="info")


if __name__ == "__main__":
    main()
