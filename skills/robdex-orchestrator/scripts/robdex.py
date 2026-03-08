#!/Users/robertsale/.codex/mcp/robdex-orchestrator/.venv/bin/python3
from __future__ import annotations

import argparse
import os
import sys

from robdex_orchestrator_mcp import server as robdex_server


class _ScriptContext:
    def __init__(self, thread_id: str) -> None:
        self.session_id = f"robdex-script:{thread_id}"


def _require_thread_id() -> str:
    thread_id = (os.getenv("CODEX_THREAD_ID") or "").strip()
    if not thread_id:
        raise SystemExit("robdex: CODEX_THREAD_ID is required")
    return thread_id


def main() -> int:
    parser = argparse.ArgumentParser(prog="robdex")
    sub = parser.add_subparsers(dest="cmd", required=True)

    sub.add_parser("list-projects")

    p_list = sub.add_parser("list-agents")
    p_list.add_argument("--include-archived", action="store_true")
    p_list.add_argument("--all-projects", action="store_true")
    p_list.add_argument("--project-path")

    p_spawn = sub.add_parser("spawn-agent")
    p_spawn.add_argument("--name", required=True)
    p_spawn.add_argument("--prompt", default="")
    p_spawn.add_argument("--cwd")
    p_spawn.add_argument("--issue-number", type=int)

    p_unarchive = sub.add_parser("unarchive-agent")
    p_unarchive.add_argument("--name", required=True)
    p_unarchive.add_argument("--prompt", default="")
    p_unarchive.add_argument("--project-path")

    p_rename = sub.add_parser("rename-agent")
    p_rename.add_argument("--new-name", required=True)
    p_rename.add_argument("--to-thread-id")
    p_rename.add_argument("--name")
    p_rename.add_argument("--project-path")

    p_send = sub.add_parser("send-message")
    p_send.add_argument("--text", required=True)
    p_send.add_argument("--to-thread-id")
    p_send.add_argument("--name")
    p_send.add_argument("--project-path")

    p_meta = sub.add_parser("set-worker-metadata")
    p_meta.add_argument("--to-thread-id")
    p_meta.add_argument("--name")
    p_meta.add_argument("--project-path")
    p_meta.add_argument("--issue-number", type=int)
    p_meta.add_argument("--clear-issue-number", action="store_true")
    p_meta.add_argument("--pr-number", type=int)
    p_meta.add_argument("--clear-pr-number", action="store_true")
    p_meta.add_argument("--blocked-reason")
    p_meta.add_argument("--unblock-when")
    p_meta.add_argument("--clear-blocked", action="store_true")

    args = parser.parse_args()
    thread_id = _require_thread_id()
    ctx = _ScriptContext(thread_id)

    try:
        if args.cmd == "list-projects":
            out = robdex_server.robdex_list_projects(from_thread_id=thread_id, ctx=ctx)
        elif args.cmd == "list-agents":
            out = robdex_server.robdex_list_agents(
                from_thread_id=thread_id,
                include_archived=args.include_archived,
                include_all_projects=args.all_projects,
                project_path=args.project_path,
                ctx=ctx,
            )
        elif args.cmd == "spawn-agent":
            out = robdex_server.robdex_spawn_agent(
                from_thread_id=thread_id,
                name=args.name,
                prompt=args.prompt,
                cwd=args.cwd,
                issue_number=args.issue_number,
                ctx=ctx,
            )
        elif args.cmd == "unarchive-agent":
            out = robdex_server.robdex_unarchive_agent(
                from_thread_id=thread_id,
                name=args.name,
                prompt=args.prompt,
                project_path=args.project_path,
                ctx=ctx,
            )
        elif args.cmd == "rename-agent":
            out = robdex_server.robdex_rename_agent(
                from_thread_id=thread_id,
                new_name=args.new_name,
                to_thread_id=args.to_thread_id,
                name=args.name,
                project_path=args.project_path,
                ctx=ctx,
            )
        elif args.cmd == "send-message":
            out = robdex_server.robdex_send_message(
                from_thread_id=thread_id,
                text=args.text,
                to_thread_id=args.to_thread_id,
                name=args.name,
                project_path=args.project_path,
                ctx=ctx,
            )
        elif args.cmd == "set-worker-metadata":
            out = robdex_server.robdex_set_worker_metadata(
                from_thread_id=thread_id,
                issue_number=args.issue_number,
                pull_request_number=args.pr_number,
                blocked_reason=args.blocked_reason,
                unblock_when=args.unblock_when,
                clear_issue_number=args.clear_issue_number,
                clear_pull_request_number=args.clear_pr_number,
                clear_blocked=args.clear_blocked,
                to_thread_id=args.to_thread_id,
                name=args.name,
                project_path=args.project_path,
                ctx=ctx,
            )
        else:
            parser.error(f"unknown command: {args.cmd}")
            return 2
    except Exception as exc:  # noqa: BLE001
        print(f"robdex: {exc}", file=sys.stderr)
        return 1

    if out:
        print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
