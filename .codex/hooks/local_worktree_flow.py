#!/usr/bin/env python3
"""Local-only Robdex worker/QA worktree lifecycle and integration helpers."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


PROTECTED_BRANCHES = {"main", "master", "trunk", "develop"}


def fail(message: str, code: int = 1) -> None:
    print(json.dumps({"ok": False, "error": message}, separators=(",", ":")))
    raise SystemExit(code)


def git(root: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", "-C", str(root), *args],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=check,
    )


def git_stdout(root: Path, *args: str) -> str:
    return git(root, *args).stdout.strip()


def canonical(path: Path) -> Path:
    return path.resolve(strict=True)


def canonical_parent(path: Path) -> Path:
    parent = path.parent.resolve(strict=True)
    return parent / path.name


def repo_root(payload: dict[str, Any] | None = None) -> Path:
    candidates = [
        os.environ.get("ROBDEX_PROJECT_ROOT"),
        (payload or {}).get("projectRoot"),
        ((payload or {}).get("project") or {}).get("root"),
        os.getcwd(),
    ]
    for candidate in candidates:
        if candidate:
            return canonical(Path(str(candidate)))
    return canonical(Path.cwd())


def slugify(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return slug or "agent"


def stable_slug(payload: dict[str, Any], role: str) -> str:
    agent = payload.get("agent") or {}
    name = str(agent.get("name") or payload.get("threadId") or role)
    base = slugify(name)
    return base if base.startswith(f"{role}-") else f"{role}-{base}"


def expected_paths(root: Path, payload: dict[str, Any], role: str) -> tuple[str, Path]:
    defaults = payload.get("defaults") or {}
    slug = stable_slug(payload, role)
    branch = str(defaults.get("branchName") or f"codex/{slug}")
    worktree = Path(str(defaults.get("worktreePath") or root / ".worktrees" / slug))
    return branch, worktree


def assert_codex_branch(branch: str) -> None:
    if not branch.startswith("codex/") or branch in PROTECTED_BRANCHES:
        fail(f"refusing protected or non-codex branch: {branch}")


def assert_under_worktrees(root: Path, worktree: Path) -> Path:
    base = root / ".worktrees"
    base.mkdir(parents=True, exist_ok=True)
    base_real = canonical(base)
    candidate = canonical(worktree) if worktree.exists() else canonical_parent(worktree)
    try:
        candidate.relative_to(base_real)
    except ValueError:
        fail(f"worktree path escapes {base_real}: {worktree}")
    return candidate


def branch_exists(root: Path, branch: str) -> bool:
    return git(root, "show-ref", "--verify", "--quiet", f"refs/heads/{branch}", check=False).returncode == 0


def worktree_for_branch(root: Path, branch: str) -> str | None:
    raw = git_stdout(root, "worktree", "list", "--porcelain")
    current_path: str | None = None
    for line in raw.splitlines():
        if line.startswith("worktree "):
            current_path = line.removeprefix("worktree ")
        elif line == f"branch refs/heads/{branch}" and current_path:
            return current_path
    return None


def same_repo(root: Path, worktree: Path) -> bool:
    root_common = canonical(Path(git_stdout(root, "rev-parse", "--path-format=absolute", "--git-common-dir")))
    worktree_common = canonical(Path(git_stdout(worktree, "rev-parse", "--path-format=absolute", "--git-common-dir")))
    return root_common == worktree_common


def validate_worktree(root: Path, worktree: Path, branch: str) -> None:
    assert_under_worktrees(root, worktree)
    if not worktree.is_dir():
        fail(f"worktree path is not a directory: {worktree}")
    top = canonical(Path(git_stdout(worktree, "rev-parse", "--show-toplevel")))
    if top != canonical(worktree):
        fail(f"worktree top-level mismatch: {top} != {worktree}")
    if not same_repo(root, worktree):
        fail(f"worktree belongs to a different repository: {worktree}")
    current_branch = git_stdout(worktree, "branch", "--show-current")
    if current_branch != branch:
        fail(f"worktree branch mismatch: expected {branch}, got {current_branch}")


def is_clean(worktree: Path) -> bool:
    status = git_stdout(worktree, "status", "--porcelain")
    return status == "" or status == "ok"


def main_branch(root: Path) -> str:
    if branch_exists(root, "main"):
        return "main"
    current = git_stdout(root, "branch", "--show-current")
    if current:
        return current
    return "HEAD"


def lifecycle_artifacts(root: Path, role: str, branch: str, worktree: Path, cleanup_state: str | None = None) -> dict[str, Any]:
    artifacts: dict[str, Any] = {
        "worktreePath": str(worktree),
        "branchName": branch,
        "projectRoot": str(root),
        "agentRole": role,
    }
    if cleanup_state:
        artifacts["cleanupState"] = cleanup_state
    return artifacts


def create(role: str) -> None:
    payload = json.load(sys.stdin)
    root = repo_root(payload)
    branch, worktree = expected_paths(root, payload, role)
    assert_codex_branch(branch)
    assert_under_worktrees(root, worktree)

    existing_for_branch = worktree_for_branch(root, branch) if branch_exists(root, branch) else None
    if worktree.exists():
        validate_worktree(root, worktree, branch)
    elif existing_for_branch:
        fail(f"branch {branch} is already checked out at {existing_for_branch}, not {worktree}")
    elif branch_exists(root, branch):
        fail(f"branch {branch} already exists without expected worktree {worktree}")
    else:
        git(root, "worktree", "add", "-b", branch, str(worktree), main_branch(root))
        validate_worktree(root, worktree, branch)

    prompt = (
        f"Use the assigned local {role} worktree at `{worktree}` on branch `{branch}`. "
        "Commit locally in that worktree. Requirements Review is the completion gate. "
        "Do not create GitHub pull requests or local review artifacts."
    )
    print(
        json.dumps(
            {
                "ok": True,
                "artifacts": lifecycle_artifacts(root, role, branch, worktree),
                "metadata": {"localWorktreeFlow": True},
                "promptAppend": [prompt],
            },
            separators=(",", ":"),
        )
    )


def archive(role: str) -> None:
    payload = json.load(sys.stdin)
    root = repo_root(payload)
    lifecycle = payload.get("lifecycle") or {}
    branch = lifecycle.get("branchName") or (lifecycle.get("artifacts") or {}).get("branchName")
    worktree_raw = lifecycle.get("worktreePath") or (lifecycle.get("artifacts") or {}).get("worktreePath")
    if not branch or not worktree_raw:
        fail("missing lifecycle worktreePath or branchName")
    branch = str(branch)
    worktree = Path(str(worktree_raw))
    assert_codex_branch(branch)
    assert_under_worktrees(root, worktree)

    branch_present = branch_exists(root, branch)
    if not worktree.exists() and not branch_present:
        print(json.dumps({"ok": True, "artifacts": lifecycle_artifacts(root, role, branch, worktree, "alreadyClean")}, separators=(",", ":")))
        return
    if worktree.exists():
        validate_worktree(root, worktree, branch)
        if not is_clean(worktree):
            fail(f"refusing to remove dirty worktree: {worktree}")
        git(root, "worktree", "remove", str(worktree))
    if branch_exists(root, branch):
        checked_out = worktree_for_branch(root, branch)
        if checked_out:
            fail(f"refusing to delete checked-out branch {branch} at {checked_out}")
        git(root, "branch", "-D", branch)
    print(json.dumps({"ok": True, "artifacts": lifecycle_artifacts(root, role, branch, worktree, "removed")}, separators=(",", ":")))


def requirements_clear(thread_id: str, root: Path) -> bool:
    result = subprocess.run(
        ["robdex", "requirements-status", "--to-thread-id", thread_id, "--project-path", str(root)],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        return False
    stdout = result.stdout
    if "status=passed" in stdout or "status=waiverAccepted" in stdout:
        return True
    if "active_requirements=0" in stdout:
        return True
    return False


def integrate(argv: list[str]) -> None:
    parser = argparse.ArgumentParser(description="Integrate a completed local Robdex worker branch into local main.")
    parser.add_argument("worktree")
    parser.add_argument("--thread-id", required=True)
    parser.add_argument("--root", default=os.getcwd())
    args = parser.parse_args(argv)
    root = canonical(Path(args.root))
    worktree = canonical(Path(args.worktree))
    assert_under_worktrees(root, worktree)
    if not requirements_clear(args.thread_id, root):
        raise SystemExit("Requirements Review is not clear for this worker")
    validate_worktree(root, worktree, git_stdout(worktree, "branch", "--show-current"))
    branch = git_stdout(worktree, "branch", "--show-current")
    assert_codex_branch(branch)
    if not branch_exists(root, branch):
        raise SystemExit(f"missing branch {branch}")
    if not is_clean(worktree) or not is_clean(root):
        raise SystemExit("refusing integration with dirty repository or worker worktree")
    base = "main"
    if not branch_exists(root, base):
        raise SystemExit("missing local main branch")
    ahead = int(git_stdout(root, "rev-list", "--count", f"{base}..{branch}") or "0")
    if ahead <= 0:
        raise SystemExit(f"branch {branch} has no commits ahead of {base}")
    main_before = git_stdout(root, "rev-parse", base)
    rebase = git(worktree, "rebase", base, check=False)
    if rebase.returncode != 0:
        raise SystemExit((rebase.stdout + rebase.stderr).strip() or f"rebase failed for {branch}")
    main_after = git_stdout(root, "rev-parse", base)
    if main_after != main_before:
        raise SystemExit("local main moved during integration")
    git(root, "checkout", base)
    git(root, "merge", "--ff-only", branch)
    git(root, "worktree", "remove", str(worktree))
    git(root, "branch", "-d", branch)
    print(f"Integrated {branch} into {base} and removed {worktree}")


def main() -> None:
    if len(sys.argv) < 2:
        fail("missing action")
    action = sys.argv[1]
    if action == "create":
        create(sys.argv[2] if len(sys.argv) > 2 else "worker")
    elif action == "archive":
        archive(sys.argv[2] if len(sys.argv) > 2 else "worker")
    elif action == "integrate":
        integrate(sys.argv[2:])
    else:
        fail(f"unknown action: {action}")


if __name__ == "__main__":
    main()
