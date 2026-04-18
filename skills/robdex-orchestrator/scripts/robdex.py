#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
RESOURCE_DIR = SCRIPT_DIR.parent / "resources"


def _require_thread_id() -> str:
    thread_id = os.environ["CODEX_THREAD_ID"].strip()
    if not thread_id:
        raise SystemExit("robdex: CODEX_THREAD_ID is empty")
    return thread_id


def _base_url() -> str:
    host = (os.getenv("ROBDEX_BRIDGE_HOST") or "127.0.0.1").strip() or "127.0.0.1"
    port = (os.getenv("ROBDEX_BRIDGE_PORT") or "42080").strip() or "42080"
    return f"http://{host}:{port}"


def _normalize_text(value: str | None) -> str | None:
    if value is None:
        return None
    trimmed = value.strip()
    return trimmed or None


def _normalize_path(value: str | None) -> str | None:
    if value is None:
        return None
    trimmed = value.strip()
    if not trimmed:
        return None
    return str(Path(trimmed).expanduser().resolve(strict=False))


def _quoted(value: str) -> str:
    return json.dumps(value, ensure_ascii=True)


def _request_json(
    method: str,
    path: str,
    *,
    query: dict[str, str] | None = None,
    body: dict[str, Any] | None = None,
) -> dict[str, Any]:
    url = _base_url() + path
    if query:
        url += "?" + urllib.parse.urlencode(query)
    data = None
    headers = {"Accept": "application/json"}
    if body is not None:
        data = json.dumps(body, separators=(",", ":"), sort_keys=True).encode("utf-8")
        headers["Content-Type"] = "application/json"
    request = urllib.request.Request(url, data=data, headers=headers, method=method.upper())
    try:
        with urllib.request.urlopen(request) as response:
            payload = response.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace").strip()
        try:
            parsed = json.loads(detail)
            if isinstance(parsed, dict) and parsed.get("error"):
                message = str(parsed["error"]).strip()
            else:
                message = detail or f"HTTP {exc.code}"
        except Exception:
            message = detail or f"HTTP {exc.code}"
        raise SystemExit(f"robdex: {message}") from exc
    except Exception as exc:
        raise SystemExit(f"robdex: request failed for {method.upper()} {path}: {exc}") from exc

    try:
        decoded = json.loads(payload)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"robdex: invalid JSON response for {method.upper()} {path}") from exc
    if not isinstance(decoded, dict):
        raise SystemExit(f"robdex: unexpected response for {method.upper()} {path}")
    return decoded


def _read_resource_text(*parts: str) -> str:
    path = RESOURCE_DIR.joinpath(*parts)
    return path.read_text(encoding="utf-8").strip()


def _whoami(thread_id: str) -> dict[str, Any]:
    return _request_json("GET", "/orchestrator/whoami", query={"senderThreadId": thread_id})


def _resolve_text_input(
    parser: argparse.ArgumentParser,
    args: argparse.Namespace,
    *,
    inline_attr: str,
    file_attr: str,
    stdin_attr: str,
    label: str,
) -> str:
    inline_value = getattr(args, inline_attr)
    if inline_value is not None:
        return inline_value
    file_path = getattr(args, file_attr)
    if file_path is not None:
        return Path(file_path).expanduser().read_text(encoding="utf-8")
    if getattr(args, stdin_attr):
        return sys.stdin.read()
    parser.error(f"{label} input is required")
    raise AssertionError("unreachable")


def _load_snapshot() -> dict[str, Any]:
    return _request_json("GET", "/state/snapshot")


def _list_visible_agents(thread_id: str) -> list[dict[str, Any]]:
    payload = _request_json(
        "GET",
        "/orchestrator/agents",
        query={"senderThreadId": thread_id, "includeArchived": "0"},
    )
    items = payload.get("items")
    return items if isinstance(items, list) else []


