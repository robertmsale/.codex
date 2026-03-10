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
    parser = argparse.ArgumentParser(
        prog="robdex",
        description=(
            "Robdex orchestration CLI. Use it to coordinate workers, bookkeeping, "
            "and reasonable approval-routing/escalation requests."
        ),
        epilog=(
            "Approval-routing bar: the command must make sense for the task and "
            "must not be blatantly destructive."
        ),
    )
    sub = parser.add_subparsers(dest="cmd", required=True)

    sub.add_parser("list-projects")

    p_list = sub.add_parser("list-agents")
    p_list.add_argument("--include-archived", action="store_true")
    p_list.add_argument("--all-projects", action="store_true")
    p_list.add_argument("--project-path")

    p_group_list = sub.add_parser("list-thread-groups")
    p_group_list.add_argument("--project-path")

    p_group_create = sub.add_parser("create-thread-group")
    p_group_create.add_argument("--title", required=True)
    p_group_create.add_argument("--project-path")
    p_group_create.add_argument("--seed-thread-id")

    p_group_update = sub.add_parser("update-thread-group")
    p_group_update.add_argument("--group-id", required=True)
    p_group_update.add_argument("--title")
    p_group_update.add_argument("--project-path")
    collapse_group = p_group_update.add_mutually_exclusive_group()
    collapse_group.add_argument("--collapsed", action="store_true")
    collapse_group.add_argument("--expanded", action="store_true")

    p_group_move = sub.add_parser("move-thread-to-group")
    p_group_move.add_argument("--thread-id", required=True)
    p_group_move.add_argument("--group-id")
    p_group_move.add_argument("--project-path")
    p_group_move.add_argument("--remove", action="store_true")

    p_group_delete = sub.add_parser("delete-thread-group")
    p_group_delete.add_argument("--group-id", required=True)
    p_group_delete.add_argument("--project-path")

    p_group_archive = sub.add_parser("archive-thread-group")
    p_group_archive.add_argument("--group-id", required=True)
    p_group_archive.add_argument("--project-path")

    p_spawn = sub.add_parser("spawn-agent")
    p_spawn.add_argument("--name", required=True)
    p_spawn.add_argument("--prompt", default="")
    p_spawn.add_argument("--cwd")
    p_spawn.add_argument("--issue-number", type=int)

    p_unarchive = sub.add_parser("unarchive-agent")
    p_unarchive.add_argument("--name", required=True)
    p_unarchive.add_argument("--prompt", default="")
    p_unarchive.add_argument("--project-path")

    p_archive = sub.add_parser("archive-agent")
    p_archive.add_argument("--to-thread-id")
    p_archive.add_argument("--name")
    p_archive.add_argument("--project-path")

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
        elif args.cmd == "list-thread-groups":
            out = robdex_server.robdex_list_thread_groups(
                from_thread_id=thread_id,
                project_path=args.project_path,
                ctx=ctx,
            )
        elif args.cmd == "create-thread-group":
            out = robdex_server.robdex_create_thread_group(
                from_thread_id=thread_id,
                title=args.title,
                project_path=args.project_path,
                seed_thread_id=args.seed_thread_id,
                ctx=ctx,
            )
        elif args.cmd == "update-thread-group":
            collapsed_state = None
            if args.collapsed:
                collapsed_state = True
            elif args.expanded:
                collapsed_state = False
            out = robdex_server.robdex_update_thread_group(
                from_thread_id=thread_id,
                group_id=args.group_id,
                title=args.title,
                is_collapsed=collapsed_state,
                project_path=args.project_path,
                ctx=ctx,
            )
        elif args.cmd == "move-thread-to-group":
            if args.remove and args.group_id:
                parser.error("move-thread-to-group accepts either --group-id or --remove")
            out = robdex_server.robdex_move_thread_to_group(
                from_thread_id=thread_id,
                thread_id=args.thread_id,
                group_id=None if args.remove else args.group_id,
                project_path=args.project_path,
                ctx=ctx,
            )
        elif args.cmd == "delete-thread-group":
            out = robdex_server.robdex_delete_thread_group(
                from_thread_id=thread_id,
                group_id=args.group_id,
                project_path=args.project_path,
                ctx=ctx,
            )
        elif args.cmd == "archive-thread-group":
            out = robdex_server.robdex_archive_thread_group(
                from_thread_id=thread_id,
                group_id=args.group_id,
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
        elif args.cmd == "archive-agent":
            out = robdex_server.robdex_archive_agent(
                from_thread_id=thread_id,
                to_thread_id=args.to_thread_id,
                name=args.name,
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
