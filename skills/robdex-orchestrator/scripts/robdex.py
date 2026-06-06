#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
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


def _read_json_file(path_text: str, label: str) -> Any:
    path = Path(path_text).expanduser()
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise SystemExit(f"robdex: unable to read {label} file {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise SystemExit(f"robdex: invalid JSON in {path}: {exc}") from exc


def _camel_case_key(text: str, fallback: str) -> str:
    words = re.findall(r"[A-Za-z0-9]+", text)
    if not words:
        return fallback
    first, *rest = words[:8]
    key = first[:1].lower() + first[1:]
    key += "".join(word[:1].upper() + word[1:] for word in rest)
    if key[:1].isdigit():
        key = f"requirement{key}"
    return key


def _requirements_from_prose(title: str, text: str) -> dict[str, Any]:
    candidates: list[str] = []
    for raw_line in text.splitlines():
        line = raw_line.strip()
        line = re.sub(r"^[-*+]\s+", "", line)
        line = re.sub(r"^\d+[.)]\s+", "", line)
        if line:
            candidates.append(line)
    if not candidates and text.strip():
        candidates = [text.strip()]
    if not candidates:
        raise SystemExit("robdex: no requirement prose provided")

    seen: dict[str, int] = {}
    requirements = []
    for index, statement in enumerate(candidates, start=1):
        base_key = _camel_case_key(statement, f"requirement{index}")
        count = seen.get(base_key, 0) + 1
        seen[base_key] = count
        key = base_key if count == 1 else f"{base_key}{count}"
        requirements.append(
            {
                "key": key,
                "statement": statement,
                "severity": "high",
                "verificationMethod": "manualEvidence",
            }
        )
    return {
        "id": _camel_case_key(title, "requirements"),
        "title": title,
        "requirements": requirements,
    }


def _selected_composables(args: argparse.Namespace) -> list[str]:
    selected: list[str] = []
    seen: set[str] = set()
    for value in getattr(args, "include_composable", None) or []:
        for raw in str(value).split(","):
            item = raw.strip()
            if item and item not in seen:
                selected.append(item)
                seen.add(item)
    return selected


def _requirement_list(payload: Any) -> list[dict[str, Any]]:
    if isinstance(payload, list):
        return [item for item in payload if isinstance(item, dict)]
    if isinstance(payload, dict):
        requirements = payload.get("requirements")
        if isinstance(requirements, list):
            return [item for item in requirements if isinstance(item, dict)]
    raise SystemExit("robdex: requirements payload must be an array or an object with requirements")


def _merge_requirement_lists(*groups: list[dict[str, Any]]) -> list[dict[str, Any]]:
    merged: list[dict[str, Any]] = []
    by_key: dict[str, dict[str, Any]] = {}
    for group in groups:
        for item in group:
            key = _normalize_text(str(item.get("key") or ""))
            if not key:
                raise SystemExit("robdex: requirement key must be non-empty")
            existing = by_key.get(key)
            if existing is not None:
                if existing != item:
                    raise SystemExit(f"robdex: conflicting requirement key {key!r} while composing requirements")
                continue
            by_key[key] = item
            merged.append(item)
    return merged


def _compose_requirements_payload(
    *,
    title: str,
    base_payload: Any,
    composables: list[dict[str, Any]],
) -> dict[str, Any]:
    base_requirements = _requirement_list(base_payload)
    composed_requirements = _merge_requirement_lists(
        *[_requirement_list(item) for item in composables],
        base_requirements,
    )
    base_object = base_payload if isinstance(base_payload, dict) else {}
    return {
        "id": base_object.get("id") or _camel_case_key(title, "requirements"),
        "title": base_object.get("title") or title,
        "active": base_object.get("active", True),
        "enforceOnTurns": base_object.get("enforceOnTurns", True),
        "includeComposables": [str(item.get("id") or "") for item in composables],
        "requirements": composed_requirements,
    }


def _selected_composable_items(thread_id: str, args: argparse.Namespace) -> list[dict[str, Any]]:
    selected = _selected_composables(args)
    if not selected:
        return []
    payload = _requirements_composables_payload(thread_id, args)
    items = payload.get("items")
    if not isinstance(items, list):
        raise SystemExit("robdex: composables response missing items")
    by_id = {str(item.get("id") or ""): item for item in items if isinstance(item, dict)}
    composables = []
    for composable_id in selected:
        item = by_id.get(composable_id)
        if item is None:
            raise SystemExit(f"robdex: unknown requirements composable {composable_id!r}")
        composables.append(item)
    return composables


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


def _resolve_recipient_thread_id(
    *,
    thread_id: str,
    recipient_thread_id: str | None,
    recipient_name: str | None,
    project_path: str | None,
) -> str:
    if project_path:
        resolved_thread_id, _ = _resolve_recipient_for_project_filter(
            thread_id=thread_id,
            recipient_thread_id=recipient_thread_id,
            recipient_name=recipient_name,
            project_path=project_path,
        )
        if resolved_thread_id:
            return resolved_thread_id
    if recipient_thread_id:
        return recipient_thread_id
    if recipient_name:
        agents = _list_visible_agents(thread_id)
        lowered = recipient_name.casefold()
        matches = []
        for item in agents:
            display_name = _normalize_text(str(item.get("displayName") or ""))
            if display_name and display_name.casefold() == lowered:
                matches.append(item)
        if not matches:
            raise SystemExit(f"robdex: no visible thread named {_quoted(recipient_name)}")
        if len(matches) > 1:
            raise SystemExit(f"robdex: multiple visible threads named {_quoted(recipient_name)}; use --to-thread-id")
        resolved = _normalize_text(str(matches[0].get("id") or ""))
        if resolved:
            return resolved
    raise SystemExit("robdex: provide --name or --to-thread-id, or use --to-self")


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


def _resolve_requirements_status_from_snapshot(args: argparse.Namespace) -> dict[str, Any]:
    snapshot = _load_snapshot()
    state = snapshot.get("state")
    projects = state.get("projects") if isinstance(state, dict) else None
    if not isinstance(projects, dict):
        raise SystemExit("robdex: state snapshot does not include projects")
    target_thread_id = _normalize_text(args.to_thread_id)
    target_name = _normalize_text(args.name)
    target_project = _normalize_path(args.project_path)
    matches: list[tuple[str, dict[str, Any]]] = []
    for _, project in projects.items():
        if not isinstance(project, dict) or project.get("detached") is True:
            continue
        project_path = _normalize_path(str(project.get("projectRoot") or project.get("cwd") or ""))
        if target_project and project_path != target_project:
            continue
        agents = project.get("agents")
        if not isinstance(agents, dict):
            continue
        for agent_id, agent in agents.items():
            if not isinstance(agent, dict) or agent.get("archived") is True:
                continue
            display_name = _normalize_text(str(agent.get("displayName") or ""))
            if target_thread_id and str(agent_id) != target_thread_id:
                continue
            if target_name and (display_name or "").casefold() != target_name.casefold():
                continue
            matches.append((str(agent_id), agent))
    if not matches:
        target = target_thread_id or target_name or "(selected thread)"
        raise SystemExit(f"robdex: no tracked requirements recipient found in snapshot for {target}")
    if len(matches) > 1:
        target = target_thread_id or target_name or "(selected thread)"
        raise SystemExit(f"robdex: multiple requirements recipients found in snapshot for {target}; pass --to-thread-id")

    thread_id, agent = matches[0]
    requirements = agent.get("requirements") if isinstance(agent.get("requirements"), dict) else {}
    review = agent.get("requirementReview") if isinstance(agent.get("requirementReview"), dict) else {}
    requirement_items = requirements.get("requirements") if isinstance(requirements, dict) else []
    latest_verdict = review.get("latestVerdictPacket") if isinstance(review.get("latestVerdictPacket"), dict) else {}
    passed = failed = blocked = waiver = waived = pending = 0
    verdicts: list[dict[str, Any]] = []
    if isinstance(requirement_items, list):
        for requirement in requirement_items:
            if not isinstance(requirement, dict):
                continue
            key = _normalize_text(str(requirement.get("key") or ""))
            item = latest_verdict.get(key) if key and isinstance(latest_verdict, dict) else None
            item = item if isinstance(item, dict) else {}
            verdict = _normalize_text(str(item.get("verdict") or ""))
            if verdict == "pass":
                passed += 1
            elif verdict in ("fail", "rejectedBlocked"):
                failed += 1
            elif verdict == "acceptedBlocked":
                blocked += 1
            elif verdict == "waiverRequired":
                waiver += 1
            elif verdict == "waiverAccepted":
                waived += 1
            else:
                pending += 1
            verdicts.append(
                {
                    "key": key,
                    "verdict": verdict or None,
                    "reason": item.get("reason"),
                    "evidenceAssessment": item.get("evidenceAssessment"),
                    "requiredCorrection": item.get("requiredCorrection"),
                }
            )

    summary = {
        "activeRequirementCount": len(requirement_items) if isinstance(requirement_items, list) and requirements.get("active") is True else 0,
        "status": review.get("status"),
        "reviewerThreadId": review.get("reviewerThreadId"),
        "requirementSetId": review.get("requirementSetId") or requirements.get("id"),
        "passedCount": passed,
        "failedCount": failed,
        "blockedCount": blocked,
        "waiverRequiredCount": waiver,
        "waiverAcceptedCount": waived,
        "unknownCount": pending,
        "verdicts": verdicts,
    }
    return {
        "threadId": thread_id,
        "displayName": agent.get("displayName") or thread_id,
        "requirements": requirements,
        "requirementReview": review,
        "summary": summary,
    }


def _cmd_requirements_status(thread_id: str, args: argparse.Namespace) -> None:
    try:
        payload = _request_json(
            "POST",
            "/orchestrator/requirements/status",
            body={
                "senderThreadId": thread_id,
                "recipientThreadId": _normalize_text(args.to_thread_id),
                "recipientName": _normalize_text(args.name),
                "projectPath": _normalize_path(args.project_path),
            },
        )
    except SystemExit as exc:
        message = str(exc)
        if "is not tracked by the bridge" not in message:
            raise
        payload = _resolve_requirements_status_from_snapshot(args)
    summary = payload.get("summary") if isinstance(payload.get("summary"), dict) else {}
    review = payload.get("requirementReview") if isinstance(payload.get("requirementReview"), dict) else {}
    requirements = payload.get("requirements") if isinstance(payload.get("requirements"), dict) else {}
    requirement_items = requirements.get("requirements") if isinstance(requirements, dict) else []
    lines = [
        f"thread={payload.get('displayName') or payload.get('threadId') or 'unknown'}",
        f"status={summary.get('status') or review.get('status') or 'notStarted'}",
        f"active_requirements={summary.get('activeRequirementCount') or 0}",
        (
            "counts="
            f"pass:{summary.get('passedCount') or 0} "
            f"fail:{summary.get('failedCount') or 0} "
            f"blocked:{summary.get('blockedCount') or 0} "
            f"waiver:{summary.get('waiverRequiredCount') or 0} "
            f"waived:{summary.get('waiverAcceptedCount') or 0} "
            f"pending:{summary.get('unknownCount') or 0}"
        ),
    ]
    reviewer = _normalize_text(str(summary.get("reviewerThreadId") or review.get("reviewerThreadId") or ""))
    if reviewer:
        lines.append(f"reviewer_thread_id={reviewer}")
    requirement_set = _normalize_text(str(summary.get("requirementSetId") or ""))
    if requirement_set:
        lines.append(f"requirement_set_id={requirement_set}")
    verdicts = summary.get("verdicts") if isinstance(summary.get("verdicts"), list) else []
    verdict_by_key = {
        str(item.get("key")): item
        for item in verdicts
        if isinstance(item, dict) and _normalize_text(str(item.get("key") or ""))
    }
    if isinstance(requirement_items, list) and requirement_items:
        lines.append("")
        lines.append("Requirements:")
        for requirement in requirement_items:
            if not isinstance(requirement, dict):
                continue
            key = _normalize_text(str(requirement.get("key") or "")) or "unknown"
            statement = _normalize_text(str(requirement.get("statement") or "")) or "(no statement)"
            severity = _normalize_text(str(requirement.get("severity") or "")) or "medium"
            verification = _normalize_text(str(requirement.get("verificationMethod") or "")) or "manualEvidence"
            verdict = verdict_by_key.get(key, {}).get("verdict") or "pending"
            lines.append(f"- {key}: {verdict} [{severity}; {verification}] {statement}")
            details = verdict_by_key.get(key, {})
            for label, field in (
                ("reason", "reason"),
                ("evidence", "evidenceAssessment"),
                ("required_correction", "requiredCorrection"),
            ):
                value = _normalize_text(str(details.get(field) or ""))
                if value:
                    lines.append(f"  {label}: {value}")
    else:
        lines.append("No active requirements.")
    _print_lines(lines)


def _cmd_requirements_from_prose(
    thread_id: str,
    args: argparse.Namespace,
    parser: argparse.ArgumentParser,
) -> None:
    text = _resolve_text_input(
        parser,
        args,
        inline_attr="text",
        file_attr="text_file",
        stdin_attr="text_stdin",
        label="requirements prose",
    )
    requirement_set = _requirements_from_prose(args.title, text)
    composables = _selected_composable_items(thread_id, args)
    if composables:
        requirement_set = _compose_requirements_payload(
            title=args.title,
            base_payload=requirement_set,
            composables=composables,
        )
    if not (args.attach or args.interrupt or args.to_self):
        print(json.dumps(requirement_set, indent=2, sort_keys=True))
        return
    _apply_requirements_set(
        thread_id,
        args,
        parser,
        requirement_set,
        command_name="requirements-from-prose",
    )


def _requirements_composables_payload(thread_id: str, args: argparse.Namespace) -> dict[str, Any]:
    return _request_json(
        "POST",
        "/orchestrator/requirements/composables",
        body={
            "senderThreadId": thread_id,
            "recipientThreadId": _normalize_text(getattr(args, "to_thread_id", None)),
            "recipientName": _normalize_text(getattr(args, "name", None)),
            "projectPath": _normalize_path(getattr(args, "project_path", None)),
        },
    )


def _cmd_requirements_composables_list(thread_id: str, args: argparse.Namespace) -> None:
    payload = _requirements_composables_payload(thread_id, args)
    items = payload.get("items")
    if not isinstance(items, list):
        return
    lines: list[str] = []
    for item in items:
        if not isinstance(item, dict):
            continue
        composable_id = _normalize_text(str(item.get("id") or "")) or "unknown"
        title = _normalize_text(str(item.get("title") or "")) or composable_id
        scope = _normalize_text(str(item.get("scope") or "")) or "unknown"
        count = item.get("requirementCount")
        description = _normalize_text(str(item.get("description") or ""))
        permanent = " | permanent" if bool(item.get("permanent")) else ""
        line = f"{composable_id} | {scope} | count={count}{permanent} | {title}"
        if description:
            line += f" | {description}"
        lines.append(line)
    _print_lines(lines)


def _cmd_requirements_composables_show(thread_id: str, args: argparse.Namespace) -> None:
    payload = _requirements_composables_payload(thread_id, args)
    items = payload.get("items")
    if not isinstance(items, list):
        raise SystemExit("robdex: composables response missing items")
    wanted = args.composable_id
    for item in items:
        if isinstance(item, dict) and item.get("id") == wanted:
            print(json.dumps(item, indent=2, sort_keys=True))
            return
    raise SystemExit(f"robdex: unknown requirements composable {wanted!r}")


def _manifest_post(thread_id: str, action: str, body: dict[str, Any]) -> dict[str, Any]:
    payload = {"senderThreadId": thread_id}
    payload.update(body)
    return _request_json("POST", f"/orchestrator/manifest/{action}", body=payload)


def _cmd_manifest_activate(thread_id: str, args: argparse.Namespace) -> None:
    payload = _manifest_post(thread_id, "activate", {"file": _normalize_path(args.file)})
    run_id = _normalize_text(str(payload.get("runId") or "")) or "unknown-run"
    phase_id = _normalize_text(str(payload.get("currentPhaseId") or "")) or "unknown-phase"
    worker = _normalize_text(str(payload.get("workerThreadId") or "")) or "unknown-worker"
    print(f"Activated manifest {run_id} | phase={phase_id} | worker={worker}")


def _cmd_manifest_status(thread_id: str, args: argparse.Namespace) -> None:
    payload = _manifest_post(
        thread_id,
        "status",
        {
            "projectPath": _normalize_path(args.project_path),
            "runId": _normalize_text(args.run_id),
        },
    )
    runs = payload.get("runs")
    if not isinstance(runs, list) or not runs:
        print("No manifest runs.")
        return
    lines: list[str] = []
    for run in runs:
        if not isinstance(run, dict):
            continue
        run_id = _normalize_text(str(run.get("runId") or "")) or "unknown-run"
        plan_id = _normalize_text(str(run.get("planId") or "")) or "unknown-plan"
        status = _normalize_text(str(run.get("status") or "")) or "unknown"
        current = _normalize_text(str(run.get("currentPhaseId") or "")) or "-"
        lines.append(f"{run_id} | plan={plan_id} | status={status} | current={current}")
        phases = run.get("phases")
        if isinstance(phases, list):
            for phase in phases:
                if not isinstance(phase, dict):
                    continue
                phase_id = _normalize_text(str(phase.get("phaseId") or "")) or "unknown-phase"
                phase_status = _normalize_text(str(phase.get("status") or "")) or "unknown"
                worker = _normalize_text(str(phase.get("workerThreadId") or "")) or "-"
                cleanup = _normalize_text(str(phase.get("archiveCleanupState") or "")) or "-"
                lines.append(f"  - {phase_id}: {phase_status} | worker={worker} | cleanup={cleanup}")
    _print_lines(lines)


def _cmd_manifest_advance(thread_id: str, args: argparse.Namespace) -> None:
    payload = _manifest_post(
        thread_id,
        "advance",
        {
            "runId": args.run_id,
            "handoffFile": _normalize_path(args.handoff_file),
        },
    )
    phase = _normalize_text(str(payload.get("advancedPhaseId") or "")) or "unknown-phase"
    archived = _normalize_text(str(payload.get("archivedWorkerThreadId") or "")) or "unknown-worker"
    cleanup = _normalize_text(str(payload.get("archiveCleanupState") or "")) or "unknown"
    next_worker = _normalize_text(str(payload.get("nextWorkerThreadId") or "")) or "-"
    print(f"Advanced manifest | phase={phase} | archived={archived} | cleanup={cleanup} | nextWorker={next_worker}")


def _cmd_manifest_cancel(thread_id: str, args: argparse.Namespace) -> None:
    payload = _manifest_post(
        thread_id,
        "cancel",
        {
            "runId": args.run_id,
            "reason": _normalize_text(args.reason),
        },
    )
    run = payload.get("run") if isinstance(payload.get("run"), dict) else {}
    run_id = _normalize_text(str(run.get("runId") or "")) or args.run_id
    status = _normalize_text(str(run.get("status") or "")) or "cancelled"
    print(f"Manifest {run_id} is {status}.")


def _cmd_manifest_decision(thread_id: str, args: argparse.Namespace) -> None:
    text = _resolve_text_input(
        argparse.ArgumentParser(prog="robdex manifest decision"),
        args,
        inline_attr="text",
        file_attr="text_file",
        stdin_attr="text_stdin",
        label="manifest decision",
    )
    payload = _manifest_post(
        thread_id,
        "decision",
        {
            "runId": args.run_id,
            "phaseId": args.phase_id,
            "type": args.type,
            "text": text,
        },
    )
    run = payload.get("run") if isinstance(payload.get("run"), dict) else {}
    run_id = _normalize_text(str(run.get("runId") or "")) or args.run_id
    print(f"Recorded manifest {args.type} decision for {run_id} phase {args.phase_id}.")


def _cmd_requirements_compose(thread_id: str, args: argparse.Namespace) -> None:
    base_payload = _read_json_file(args.requirements_file, "requirements")
    composables = _selected_composable_items(thread_id, args)
    if composables:
        requirement_set = _compose_requirements_payload(
            title=args.title,
            base_payload=base_payload,
            composables=composables,
        )
    else:
        requirement_set = base_payload
    if not args.attach:
        print(json.dumps(requirement_set, indent=2, sort_keys=True))
        return
    payload = _request_json(
        "POST",
        "/orchestrator/requirements/set",
        body={
            "senderThreadId": thread_id,
            "recipientThreadId": _normalize_text(args.to_thread_id),
            "recipientName": _normalize_text(args.name),
            "projectPath": _normalize_path(args.project_path),
            "requirementSet": requirement_set,
        },
    )
    print(
        "Set requirements for "
        f"{_quoted(str(payload.get('displayName') or payload.get('threadId') or 'unknown'))} "
        f"| count={payload.get('requirementCount')} enforceOnTurns={payload.get('enforceOnTurns')}"
    )


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


def _cmd_spawn_agent(thread_id: str, args: argparse.Namespace, parser: argparse.ArgumentParser) -> None:
    if args.prompt_file and args.prompt_stdin:
        parser.error("spawn-agent accepts either --prompt-file or --prompt-stdin")
    if args.prompt_file:
        try:
            prompt = Path(args.prompt_file).expanduser().read_text(encoding="utf-8").strip()
        except OSError as exc:
            raise SystemExit(f"robdex: unable to read spawn prompt file: {exc}") from exc
    elif args.prompt_stdin:
        prompt = sys.stdin.read().strip()
    else:
        parser.error("spawn prompt input is required; use --prompt-file or --prompt-stdin")
        raise AssertionError("unreachable")
    if not prompt:
        raise SystemExit("robdex: spawn prompt is empty")

    if args.requirements_json and args.requirements_file:
        parser.error("spawn-agent accepts either --requirements-json or --requirements-file")

    requirement_set = None
    requirements_path = args.requirements_json or args.requirements_file
    if requirements_path:
        requirement_set = _read_json_file(requirements_path, "requirements")

    payload = _request_json(
        "POST",
        "/orchestrator/spawn-agent",
        body={
            "senderThreadId": thread_id,
            "name": args.name,
            "prompt": prompt,
            "cwd": _normalize_path(args.cwd),
            "role": _normalize_text(args.role),
            "issueNumber": args.issue_number,
            "requirementSet": requirement_set,
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


def _interrupt_thread(thread_id: str) -> None:
    encoded_thread_id = urllib.parse.quote(thread_id, safe="")
    _request_json("POST", f"/threads/{encoded_thread_id}/interrupt")


def _send_message_to_thread(sender_thread_id: str, recipient_thread_id: str, text: str) -> None:
    _request_json(
        "POST",
        "/orchestrator/agent-message",
        body={
            "senderThreadId": sender_thread_id,
            "recipientThreadId": recipient_thread_id,
            "recipientName": None,
            "text": text,
        },
    )


def _set_requirements_payload(
    *,
    sender_thread_id: str,
    recipient_thread_id: str | None,
    recipient_name: str | None,
    project_path: str | None,
    requirement_set: Any,
) -> dict[str, Any]:
    return _request_json(
        "POST",
        "/orchestrator/requirements/set",
        body={
            "senderThreadId": sender_thread_id,
            "recipientThreadId": recipient_thread_id,
            "recipientName": recipient_name,
            "projectPath": project_path,
            "requirementSet": requirement_set,
        },
    )


def _print_set_requirements_summary(payload: dict[str, Any]) -> None:
    print(
        "Set requirements for "
        f"{_quoted(str(payload.get('displayName') or payload.get('threadId') or 'unknown'))} "
        f"| count={payload.get('requirementCount')} enforceOnTurns={payload.get('enforceOnTurns')}"
    )


def _apply_requirements_set(
    thread_id: str,
    args: argparse.Namespace,
    parser: argparse.ArgumentParser,
    requirement_set: Any,
    *,
    command_name: str,
) -> None:
    if args.to_self and (args.to_thread_id or args.name or args.project_path):
        parser.error("--to-self cannot be combined with --name, --to-thread-id, or --project-path")
    if not args.to_self and not (args.to_thread_id or args.name):
        parser.error(f"{command_name} requires --name or --to-thread-id unless --to-self is provided")

    if args.to_self:
        payload = _set_requirements_payload(
            sender_thread_id=thread_id,
            recipient_thread_id=thread_id,
            recipient_name=None,
            project_path=None,
            requirement_set=requirement_set,
        )
        time.sleep(0.25)
        _interrupt_thread(thread_id)
        _send_message_to_thread(thread_id, thread_id, "Begin")
        _print_set_requirements_summary(payload)
        print(f"Interrupted {_quoted(thread_id)} and sent {_quoted('Begin')}")
        return

    project_path = _normalize_path(args.project_path)
    recipient_thread_id = _normalize_text(args.to_thread_id)
    recipient_name = _normalize_text(args.name)
    if args.interrupt:
        recipient_thread_id = _resolve_recipient_thread_id(
            thread_id=thread_id,
            recipient_thread_id=recipient_thread_id,
            recipient_name=recipient_name,
            project_path=project_path,
        )
        if recipient_thread_id == thread_id:
            parser.error("--interrupt cannot target the current thread; use --to-self")
        recipient_name = None
        _interrupt_thread(recipient_thread_id)

    payload = _set_requirements_payload(
        sender_thread_id=thread_id,
        recipient_thread_id=recipient_thread_id,
        recipient_name=recipient_name,
        project_path=project_path,
        requirement_set=requirement_set,
    )
    if args.interrupt:
        _send_message_to_thread(thread_id, recipient_thread_id or "", "Requirements updated")
    _print_set_requirements_summary(payload)
    if args.interrupt:
        print(f"Interrupted {_quoted(recipient_thread_id or 'unknown')} and sent {_quoted('Requirements updated')}")


def _cmd_set_requirements(thread_id: str, args: argparse.Namespace, parser: argparse.ArgumentParser) -> None:
    requirements_payload = _read_json_file(args.requirements_file, "requirements")
    include_composables = _selected_composables(args)
    if include_composables and isinstance(requirements_payload, dict):
        requirements_payload = dict(requirements_payload)
        requirements_payload["includeComposables"] = include_composables
    elif include_composables:
        parser.error("--include-composable requires an object requirements file, not a raw array")

    _apply_requirements_set(
        thread_id,
        args,
        parser,
        requirements_payload,
        command_name="set-requirements",
    )


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
    p_spawn.add_argument(
        "--prompt",
        action="store_true",
        help=argparse.SUPPRESS,
    )
    p_spawn.add_argument("--prompt-file")
    p_spawn.add_argument("--prompt-stdin", action="store_true")
    p_spawn.add_argument("--cwd")
    p_spawn.add_argument("--role", choices=["worker", "qa", "hidden", "requirements-reviewer"], default="worker")
    p_spawn.add_argument("--issue-number", type=int)
    p_spawn.add_argument(
        "--requirements-json",
        help="Path to a RequirementSet JSON file to attach before the first turn starts.",
    )
    p_spawn.add_argument(
        "--requirements-file",
        help="Alias for --requirements-json.",
    )

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

    p_req_set = sub.add_parser(
        "set-requirements",
        help="Attach a requirements JSON file to an agent/thread.",
        epilog='Example: robdex set-requirements --name "Ezra Worker 1A" --requirements-file /tmp/requirements.json',
    )
    p_req_set.add_argument("--to-thread-id")
    p_req_set.add_argument("--name")
    p_req_set.add_argument("--project-path")
    p_req_set.add_argument("--requirements-file", required=True)
    target_mode = p_req_set.add_mutually_exclusive_group()
    target_mode.add_argument(
        "--interrupt",
        action="store_true",
        help="Interrupt the target thread, set requirements, then send 'Requirements updated'.",
    )
    target_mode.add_argument(
        "--to-self",
        action="store_true",
        help="Set requirements on this thread, briefly delay, interrupt this thread, then send 'Begin'.",
    )
    p_req_set.add_argument(
        "--include-composable",
        action="append",
        default=[],
        help="Composable requirement id to merge before task-specific requirements. May be repeated or comma-separated.",
    )

    p_req_status = sub.add_parser(
        "requirements-status",
        help="Print active requirements and latest claim/review verdict state.",
        epilog='Example: robdex requirements-status --name "Ezra Worker 1A"',
    )
    p_req_status.add_argument("--to-thread-id")
    p_req_status.add_argument("--name")
    p_req_status.add_argument("--project-path")

    p_req_from_prose = sub.add_parser(
        "requirements-from-prose",
        help="Convert requirement-like prose into a RequirementSet JSON, optionally attaching it.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Formatting:\n"
            "  The parser turns each non-empty bullet, numbered item, or line into one requirement.\n"
            "  Put one complete requirement per line. Do not pack multiple requirements into one paragraph.\n"
            "  Do not include headings like 'Requirements:' in stdin; headings become accidental requirements.\n"
            "  Avoid paragraph blobs. Split obligations so each line has one reviewable completion contract.\n"
            "\n"
            "Good stdin format:\n"
            "  - Preserve existing behavior around the changed area unless explicitly assigned otherwise.\n"
            "  - Add targeted tests proving the new command behavior.\n"
            "  - Update active docs that discuss the changed command.\n"
            "  - Remove obsolete fallback paths, aliases, flags, docs, tests, and UI affordances rather than leaving legacy behavior behind.\n"
            "\n"
            "Bad stdin format:\n"
            "  Requirements:\n"
            "  Implement the parser refactor. Also update tests and docs. If legacy tests fail, keep the old parser for now.\n"
            "\n"
            "Why bad:\n"
            "  The heading becomes a requirement, several obligations collapse into one paragraph blob, and the legacy fallback weakens no-legacy policy.\n"
            "\n"
            "Preview example:\n"
            "  robdex requirements-from-prose --title \"Design gate\" --text-stdin <<'EOF'\n"
            "  - Match the approved reference image without fake UI or dead controls.\n"
            "  - Provide Design Lab screenshot evidence from the sanctioned capture path.\n"
            "  EOF\n"
            "\n"
            "Attach with composables:\n"
            "  robdex requirements-from-prose --title \"Design gate\" --include-composable design-non-negotiables --text-stdin --attach --name \"Worker\" <<'EOF'\n"
            "  - Match the approved reference image without fake UI or dead controls.\n"
            "  EOF\n"
            "\n"
            "No-legacy cleanup example:\n"
            "  robdex requirements-from-prose --title \"Parser replacement\" --include-composable no-legacy --text-stdin --attach --name \"Worker\" <<'EOF'\n"
            "  - Replace the old parser with the approved new parser as the only runtime parser path.\n"
            "  - Update or remove tests that only preserve obsolete parser behavior while preserving documented current CLI behavior.\n"
            "  - Provide exact command evidence for the real CLI path and exact changed-file evidence for removed legacy affordances.\n"
            "  EOF\n"
            "\n"
            "Never run --text-stdin without a heredoc, pipe, or redirected file attached."
        ),
    )
    p_req_from_prose.add_argument("--title", required=True)
    prose_input = p_req_from_prose.add_mutually_exclusive_group(required=True)
    prose_input.add_argument("--text")
    prose_input.add_argument("--text-file")
    prose_input.add_argument("--text-stdin", action="store_true")
    prose_apply_mode = p_req_from_prose.add_mutually_exclusive_group()
    prose_apply_mode.add_argument("--attach", action="store_true")
    prose_apply_mode.add_argument(
        "--interrupt",
        action="store_true",
        help="Interrupt the target thread, set generated requirements, then send 'Requirements updated'.",
    )
    prose_apply_mode.add_argument(
        "--to-self",
        action="store_true",
        help="Set generated requirements on this thread, briefly delay, interrupt this thread, then send 'Begin'.",
    )
    p_req_from_prose.add_argument("--to-thread-id")
    p_req_from_prose.add_argument("--name")
    p_req_from_prose.add_argument("--project-path")
    p_req_from_prose.add_argument(
        "--include-composable",
        action="append",
        default=[],
        help="Composable requirement id to merge before generated prose requirements. May be repeated or comma-separated.",
    )

    p_req_composables = sub.add_parser(
        "requirements-composables",
        help="List or inspect composable Requirements available to the selected recipient.",
    )
    p_req_composables.add_argument("action", choices=["list", "show"])
    p_req_composables.add_argument("composable_id", nargs="?")
    p_req_composables.add_argument("--to-thread-id")
    p_req_composables.add_argument("--name")
    p_req_composables.add_argument("--project-path")

    p_req_compose = sub.add_parser(
        "requirements-compose",
        help="Compose selected composables with a task-specific RequirementSet JSON.",
    )
    p_req_compose.add_argument("--title", required=True)
    p_req_compose.add_argument("--requirements-file", required=True)
    p_req_compose.add_argument("--include-composable", action="append", default=[])
    p_req_compose.add_argument("--attach", action="store_true")
    p_req_compose.add_argument("--to-thread-id")
    p_req_compose.add_argument("--name")
    p_req_compose.add_argument("--project-path")

    p_manifest = sub.add_parser(
        "manifest",
        help="Manage file-backed serial Robdex manifests.",
    )
    manifest_sub = p_manifest.add_subparsers(dest="manifest_cmd", required=True)

    p_manifest_activate = manifest_sub.add_parser(
        "activate",
        help="Activate a Markdown manifest from PROJECT/.codex/manifests/.",
    )
    p_manifest_activate.add_argument("--file", required=True)

    p_manifest_status = manifest_sub.add_parser(
        "status",
        help="Show manifest runs and phase state.",
    )
    p_manifest_status.add_argument("--project-path")
    p_manifest_status.add_argument("--run-id")

    p_manifest_advance = manifest_sub.add_parser(
        "advance",
        help="Advance the current phase after passed Requirements review and handoff.",
    )
    p_manifest_advance.add_argument("--run-id", required=True)
    p_manifest_advance.add_argument("--handoff-file", required=True)

    p_manifest_cancel = manifest_sub.add_parser(
        "cancel",
        help="Cancel an active manifest run and stop future phase materialization.",
    )
    p_manifest_cancel.add_argument("--run-id", required=True)
    p_manifest_cancel.add_argument("--reason")

    p_manifest_decision = manifest_sub.add_parser(
        "decision",
        help="Record a durable blocker, waiver, or resume decision for a manifest phase.",
    )
    p_manifest_decision.add_argument("--run-id", required=True)
    p_manifest_decision.add_argument("--phase-id", required=True)
    p_manifest_decision.add_argument("--type", choices=["blocker", "waiver", "resume"], required=True)
    decision_text = p_manifest_decision.add_mutually_exclusive_group(required=True)
    decision_text.add_argument("--text")
    decision_text.add_argument("--text-file")
    decision_text.add_argument("--text-stdin", action="store_true")

    p_handoff = _add_handoff_parser(sub)

    p_approve = sub.add_parser("approve-approval", help="disabled: approval-based command execution is not allowed")
    p_approve.add_argument("--approval-id", required=True)

    p_decline = sub.add_parser("decline-approval")
    p_decline.add_argument("--approval-id", required=True)
    p_decline.add_argument("--message")

    return parser, p_handoff, p_spawn


def main() -> int:
    parser, handoff_parser, spawn_parser = build_parser()
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
        if args.prompt:
            raise SystemExit("robdex: --prompt no longer exists for spawn-agent; use --prompt-file or --prompt-stdin")
        _cmd_spawn_agent(thread_id, args, spawn_parser)
    elif args.cmd == "archive-agent":
        _cmd_archive_agent(thread_id, args)
    elif args.cmd == "rename-agent":
        _cmd_rename_agent(thread_id, args)
    elif args.cmd == "send-message":
        _cmd_send_message(thread_id, args, parser)
    elif args.cmd == "set-worker-metadata":
        _cmd_set_worker_metadata(thread_id, args)
    elif args.cmd == "set-requirements":
        _cmd_set_requirements(thread_id, args, parser)
    elif args.cmd == "requirements-status":
        _cmd_requirements_status(thread_id, args)
    elif args.cmd == "requirements-from-prose":
        _cmd_requirements_from_prose(thread_id, args, parser)
    elif args.cmd == "requirements-composables":
        if args.action == "show" and not args.composable_id:
            parser.error("requirements-composables show requires composable_id")
        if args.action == "list":
            _cmd_requirements_composables_list(thread_id, args)
        else:
            _cmd_requirements_composables_show(thread_id, args)
    elif args.cmd == "requirements-compose":
        _cmd_requirements_compose(thread_id, args)
    elif args.cmd == "manifest":
        if args.manifest_cmd == "activate":
            _cmd_manifest_activate(thread_id, args)
        elif args.manifest_cmd == "status":
            _cmd_manifest_status(thread_id, args)
        elif args.manifest_cmd == "advance":
            _cmd_manifest_advance(thread_id, args)
        elif args.manifest_cmd == "cancel":
            _cmd_manifest_cancel(thread_id, args)
        elif args.manifest_cmd == "decision":
            _cmd_manifest_decision(thread_id, args)
        else:
            parser.error(f"unknown manifest command: {args.manifest_cmd}")
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