def _resolve_recipient_for_project_filter(
    *,
    thread_id: str,
    recipient_thread_id: str | None,
    recipient_name: str | None,
    project_path: str,
) -> tuple[str | None, str | None]:
    agents = _list_visible_agents(thread_id)
    normalized_project = _normalize_path(project_path)
    scoped = [item for item in agents if _normalize_path(str(item.get("projectPath") or "")) == normalized_project]
    if recipient_thread_id:
        for item in scoped:
            if str(item.get("id") or "").strip() == recipient_thread_id:
                return recipient_thread_id, None
        raise SystemExit(f"robdex: thread not visible in project {_quoted(normalized_project or project_path)}: {recipient_thread_id}")
    if recipient_name:
        matches = []
        lowered = recipient_name.casefold()
        for item in scoped:
            display_name = _normalize_text(str(item.get("displayName") or ""))
            if display_name and display_name.casefold() == lowered:
                matches.append(item)
        if not matches:
            raise SystemExit(f"robdex: no visible thread named {_quoted(recipient_name)} in project {_quoted(normalized_project or project_path)}")
        if len(matches) > 1:
            raise SystemExit(f"robdex: multiple visible threads named {_quoted(recipient_name)} in project {_quoted(normalized_project or project_path)}")
        return str(matches[0].get("id") or "").strip(), None
    raise SystemExit("robdex: provide to_thread_id or name")


def _print_lines(lines: list[str]) -> None:
    if lines:
        print("\n".join(lines))


def _cmd_whoami(thread_id: str) -> None:
    payload = _whoami(thread_id)
    lines = [
        f"role={payload.get('role') or 'unknown'}",
        f"thread_id={payload.get('threadId') or thread_id}",
    ]
    display_name = _normalize_text(str(payload.get("displayName") or ""))
    if display_name:
        lines.append(f"display_name={display_name}")
    project_path = _normalize_text(str(payload.get("projectPath") or ""))
    if project_path:
        lines.append(f"project_path={project_path}")
    cwd = _normalize_text(str(payload.get("cwd") or ""))
    if cwd:
        lines.append(f"cwd={cwd}")
    _print_lines(lines)


def _current_role(thread_id: str) -> str:
    payload = _whoami(thread_id)
    role = _normalize_text(str(payload.get("role") or ""))
    return role or "unknown"


def _handoff_guidance_text(role: str) -> str:
    chunks = [_read_resource_text("handoff", "shared.md")]
    role_map = {
        "designer": "designer.md",
        "orchestrator": "orchestrator.md",
        "operator": "operator.md",
        "hidden": "hidden.md",
    }
    role_file = role_map.get(role)
    if role_file:
        chunks.append(_read_resource_text("handoff", role_file))
    return "\n\n".join(chunk for chunk in chunks if chunk)


def _print_handoff_help(parser: argparse.ArgumentParser, thread_id: str) -> None:
    parser.print_help()
    try:
        role = _current_role(thread_id)
        guidance = _handoff_guidance_text(role)
    except SystemExit as exc:
        guidance = _read_resource_text("handoff", "shared.md")
        print(f"\nRole-specific guidance unavailable: {exc}", file=sys.stderr)
    print()
    print(guidance)


def _cmd_handoff(thread_id: str, args: argparse.Namespace, parser: argparse.ArgumentParser) -> None:
    payload = _whoami(thread_id)
    role = _normalize_text(str(payload.get("role") or "")) or "unknown"
    if role in {"worker", "qa"}:
        raise SystemExit(f"robdex: handoff is not available for role {_quoted(role)}")

    project_path = _normalize_path(str(payload.get("projectPath") or ""))
    if not project_path:
        raise SystemExit("robdex: current thread is not attached to a project")

    prompt = _resolve_text_input(
        parser,
        args,
        inline_attr="prompt",
        file_attr="prompt_file",
        stdin_attr="prompt_stdin",
        label="handoff prompt",
    ).strip()
    if not prompt:
        raise SystemExit("robdex: handoff prompt is empty")

    response = _request_json(
        "POST",
        "/orchestrator/warm-handoff",
        body={
            "senderThreadId": thread_id,
            "recipientThreadId": thread_id,
            "projectPath": project_path,
            "prompt": prompt,
        },
    )
    replacement_thread_id = _normalize_text(str(response.get("replacementThreadId") or "")) or "unknown-thread-id"
    previous_thread_id = _normalize_text(str(response.get("previousThreadId") or "")) or thread_id
    print(f"Warmed handoff {previous_thread_id} -> {replacement_thread_id}")


def _cmd_list_projects() -> None:
    snapshot = _load_snapshot()
    state = snapshot.get("state")
    projects = state.get("projects") if isinstance(state, dict) else None
    if not isinstance(projects, dict) or not projects:
        return
    lines: list[str] = []
    for _, raw in sorted(projects.items(), key=lambda item: str((item[1] or {}).get("name") or item[0]).casefold()):
        if not isinstance(raw, dict) or raw.get("detached") is True:
            continue
        name = _normalize_text(str(raw.get("name") or "")) or "unnamed"
        root = _normalize_text(str(raw.get("projectRoot") or ""))
        cwd = _normalize_text(str(raw.get("cwd") or ""))
        orchestrator = _normalize_text(str(raw.get("orchestratorThreadID") or ""))
        parts = [name]
        if root:
            parts.append(f"root={root}")
        if cwd:
            parts.append(f"cwd={cwd}")
        if orchestrator:
            parts.append(f"orchestrator={orchestrator}")
        lines.append(" | ".join(parts))
    _print_lines(lines)


