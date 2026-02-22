from __future__ import annotations

import argparse
import os
import re
import subprocess
from pathlib import Path

from dotenv import load_dotenv
from github import Github

PROJECT_ROOT = Path(__file__).resolve().parents[2]


def _load_env() -> None:
    load_dotenv(PROJECT_ROOT / ".env", override=True)


def _resolve_repo_from_origin(path: Path) -> str:
    process = subprocess.run(
        ["git", "remote", "get-url", "origin"],
        cwd=path,
        capture_output=True,
        text=True,
    )
    if process.returncode != 0:
        raise RuntimeError("Unable to resolve git origin URL for current repository")

    origin = process.stdout.strip()
    patterns = [
        r"^git@github\.com:(?P<repo>[^\s]+?)(?:\.git)?$",
        r"^https://github\.com/(?P<repo>[^\s]+?)(?:\.git)?$",
        r"^ssh://git@github\.com/(?P<repo>[^\s]+?)(?:\.git)?$",
    ]
    for pattern in patterns:
        match = re.match(pattern, origin)
        if match:
            return match.group("repo")
    raise RuntimeError(f"Unsupported origin URL format: {origin}")


def main() -> None:
    _load_env()

    parser = argparse.ArgumentParser(description="Smoke-test PyGithub auth and repo access")
    parser.add_argument("--repo", help="owner/name repo to test. Defaults to current repo origin.")
    parser.add_argument("--cwd", help="repo path used for default --repo resolution", default=os.getcwd())
    args = parser.parse_args()

    token = os.getenv("GITHUB_TOKEN", "").strip()
    if not token:
        raise SystemExit("Missing GITHUB_TOKEN in ~/.codex/mcp/gitops/.env")

    gh = Github(token)
    me = gh.get_user().login

    repo_name = args.repo or _resolve_repo_from_origin(Path(args.cwd).expanduser())
    repo = gh.get_repo(repo_name)

    print(f"authenticated_as={me}")
    print(f"repo={repo.full_name}")
    print(f"default_branch={repo.default_branch}")
    print(f"open_issues={repo.open_issues_count}")


if __name__ == "__main__":
    main()