def _cmd_list_agents(thread_id: str, project_path: str | None) -> None:
    items = _list_visible_agents(thread_id)
    normalized_project = _normalize_path(project_path)
    if normalized_project:
        items = [item for item in items if _normalize_path(str(item.get("projectPath") or "")) == normalized_project]
    lines: list[str] = []
    for item in items:
        agent_id = _normalize_text(str(item.get("id") or "")) or "unknown-thread-id"
        display_name = _normalize_text(str(item.get("displayName") or "")) or agent_id
        role = _normalize_text(str(item.get("role") or "")) or ("orchestrator" if bool(item.get("isOrchestrator")) else "worker")
        status = "running" if bool(item.get("isRunning")) else "idle"
        parts = [f"{display_name} ({agent_id})", role, status]
        project = _normalize_text(str(item.get("projectPath") or ""))
        if project:
            parts.append(f"project={project}")
        issue = item.get("issueNumber")
        if isinstance(issue, int):
            parts.append(f"issue=#{issue}")
        pr_number = item.get("pullRequestNumber")
        if isinstance(pr_number, int):
            parts.append(f"pr=#{pr_number}")
        blocked_reason = _normalize_text(str(item.get("blockedReason") or ""))
        if blocked_reason:
            blocked = f"blocked={_quoted(blocked_reason)}"
            unblock_when = _normalize_text(str(item.get("unblockWhen") or ""))
            if unblock_when:
                blocked += f" until={_quoted(unblock_when)}"
            parts.append(blocked)
        lines.append(" | ".join(parts))
    _print_lines(lines)


def _cmd_list_pending_approvals(thread_id: str) -> None:
    payload = _request_json("GET", "/orchestrator/pending-approvals", query={"senderThreadId": thread_id})
    items = payload.get("items")
    if not isinstance(items, list):
        return
    lines: list[str] = []
    for item in items:
        if not isinstance(item, dict):
            continue
        approval_id = _normalize_text(str(item.get("id") or "")) or "unknown-approval"
        kind = _normalize_text(str(item.get("kind") or "")) or "unknown"
        target_thread = _normalize_text(str(item.get("threadID") or "")) or "unknown-thread-id"
        title = _normalize_text(str(item.get("title") or "")) or "Pending approval"
        parts = [approval_id, f"kind={kind}", f"thread={target_thread}", title]
        command = _normalize_text(str(item.get("command") or ""))
        if command:
            parts.append(f"command={_quoted(command)}")
        command_cwd = _normalize_text(str(item.get("commandCWD") or ""))
        if command_cwd:
            parts.append(f"cwd={command_cwd}")
        lines.append(" | ".join(parts))
    _print_lines(lines)


def _cmd_list_thread_groups(thread_id: str, project_path: str | None) -> None:
    query = {"senderThreadId": thread_id}
    normalized_project = _normalize_path(project_path)
    if normalized_project:
        query["projectPath"] = normalized_project
    payload = _request_json("GET", "/orchestrator/thread-groups", query=query)
    items = payload.get("items")
    if not isinstance(items, list):
        return
    lines: list[str] = []
    for item in items:
        if not isinstance(item, dict):
            continue
        group_id = _normalize_text(str(item.get("id") or "")) or "unknown-group-id"
        title = _normalize_text(str(item.get("title") or "")) or group_id
        thread_ids = item.get("threadIDs")
        thread_summary = ", ".join(str(entry).strip() for entry in thread_ids) if isinstance(thread_ids, list) and thread_ids else "(empty)"
        collapsed = "collapsed" if bool(item.get("isCollapsed")) else "expanded"
        lines.append(f"{title} ({group_id}) | {collapsed} | threads={thread_summary}")
    _print_lines(lines)


def _print_group_mutation(payload: dict[str, Any]) -> None:
    project_path = _normalize_text(str(payload.get("projectPath") or ""))
    changed = _normalize_text(str(payload.get("changedGroupId") or ""))
    items = payload.get("items")
    count = len(items) if isinstance(items, list) else 0
    parts = [f"groups={count}"]
    if project_path:
        parts.append(f"project={project_path}")
    if changed:
        parts.append(f"changed={changed}")
    print(" | ".join(parts))


def _cmd_spawn_agent(thread_id: str, args: argparse.Namespace) -> None:
    payload = _request_json(
        "POST",
        "/orchestrator/spawn-agent",
        body={
            "senderThreadId": thread_id,
            "name": args.name,
            "prompt": args.prompt,
            "cwd": _normalize_path(args.cwd),
            "role": _normalize_text(args.role),
            "issueNumber": args.issue_number,
        },
    )
    agent = payload.get("agent")
    if not isinstance(agent, dict):
        raise SystemExit("robdex: spawn-agent response missing agent")
    display_name = _normalize_text(str(agent.get("displayName") or "")) or args.name
    spawned_thread_id = _normalize_text(str(agent.get("threadId") or "")) or "no-thread-id"
    print(f"Spawned {_quoted(display_name)} ({spawned_thread_id})")


def _cmd_archive_agent(thread_id: str, args: argparse.Namespace) -> None:
    payload = _request_json(
        "POST",
        "/orchestrator/archive-agent",
        body={
            "senderThreadId": thread_id,
            "recipientThreadId": _normalize_text(args.to_thread_id),
            "recipientName": _normalize_text(args.name),
            "projectPath": _normalize_path(args.project_path),
        },
    )
    target_id = _normalize_text(str(payload.get("recipientThreadId") or "")) or "unknown-thread-id"
    display_name = _normalize_text(str(payload.get("recipientDisplayName") or "")) or target_id
    status = "Already archived" if bool(payload.get("alreadyArchived")) else "Archived"
    print(f"{status} {_quoted(display_name)} ({target_id})")


def _cmd_rename_agent(thread_id: str, args: argparse.Namespace) -> None:
    payload = _request_json(
        "POST",
        "/orchestrator/rename-agent",
        body={
            "senderThreadId": thread_id,
            "recipientThreadId": _normalize_text(args.to_thread_id),
            "recipientName": _normalize_text(args.name),
            "projectPath": _normalize_path(args.project_path),
            "newName": args.new_name,
        },
    )
    target_id = _normalize_text(str(payload.get("recipientThreadId") or "")) or "unknown-thread-id"
    previous = _normalize_text(str(payload.get("previousDisplayName") or "")) or target_id
    new_name = _normalize_text(str(payload.get("newName") or "")) or args.new_name
    print(f"Renamed {_quoted(previous)} ({target_id}) -> {_quoted(new_name)}")


def _cmd_send_message(thread_id: str, args: argparse.Namespace, parser: argparse.ArgumentParser) -> None:
    text = _resolve_text_input(
        parser,
        args,
        inline_attr="text",
        file_attr="text_file",
        stdin_attr="text_stdin",
        label="send-message text",
    )
    recipient_thread_id = _normalize_text(args.to_thread_id)
    recipient_name = _normalize_text(args.name)
    project_path = _normalize_path(args.project_path)
    if project_path:
        recipient_thread_id, recipient_name = _resolve_recipient_for_project_filter(
            thread_id=thread_id,
            recipient_thread_id=recipient_thread_id,
            recipient_name=recipient_name,
            project_path=project_path,
        )
    payload = _request_json(
        "POST",
        "/orchestrator/agent-message",
        body={
            "senderThreadId": thread_id,
            "recipientThreadId": recipient_thread_id,
            "recipientName": recipient_name,
            "text": text,
        },
    )
    target_id = _normalize_text(str(payload.get("recipientThreadId") or "")) or recipient_thread_id or "unknown-thread-id"
    display_name = _normalize_text(str(payload.get("recipientDisplayName") or "")) or recipient_name or target_id
    print(f"Sent to {_quoted(display_name)} ({target_id})")


def _cmd_set_worker_metadata(thread_id: str, args: argparse.Namespace) -> None:
    payload = _request_json(
        "POST",
        "/orchestrator/worker-metadata",
        body={
            "senderThreadId": thread_id,
            "recipientThreadId": _normalize_text(args.to_thread_id),
            "recipientName": _normalize_text(args.name),
            "projectPath": _normalize_path(args.project_path),
            "issueNumber": args.issue_number,
            "pullRequestNumber": args.pr_number,
            "blockedReason": _normalize_text(args.blocked_reason),
            "unblockWhen": _normalize_text(args.unblock_when),
            "clearIssueNumber": bool(args.clear_issue_number),
            "clearPullRequestNumber": bool(args.clear_pr_number),
            "clearBlocked": bool(args.clear_blocked),
        },
    )
    target_id = _normalize_text(str(payload.get("recipientThreadId") or "")) or _normalize_text(args.to_thread_id) or "unknown-thread-id"
    display_name = _normalize_text(str(payload.get("recipientDisplayName") or "")) or _normalize_text(args.name) or target_id
    summary_parts: list[str] = []
    if isinstance(payload.get("issueNumber"), int):
        summary_parts.append(f"issue=#{payload['issueNumber']}")
    if isinstance(payload.get("pullRequestNumber"), int):
        summary_parts.append(f"pr=#{payload['pullRequestNumber']}")
    blocked_reason = _normalize_text(str(payload.get("blockedReason") or ""))
    if blocked_reason:
        blocked = f"blocked={_quoted(blocked_reason)}"
        unblock_when = _normalize_text(str(payload.get("unblockWhen") or ""))
        if unblock_when:
            blocked += f" until={_quoted(unblock_when)}"
        summary_parts.append(blocked)
    summary = ", ".join(summary_parts) if summary_parts else "cleared"
    print(f"Updated {_quoted(display_name)} ({target_id}): {summary}")


def _cmd_approval_decision(thread_id: str, approval_id: str, decision: str, message: str | None) -> None:
    if decision == "accept":
        raise SystemExit(
            "robdex: Approvals are disabled.\n"
            "A command that requires approval is a sign of drift or instructions not being followed.\n"
            "Privileged commands do not require escalation, but they must be executed plainly.\n"
            "Do not use logical operators like `&&` or `||`, command separators like `;`, shell expansions, command substitution, or subshells.\n"
            "Run the privileged command as a single plain command segment that the privileged executor can accept."
        )
    payload = _request_json(
        "POST",
        "/orchestrator/approval-decision",
        body={
            "senderThreadId": thread_id,
            "approvalId": approval_id,
            "decision": decision,
            "message": _normalize_text(message),
        },
    )
    result = payload.get("result")
    if not isinstance(result, dict):
        raise SystemExit("robdex: approval-decision response missing result")
    suffix_parts: list[str] = []
    if bool(result.get("followUpMessageRequested")):
        suffix_parts.append("follow-up requested")
    if bool(result.get("followUpMessageSent")):
        suffix_parts.append("follow-up sent")
    follow_up_error = _normalize_text(str(result.get("followUpError") or ""))
    if follow_up_error:
        suffix_parts.append(f"follow-up error={_quoted(follow_up_error)}")
    suffix = f" | {'; '.join(suffix_parts)}" if suffix_parts else ""
    verb = "Approved" if decision == "accept" else "Declined"
    print(f"{verb} {_quoted(approval_id)}{suffix}")


def _add_handoff_parser(sub: argparse._SubParsersAction[argparse.ArgumentParser]) -> argparse.ArgumentParser:
    parser = sub.add_parser(
        "handoff",
        help="Warm handoff the current thread into a replacement thread with a fresh initial prompt.",
    )
    prompt_group = parser.add_mutually_exclusive_group(required=True)
    prompt_group.add_argument("--prompt")
    prompt_group.add_argument("--prompt-file")
    prompt_group.add_argument("--prompt-stdin", action="store_true")
    return parser


def build_parser() -> tuple[argparse.ArgumentParser, argparse.ArgumentParser]:
    parser = argparse.ArgumentParser(prog="robdex", description="Robdex bridge CLI.")
    sub = parser.add_subparsers(dest="cmd", required=True)

    sub.add_parser("whoami")
    sub.add_parser("list-projects")

    p_list = sub.add_parser("list-agents")
    p_list.add_argument("--project-path")

    sub.add_parser("list-pending-approvals")

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
    p_spawn.add_argument("--role", choices=["worker", "qa", "hidden"], default="worker")
    p_spawn.add_argument("--issue-number", type=int)

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
    send_text = p_send.add_mutually_exclusive_group(required=True)
    send_text.add_argument("--text")
    send_text.add_argument("--text-file")
    send_text.add_argument("--text-stdin", action="store_true")
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

    p_handoff = _add_handoff_parser(sub)

    p_approve = sub.add_parser("approve-approval", help="disabled: approval-based command execution is not allowed")
    p_approve.add_argument("--approval-id", required=True)

    p_decline = sub.add_parser("decline-approval")
    p_decline.add_argument("--approval-id", required=True)
    p_decline.add_argument("--message")

    return parser, p_handoff


def main() -> int:
    parser, handoff_parser = build_parser()
    if len(sys.argv) >= 2 and sys.argv[1] == "handoff" and any(flag in sys.argv[2:] for flag in ("-h", "--help")):
        _print_handoff_help(handoff_parser, _require_thread_id())
        return 0
    args = parser.parse_args()
    thread_id = _require_thread_id()

    if args.cmd == "whoami":
        _cmd_whoami(thread_id)
    elif args.cmd == "list-projects":
        _cmd_list_projects()
    elif args.cmd == "list-agents":
        _cmd_list_agents(thread_id, args.project_path)
    elif args.cmd == "list-pending-approvals":
        _cmd_list_pending_approvals(thread_id)
    elif args.cmd == "list-thread-groups":
        _cmd_list_thread_groups(thread_id, args.project_path)
    elif args.cmd == "create-thread-group":
        payload = _request_json(
            "POST",
            "/orchestrator/thread-groups/create",
            body={
                "senderThreadId": thread_id,
                "projectPath": _normalize_path(args.project_path),
                "title": args.title,
                "seedThreadId": _normalize_text(args.seed_thread_id),
            },
        )
        _print_group_mutation(payload)
    elif args.cmd == "update-thread-group":
        collapsed = True if args.collapsed else False if args.expanded else None
        payload = _request_json(
            "POST",
            "/orchestrator/thread-groups/update",
            body={
                "senderThreadId": thread_id,
                "projectPath": _normalize_path(args.project_path),
                "groupId": args.group_id,
                "title": _normalize_text(args.title),
                "collapsed": collapsed,
            },
        )
        _print_group_mutation(payload)
    elif args.cmd == "move-thread-to-group":
        if args.remove and args.group_id:
            parser.error("move-thread-to-group accepts either --group-id or --remove")
        payload = _request_json(
            "POST",
            "/orchestrator/thread-groups/move-thread",
            body={
                "senderThreadId": thread_id,
                "projectPath": _normalize_path(args.project_path),
                "threadId": args.thread_id,
                "targetGroupId": None if args.remove else _normalize_text(args.group_id),
            },
        )
        _print_group_mutation(payload)
    elif args.cmd == "delete-thread-group":
        payload = _request_json(
            "POST",
            "/orchestrator/thread-groups/delete",
            body={
                "senderThreadId": thread_id,
                "projectPath": _normalize_path(args.project_path),
                "groupId": args.group_id,
            },
        )
        _print_group_mutation(payload)
    elif args.cmd == "archive-thread-group":
        payload = _request_json(
            "POST",
            "/orchestrator/thread-groups/archive",
            body={
                "senderThreadId": thread_id,
                "projectPath": _normalize_path(args.project_path),
                "groupId": args.group_id,
            },
        )
        archived = payload.get("archivedThreadIds")
        skipped = payload.get("skippedThreadIds")
        title = _normalize_text(str(payload.get("title") or "")) or args.group_id
        group_id = _normalize_text(str(payload.get("groupId") or "")) or args.group_id
        archived_summary = ",".join(str(item).strip() for item in archived) if isinstance(archived, list) and archived else "(none)"
        skipped_summary = ",".join(str(item).strip() for item in skipped) if isinstance(skipped, list) and skipped else "(none)"
        print(f"Archived {_quoted(title)} ({group_id}) | archived={archived_summary}; skipped={skipped_summary}")
    elif args.cmd == "spawn-agent":
        _cmd_spawn_agent(thread_id, args)
    elif args.cmd == "archive-agent":
        _cmd_archive_agent(thread_id, args)
    elif args.cmd == "rename-agent":
        _cmd_rename_agent(thread_id, args)
    elif args.cmd == "send-message":
        _cmd_send_message(thread_id, args, parser)
    elif args.cmd == "set-worker-metadata":
        _cmd_set_worker_metadata(thread_id, args)
    elif args.cmd == "handoff":
        _cmd_handoff(thread_id, args, parser)
    elif args.cmd == "approve-approval":
        _cmd_approval_decision(thread_id, args.approval_id, "accept", None)
    elif args.cmd == "decline-approval":
        _cmd_approval_decision(thread_id, args.approval_id, "decline", args.message)
    else:
        parser.error(f"unknown command: {args.cmd}")
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
