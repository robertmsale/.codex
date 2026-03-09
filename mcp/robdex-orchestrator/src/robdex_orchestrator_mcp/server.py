from __future__ import annotations

import asyncio
import json
import os
import subprocess
import threading
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import websockets
from fastmcp import Context, FastMCP

CONTINUATION_SUFFIX = "Continue working unless told explicitly to stop, and respond to this message using $robdex-orchestrator only if necessary."
DEFAULT_INSTANCE_ID = "mgmt-global"
ROBDEX_STATE_FILE = Path.home() / ".codex" / "robdex" / "robdex.json"
ROBDEX_TOKEN_FILE = Path(__file__).resolve().parents[2] / ".bridge-token"
SESSION_THREAD_LOCKS: dict[str, str] = {}
SESSION_THREAD_LOCKS_MUTEX = threading.Lock()

mcp = FastMCP("robdex-orchestrator-mcp")


class BridgeError(RuntimeError):
    pass


@dataclass(frozen=True)
class Context:
    host: str
    port: int
    token: str | None
    instance_id: str
    current_thread_id: str | None
    current_project_path: str | None
    current_is_orchestrator: bool
    titles_by_thread_id: dict[str, str]
    orchestrator_by_project: dict[str, str]


@dataclass(frozen=True)
class ThreadEntry:
    id: str
    cwd: str | None
    preview: str | None
    display_name: str
    project_path: str | None
    has_custom_title: bool
    issue_number: int | None = None
    pull_request_number: int | None = None
    blocked_reason: str | None = None
    unblock_when: str | None = None
    hidden: bool = False


@dataclass(frozen=True)
class AgentEntry:
    id: str
    instance_id: str | None
    thread_id: str | None
    status: str
    project_path: str | None


@dataclass(frozen=True)
class InstanceEntry:
    id: str


def _normalize_text(value: str | None) -> str | None:
    if value is None:
        return None
    trimmed = value.strip()
    return trimmed or None


def _normalized_path(value: str | None) -> str | None:
    trimmed = _normalize_text(value)
    if not trimmed:
        return None
    return str(Path(trimmed).expanduser().resolve())


def _path_is_within_project(path: str, project_path: str) -> bool:
    try:
        Path(path).relative_to(Path(project_path))
    except ValueError:
        return False
    return True


def _normalized_title(value: str) -> str:
    return value.strip().lower()


def _quoted(value: str) -> str:
    return json.dumps(value)


def _read_bridge_token_from_keychain() -> str | None:
    command = [
        "security",
        "find-generic-password",
        "-s",
        "com.robertmsale.robdex.bridge",
        "-a",
        "bridge-auth-token",
        "-w",
    ]
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            check=False,
            timeout=2.0,
        )
    except Exception:
        return None
    if completed.returncode != 0:
        return None
    return _normalize_text(completed.stdout)


def _read_bridge_token_from_file() -> str | None:
    try:
        text = ROBDEX_TOKEN_FILE.read_text(encoding="utf-8")
    except Exception:
        return None
    return _normalize_text(text)


def _load_state() -> tuple[dict[str, str], dict[str, str], str | None]:
    if not ROBDEX_STATE_FILE.exists():
        return {}, {}, None
    try:
        payload = json.loads(ROBDEX_STATE_FILE.read_text(encoding="utf-8", errors="replace"))
    except Exception as exc:  # noqa: BLE001
        raise BridgeError(f"Failed to parse {ROBDEX_STATE_FILE}: {exc}") from exc

    if not isinstance(payload, dict):
        return {}, {}, None

    titles: dict[str, str] = {}
    metadata = payload.get("threadMetadataByID", {})
    if isinstance(metadata, dict):
        for thread_id, raw in metadata.items():
            thread_key = _normalize_text(str(thread_id))
            if not thread_key:
                continue
            if isinstance(raw, str):
                title = _normalize_text(raw)
            elif isinstance(raw, dict):
                title = _normalize_text(raw.get("displayName"))
            else:
                title = None
            if title:
                titles[thread_key] = title

    orchestrators: dict[str, str] = {}
    raw_orchestrators = payload.get("orchestratorThreadIDByProjectPath", {})
    if isinstance(raw_orchestrators, dict):
        for project_path, thread_id in raw_orchestrators.items():
            project = _normalized_path(str(project_path))
            thread = _normalize_text(str(thread_id))
            if project and thread:
                orchestrators[project] = thread

    global_orchestrator = _normalize_text(payload.get("orchestratorThreadID"))
    return titles, orchestrators, global_orchestrator


def _load_state_payload() -> dict[str, Any]:
    if not ROBDEX_STATE_FILE.exists():
        return {}
    try:
        payload = json.loads(ROBDEX_STATE_FILE.read_text(encoding="utf-8", errors="replace"))
    except Exception as exc:  # noqa: BLE001
        raise BridgeError(f"Failed to parse {ROBDEX_STATE_FILE}: {exc}") from exc
    if not isinstance(payload, dict):
        raise BridgeError(f"Invalid robdex state payload in {ROBDEX_STATE_FILE}")
    return payload


def _write_state_payload(payload: dict[str, Any]) -> None:
    ROBDEX_STATE_FILE.parent.mkdir(parents=True, exist_ok=True)
    ROBDEX_STATE_FILE.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _load_thread_metadata_map() -> dict[str, dict[str, Any]]:
    payload = _load_state_payload()
    metadata = payload.get("threadMetadataByID")
    if not isinstance(metadata, dict):
        return {}
    result: dict[str, dict[str, Any]] = {}
    for thread_id, raw in metadata.items():
        normalized_id = _normalize_text(str(thread_id))
        if not normalized_id or not isinstance(raw, dict):
            continue
        result[normalized_id] = dict(raw)
    return result


def _set_thread_metadata_fields(thread_id: str, updates: dict[str, Any | None]) -> None:
    payload = _load_state_payload()
    metadata = payload.get("threadMetadataByID")
    if not isinstance(metadata, dict):
        metadata = {}
        payload["threadMetadataByID"] = metadata

    current_raw = metadata.get(thread_id)
    current = dict(current_raw) if isinstance(current_raw, dict) else {}

    for key, value in updates.items():
        if value is None:
            current.pop(key, None)
        else:
            current[key] = value

    metadata[thread_id] = current
    _write_state_payload(payload)


def _resolve_session_thread_id(
    *,
    from_thread_id: str,
    tool_context: Context,
) -> str:
    provided_thread_id = _normalize_text(from_thread_id)
    if not provided_thread_id:
        raise BridgeError("from_thread_id is required. Use `echo \"$CODEX_THREAD_ID\"` and pass that value.")

    environment_thread_id = _normalize_text(os.getenv("CODEX_THREAD_ID"))
    if environment_thread_id and environment_thread_id != provided_thread_id:
        raise BridgeError(
            f"from_thread_id {_quoted(provided_thread_id)} does not match this thread identity {_quoted(environment_thread_id)}."
        )

    try:
        session_id = tool_context.session_id
    except Exception:
        session_id = "unknown-session"

    with SESSION_THREAD_LOCKS_MUTEX:
        locked_thread_id = SESSION_THREAD_LOCKS.get(session_id)
        if locked_thread_id:
            if locked_thread_id != provided_thread_id:
                raise BridgeError(
                    f"Session is locked to from_thread_id {_quoted(locked_thread_id)}; refusing {_quoted(provided_thread_id)}."
                )
            return locked_thread_id

        SESSION_THREAD_LOCKS[session_id] = provided_thread_id
        return provided_thread_id


async def _run_command_async(
    host: str,
    port: int,
    token: str | None,
    *,
    name: str,
    payload: dict[str, Any] | None = None,
    timeout_seconds: float = 30.0,
) -> dict[str, Any]:
    command_id = str(uuid.uuid4())
    url = f"ws://{host}:{port}/ws"
    connect_kwargs: dict[str, Any] = {
        "max_size": None,
        "open_timeout": 20,
    }
    if token:
        connect_kwargs["additional_headers"] = {"Authorization": f"Bearer {token}"}

    async with websockets.connect(url, **connect_kwargs) as socket:
        hello = {
            "type": "hello",
            "payload": {
                "protocolVersion": 1,
                "clientName": "robdex-orchestrator-mcp",
                "clientVersion": "0.1.0",
                "deviceName": os.uname().nodename,
            },
        }
        await socket.send(json.dumps(hello, separators=(",", ":"), sort_keys=True))

        command: dict[str, Any] = {"name": name}
        if payload:
            command["payload"] = payload

        envelope = {
            "type": "command",
            "payload": {
                "id": command_id,
                "command": command,
            },
        }
        await socket.send(json.dumps(envelope, separators=(",", ":"), sort_keys=True))

        while True:
            try:
                raw = await asyncio.wait_for(socket.recv(), timeout=timeout_seconds)
            except TimeoutError as exc:
                raise BridgeError(f"Bridge command timed out: {name}") from exc

            if isinstance(raw, bytes):
                text = raw.decode("utf-8", errors="replace")
            else:
                text = raw

            try:
                root = json.loads(text)
            except json.JSONDecodeError:
                continue

            if not isinstance(root, dict) or root.get("type") != "event":
                continue
            event_payload = root.get("payload")
            if isinstance(event_payload, dict):
                nested_event = event_payload.get("event")
                if isinstance(nested_event, dict):
                    event_payload = nested_event
            if not isinstance(event_payload, dict) or event_payload.get("name") != "commandResult":
                continue
            data = event_payload.get("data")
            if not isinstance(data, dict) or data.get("id") != command_id:
                continue

            error_message = _normalize_text(data.get("errorMessage"))
            if error_message:
                raise BridgeError(error_message)

            result_payload = data.get("payload")
            if not isinstance(result_payload, dict):
                raise BridgeError(f"Bridge command returned invalid payload: {name}")
            return result_payload


def _run_command(
    host: str,
    port: int,
    token: str | None,
    *,
    name: str,
    payload: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return asyncio.run(_run_command_async(host, port, token, name=name, payload=payload))


def _parse_agents(result: dict[str, Any]) -> list[AgentEntry]:
    if result.get("type") != "agents":
        raise BridgeError("Bridge response was not agents payload")
    payload = result.get("payload")
    if not isinstance(payload, list):
        raise BridgeError("Bridge agents payload malformed")

    agents: list[AgentEntry] = []
    for entry in payload:
        if not isinstance(entry, dict):
            continue
        agent_id = _normalize_text(entry.get("id"))
        if not agent_id:
            continue
        agents.append(
            AgentEntry(
                id=agent_id,
                instance_id=_normalize_text(entry.get("instanceId")),
                thread_id=_normalize_text(entry.get("threadId")),
                status=_normalize_text(entry.get("status")) or "unknown",
                project_path=_normalized_path(entry.get("projectPath")),
            )
        )
    return agents


def _parse_instances(result: dict[str, Any]) -> list[InstanceEntry]:
    if result.get("type") != "instances":
        raise BridgeError("Bridge response was not instances payload")
    payload = result.get("payload")
    if not isinstance(payload, list):
        raise BridgeError("Bridge instances payload malformed")

    instances: list[InstanceEntry] = []
    for entry in payload:
        if not isinstance(entry, dict):
            continue
        instance_id = _normalize_text(entry.get("id"))
        if not instance_id:
            continue
        instances.append(InstanceEntry(id=instance_id))
    return instances


def _is_active_turn_changed_error(message: str) -> bool:
    lowered = str(message or "").lower()
    return (
        ("expected active turn id" in lowered and "but found" in lowered)
        or ("missing field" in lowered and "expectedturnid" in lowered)
    )


def _is_instance_not_found_error(message: str) -> bool:
    return "instancenotfound(" in str(message or "").lower()


def _resolve_live_instance_id(host: str, port: int, token: str | None, preferred_instance_id: str) -> str:
    try:
        instances_result = _run_command(host, port, token, name="listInstances")
        instances = _parse_instances(instances_result)
    except Exception:
        return preferred_instance_id

    if not instances:
        return preferred_instance_id

    instance_ids = {instance.id for instance in instances}
    if preferred_instance_id in instance_ids:
        return preferred_instance_id
    if DEFAULT_INSTANCE_ID in instance_ids:
        return DEFAULT_INSTANCE_ID

    first_non_management = next(
        (instance.id for instance in instances if not instance.id.startswith("mgmt-")),
        None,
    )
    if first_non_management:
        return first_non_management

    return instances[0].id


def _run_instance_command(
    host: str,
    port: int,
    token: str | None,
    *,
    name: str,
    payload: dict[str, Any],
) -> dict[str, Any]:
    try:
        return _run_command(host, port, token, name=name, payload=payload)
    except BridgeError as exc:
        if not _is_instance_not_found_error(str(exc)):
            raise
        original_error = exc

    original_instance_id = _normalize_text(payload.get("instanceId"))
    if not original_instance_id:
        raise original_error

    fallback_instance_id = _resolve_live_instance_id(host, port, token, DEFAULT_INSTANCE_ID)
    if fallback_instance_id == original_instance_id:
        raise original_error

    recovered_payload = dict(payload)
    recovered_payload["instanceId"] = fallback_instance_id
    return _run_command(host, port, token, name=name, payload=recovered_payload)


def _parse_thread_list(result: dict[str, Any], titles_by_thread_id: dict[str, str], orchestrator_by_project: dict[str, str], current_project: str | None) -> list[ThreadEntry]:
    if result.get("type") != "threadList":
        raise BridgeError("Bridge response was not threadList payload")

    payload = result.get("payload")
    if not isinstance(payload, dict):
        raise BridgeError("Bridge threadList payload malformed")

    data = payload.get("data")
    if not isinstance(data, list):
        raise BridgeError("Bridge threadList payload missing data")

    known_projects = set(orchestrator_by_project.keys())
    if current_project:
        known_projects.add(current_project)
    metadata_by_thread = _load_thread_metadata_map()

    threads: list[ThreadEntry] = []
    for row in data:
        if not isinstance(row, dict):
            continue
        thread_id = _normalize_text(row.get("id"))
        if not thread_id:
            continue
        cwd = _normalized_path(row.get("cwd"))
        preview = _normalize_text(row.get("preview"))
        custom_title = titles_by_thread_id.get(thread_id)
        display_name = custom_title or preview or thread_id
        metadata = metadata_by_thread.get(thread_id, {})

        project_path: str | None = None
        if cwd:
            matches = [candidate for candidate in known_projects if _path_is_within_project(cwd, candidate)]
            if matches:
                project_path = max(matches, key=len)

        threads.append(
            ThreadEntry(
                id=thread_id,
                cwd=cwd,
                preview=preview,
                display_name=display_name,
                project_path=project_path,
                has_custom_title=custom_title is not None,
                issue_number=metadata.get("issueNumber") if isinstance(metadata.get("issueNumber"), int) else None,
                pull_request_number=metadata.get("pullRequestNumber") if isinstance(metadata.get("pullRequestNumber"), int) else None,
                blocked_reason=_normalize_text(metadata.get("blockedReason")) if isinstance(metadata.get("blockedReason"), str) else None,
                unblock_when=_normalize_text(metadata.get("unblockWhen")) if isinstance(metadata.get("unblockWhen"), str) else None,
                hidden=bool(metadata.get("hidden")) if "hidden" in metadata else False,
            )
        )
    return threads


def _resolve_context(from_thread_id: str, tool_context: Context) -> Context:
    host = _normalize_text(os.getenv("ROBDEX_BRIDGE_HOST")) or "127.0.0.1"
    port_text = _normalize_text(os.getenv("ROBDEX_BRIDGE_PORT")) or "42080"
    token = _normalize_text(os.getenv("ROBDEX_BRIDGE_TOKEN"))
    if not token:
        token = _read_bridge_token_from_file()
    if not token:
        token = _read_bridge_token_from_keychain()

    try:
        port = int(port_text)
    except ValueError as exc:
        raise BridgeError(f"Invalid ROBDEX_BRIDGE_PORT: {port_text}") from exc

    preferred_instance_id = _normalize_text(os.getenv("ROBDEX_INSTANCE_ID")) or DEFAULT_INSTANCE_ID
    instance_id = _resolve_live_instance_id(host, port, token, preferred_instance_id)
    current_thread_id = _resolve_session_thread_id(
        from_thread_id=from_thread_id,
        tool_context=tool_context,
    )

    titles_by_thread_id, orchestrator_by_project, global_orchestrator = _load_state()

    current_project = None
    for project_path, thread_id in orchestrator_by_project.items():
        if thread_id == current_thread_id:
            current_project = project_path
            break

    if not current_project and current_thread_id:
        try:
            agents_result = _run_command(host, port, token, name="listAgents")
            for agent in _parse_agents(agents_result):
                if agent.thread_id == current_thread_id and agent.project_path:
                    current_project = agent.project_path
                    break
        except Exception:
            current_project = None

    if not current_project and current_thread_id:
        thread_payload = {
            "instanceId": instance_id,
            "archived": False,
            "limit": 1000,
            "modelProviders": [],
            "sortKey": "updated_at",
        }
        try:
            thread_result = _run_instance_command(host, port, token, name="threadList", payload=thread_payload)
            parsed_threads = _parse_thread_list(thread_result, titles_by_thread_id, orchestrator_by_project, None)
            for thread in parsed_threads:
                if thread.id == current_thread_id and thread.project_path:
                    current_project = thread.project_path
                    break
        except Exception:
            current_project = None

    if not current_project:
        current_project = _normalized_path(os.getenv("ROBDEX_PROJECT_PATH"))
    if not current_project:
        current_project = _normalized_path(os.getcwd())

    current_is_orchestrator = False
    if current_thread_id:
        current_is_orchestrator = current_thread_id in set(orchestrator_by_project.values())
        if not current_is_orchestrator and global_orchestrator:
            current_is_orchestrator = current_thread_id == global_orchestrator

    return Context(
        host=host,
        port=port,
        token=token,
        instance_id=instance_id,
        current_thread_id=current_thread_id,
        current_project_path=current_project,
        current_is_orchestrator=current_is_orchestrator,
        titles_by_thread_id=titles_by_thread_id,
        orchestrator_by_project=orchestrator_by_project,
    )


def _list_threads(ctx: Context, archived: bool, *, include_hidden: bool = False) -> list[ThreadEntry]:
    payload = {
        "instanceId": ctx.instance_id,
        "archived": archived,
        "limit": 1000,
        "modelProviders": [],
        "sortKey": "updated_at",
    }
    result = _run_instance_command(ctx.host, ctx.port, ctx.token, name="threadList", payload=payload)
    threads = _parse_thread_list(result, ctx.titles_by_thread_id, ctx.orchestrator_by_project, ctx.current_project_path)
    if include_hidden:
        return threads
    return [thread for thread in threads if not thread.hidden]


def _current_sender_label(ctx: Context) -> str:
    if ctx.current_thread_id:
        return ctx.titles_by_thread_id.get(ctx.current_thread_id, ctx.current_thread_id)
    return "unknown-thread"


def _is_prefixed_agent_message(text: str) -> bool:
    return text.startswith("[") and "]:" in text.split("\n", 1)[0]


def _append_suffix(text: str) -> str:
    if CONTINUATION_SUFFIX.lower() in text.lower():
        return text
    return f"{text}\n\n{CONTINUATION_SUFFIX}"


def _format_agent_line(
    thread_id: str,
    display_name: str,
    orchestrator_ids: set[str],
    project_path: str | None = None,
    show_project: bool = False,
    current_thread_id: str | None = None,
) -> str:
    you_prefix = "**YOU** " if current_thread_id and thread_id == current_thread_id else ""
    suffix = " [orchestrator]" if thread_id in orchestrator_ids else ""
    base = f"{you_prefix}{_quoted(display_name)} ({thread_id}){suffix}"
    if show_project and project_path:
        return f"{base} | {project_path}"
    return base


def _display_name_for_listing(thread: ThreadEntry, max_unnamed_length: int = 96) -> str:
    if thread.has_custom_title:
        return thread.display_name
    if thread.display_name == thread.id:
        return thread.display_name

    if len(thread.display_name) <= max_unnamed_length:
        return thread.display_name
    clipped = thread.display_name[: max_unnamed_length - 3].rstrip()
    return f"{clipped}..."


def _thread_metadata_suffix(thread: ThreadEntry) -> str:
    parts: list[str] = []
    if thread.issue_number is not None:
        parts.append(f"issue=#{thread.issue_number}")
    if thread.pull_request_number is not None:
        parts.append(f"pr=#{thread.pull_request_number}")
    if thread.blocked_reason:
        blocked = f"blocked={_quoted(thread.blocked_reason)}"
        if thread.unblock_when:
            blocked += f" until={_quoted(thread.unblock_when)}"
        parts.append(blocked)
    return f" | {'; '.join(parts)}" if parts else ""


def _resolve_project_scope(ctx: Context, requested_project_path: str | None) -> str | None:
    if requested_project_path:
        return _normalized_path(requested_project_path)
    return ctx.current_project_path


def _ensure_orchestrator_for_project(ctx: Context, project_path: str | None, action: str) -> None:
    if not project_path:
        raise BridgeError(f"Unable to resolve target project for {action}.")

    expected = ctx.orchestrator_by_project.get(project_path)
    if expected:
        if ctx.current_thread_id != expected:
            required_name = ctx.titles_by_thread_id.get(expected, expected)
            raise BridgeError(f"Only orchestrator {_quoted(required_name)} can {action} for {project_path}.")
        return

    if ctx.orchestrator_by_project and not ctx.current_is_orchestrator:
        raise BridgeError(f"Only orchestrator threads can {action} for unassigned project scope.")


def _ensure_unique_title(ctx: Context, title: str, excluding_thread_id: str | None = None) -> None:
    normalized = _normalized_title(title)
    if not normalized:
        return
    for thread_id, existing in ctx.titles_by_thread_id.items():
        if excluding_thread_id and thread_id == excluding_thread_id:
            continue
        if _normalized_title(existing) == normalized:
            raise BridgeError(f"Thread title {_quoted(title)} already exists as {_quoted(existing)} ({thread_id}).")


def _resolve_thread_by_name(name: str, threads: list[ThreadEntry]) -> ThreadEntry:
    normalized = _normalized_title(name)
    matches = [thread for thread in threads if _normalized_title(thread.display_name) == normalized]
    if not matches:
        raise BridgeError(f"No thread found with title {_quoted(name)}.")
    if len(matches) > 1:
        raise BridgeError(f"Multiple threads match {_quoted(name)}; use to_thread_id instead.")
    return matches[0]


def _resolve_int_metadata(value: int | None, clear: bool, field_name: str) -> int | None:
    if clear:
        return None
    if value is None:
        return None
    if value <= 0:
        raise BridgeError(f"{field_name} must be a positive integer.")
    return value


def _resolve_text_metadata(value: str | None, clear: bool) -> str | None:
    if clear:
        return None
    return _normalize_text(value)


def _resolve_thread_target(
    *,
    to_thread_id: str | None,
    name: str | None,
    project_path: str | None,
    threads: list[ThreadEntry],
) -> ThreadEntry:
    if to_thread_id:
        normalized_id = _normalize_text(to_thread_id)
        if not normalized_id:
            raise BridgeError("to_thread_id cannot be empty")
        for thread in threads:
            if thread.id == normalized_id:
                return thread
        raise BridgeError(f"Thread not found: {normalized_id}")

    if name:
        scope_threads = threads
        if project_path:
            normalized_project = _normalized_path(project_path)
            scope_threads = [thread for thread in threads if thread.project_path == normalized_project]
        return _resolve_thread_by_name(name, scope_threads)

    raise BridgeError("Provide to_thread_id or name")


def _resolve_send_target(
    *,
    ctx: Context,
    to_thread_id: str | None,
    name: str | None,
    project_path: str | None,
    threads: list[ThreadEntry],
) -> ThreadEntry:
    try:
        return _resolve_thread_target(
            to_thread_id=to_thread_id,
            name=name,
            project_path=_resolve_project_scope(ctx, project_path),
            threads=threads,
        )
    except BridgeError as exc:
        # Orchestrator-to-orchestrator sends should resolve by unique title even
        # when the caller omits project_path.
        if not (
            ctx.current_is_orchestrator
            and not to_thread_id
            and name
            and project_path is None
            and "No thread found with title" in str(exc)
        ):
            raise
        return _resolve_thread_target(
            to_thread_id=None,
            name=name,
            project_path=None,
            threads=threads,
        )


def _compose_message(ctx: Context, text: str) -> str:
    trimmed = text.strip()
    if not trimmed:
        raise BridgeError("Message text cannot be empty.")
    sender_label = _current_sender_label(ctx)
    prefixed = trimmed if _is_prefixed_agent_message(trimmed) else f"[{sender_label}]: {trimmed}"
    return _append_suffix(prefixed)


def _send_text_to_thread(ctx: Context, target_thread_id: str, text: str) -> None:
    agents_result = _run_command(ctx.host, ctx.port, ctx.token, name="listAgents")
    agents = _parse_agents(agents_result)
    sender_agent_id = next(
        (
            agent.id
            for agent in agents
            if agent.thread_id == ctx.current_thread_id and agent.status not in {"closed"}
        ),
        None,
    )

    running_agent = next(
        (
            agent
            for agent in agents
            if agent.thread_id == target_thread_id and agent.status not in {"closed"}
        ),
        None,
    )

    if running_agent:
        payload: dict[str, Any] = {"agentId": running_agent.id, "text": text}
        if sender_agent_id:
            payload["senderAgentId"] = sender_agent_id
        try:
            _run_command(ctx.host, ctx.port, ctx.token, name="sendAgentInput", payload=payload)
            return
        except BridgeError as error:
            if not _is_active_turn_changed_error(str(error)):
                raise

        time.sleep(0.2)
        _run_command(ctx.host, ctx.port, ctx.token, name="sendAgentInput", payload=payload)
        return

    target_agent = next((agent for agent in agents if agent.thread_id == target_thread_id), None)
    payload = {
        "instanceId": target_agent.instance_id if target_agent and target_agent.instance_id else ctx.instance_id,
        "threadId": target_thread_id,
        "text": text,
    }
    _run_instance_command(ctx.host, ctx.port, ctx.token, name="turnStart", payload=payload)


@mcp.tool
def robdex_list_projects(from_thread_id: str, ctx: Context) -> str:
    """List known project paths and their configured orchestrator thread IDs."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    if not resolved_context.orchestrator_by_project:
        return "(no project orchestrators configured)"

    lines: list[str] = []
    for project_path in sorted(resolved_context.orchestrator_by_project.keys()):
        thread_id = resolved_context.orchestrator_by_project[project_path]
        display_name = resolved_context.titles_by_thread_id.get(thread_id, thread_id)
        lines.append(f"{project_path} -> {_quoted(display_name)} ({thread_id})")
    return "\n".join(lines)


@mcp.tool
def robdex_list_agents(
    from_thread_id: str,
    include_archived: bool = False,
    include_all_projects: bool = False,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """List thread identities in robdex format; default is current project, unarchived only."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    target_project = _resolve_project_scope(resolved_context, project_path)

    include_all_projects = include_all_projects and resolved_context.current_is_orchestrator
    if not include_all_projects and not target_project:
        raise BridgeError("Unable to resolve project scope for list-agents without --all-projects.")

    threads = _list_threads(resolved_context, archived=include_archived)
    current_project = resolved_context.current_project_path
    is_cross_project_scope = bool(target_project and current_project and target_project != current_project)

    if include_all_projects:
        filtered: list[ThreadEntry] = []
        for thread in threads:
            if thread.project_path == current_project:
                filtered.append(thread)
                continue

            project = thread.project_path
            if not project:
                continue
            orchestrator_id = resolved_context.orchestrator_by_project.get(project)
            if not orchestrator_id:
                continue
            if include_archived:
                continue
            if thread.id != orchestrator_id:
                continue
            filtered.append(thread)
        threads = filtered
    else:
        if target_project:
            threads = [thread for thread in threads if thread.project_path == target_project]
        if is_cross_project_scope:
            orchestrator_id = resolved_context.orchestrator_by_project.get(target_project)
            if not orchestrator_id or include_archived:
                threads = []
            else:
                threads = [thread for thread in threads if thread.id == orchestrator_id]

    if not threads:
        return "(no matching threads)"

    orchestrator_ids = set(resolved_context.orchestrator_by_project.values())
    sorted_threads = sorted(threads, key=lambda item: (_normalized_title(item.display_name), item.id))
    show_project = include_all_projects or bool(project_path and project_path != resolved_context.current_project_path)

    lines = [
        _format_agent_line(
            thread_id=thread.id,
            display_name=_display_name_for_listing(thread),
            orchestrator_ids=orchestrator_ids,
            project_path=thread.project_path,
            show_project=show_project,
            current_thread_id=resolved_context.current_thread_id,
        ) + _thread_metadata_suffix(thread)
        for thread in sorted_threads
    ]
    return "\n".join(lines)


@mcp.tool
def robdex_spawn_agent(
    from_thread_id: str,
    name: str,
    prompt: str = "",
    cwd: str | None = None,
    issue_number: int | None = None,
    ctx: Context = None,
) -> str:
    """Spawn a worker agent thread with a unique display name."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    name_value = _normalize_text(name)
    if not name_value:
        raise BridgeError("name is required")

    target_project = resolved_context.current_project_path
    _ensure_orchestrator_for_project(resolved_context, target_project, "spawn agents")
    _ensure_unique_title(resolved_context, name_value)

    sender_label = _current_sender_label(resolved_context)
    spawn_context = (
        f"Spawned by {sender_label} ({resolved_context.current_thread_id or 'unknown-thread-id'}). "
        "Communicate with this sender using the $robdex-orchestrator skill unless instructed otherwise."
    )
    trimmed_prompt = prompt.strip()
    initial_prompt = spawn_context if not trimmed_prompt else f"{spawn_context}\n\n{trimmed_prompt}"

    payload: dict[str, Any] = {
        "displayName": name_value,
        "configOverrides": [],
        "initialPrompt": initial_prompt,
    }
    if target_project:
        payload["projectPath"] = target_project
    parent_thread = resolved_context.current_thread_id
    if parent_thread:
        payload["parentThreadId"] = parent_thread
    parent_instance = _normalize_text(os.getenv("ROBDEX_INSTANCE_ID"))
    if parent_instance:
        payload["parentInstanceId"] = parent_instance
    parent_agent = _normalize_text(os.getenv("ROBDEX_AGENT_ID"))
    if parent_agent:
        payload["parentAgentId"] = parent_agent
    normalized_cwd = _normalized_path(cwd)
    if normalized_cwd:
        payload["cwd"] = normalized_cwd

    result = _run_command(resolved_context.host, resolved_context.port, resolved_context.token, name="spawnAgent", payload=payload)
    if result.get("type") != "agent":
        raise BridgeError("Bridge response was not agent payload")
    agent_payload = result.get("payload")
    if not isinstance(agent_payload, dict):
        raise BridgeError("Bridge agent payload malformed")
    thread_id = _normalize_text(agent_payload.get("threadId")) or "no-thread-id"
    display_name = _normalize_text(agent_payload.get("displayName")) or name_value
    if issue_number is not None:
        if issue_number <= 0:
            raise BridgeError("issue_number must be a positive integer.")
        _set_thread_metadata_fields(thread_id, {"issueNumber": issue_number})
    return f"Spawned {_quoted(display_name)} ({thread_id})"


@mcp.tool
def robdex_set_worker_metadata(
    from_thread_id: str,
    issue_number: int | None = None,
    pull_request_number: int | None = None,
    blocked_reason: str | None = None,
    unblock_when: str | None = None,
    clear_issue_number: bool = False,
    clear_pull_request_number: bool = False,
    clear_blocked: bool = False,
    to_thread_id: str | None = None,
    name: str | None = None,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Set issue/PR/blocked bookkeeping fields for a worker thread."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    all_threads = _list_threads(resolved_context, archived=False) + _list_threads(resolved_context, archived=True)
    target = _resolve_thread_target(
        to_thread_id=to_thread_id,
        name=name,
        project_path=_resolve_project_scope(resolved_context, project_path),
        threads=all_threads,
    )

    _ensure_orchestrator_for_project(resolved_context, target.project_path, "set worker metadata")

    if target.id == resolved_context.current_thread_id:
        raise BridgeError("Orchestrators cannot set worker metadata on themselves.")

    orchestrator_ids = set(resolved_context.orchestrator_by_project.values())
    if target.id in orchestrator_ids:
        raise BridgeError("Worker metadata can only be set on non-orchestrator threads.")

    resolved_issue_number = _resolve_int_metadata(issue_number, clear_issue_number, "issue_number")
    resolved_pr_number = _resolve_int_metadata(pull_request_number, clear_pull_request_number, "pull_request_number")
    resolved_blocked_reason = _resolve_text_metadata(blocked_reason, clear_blocked)
    resolved_unblock_when = _resolve_text_metadata(unblock_when, clear_blocked)

    if resolved_unblock_when is not None and resolved_blocked_reason is None:
        raise BridgeError("unblock_when requires blocked_reason unless clear_blocked is used.")

    updates: dict[str, Any | None] = {}
    if issue_number is not None or clear_issue_number:
        updates["issueNumber"] = resolved_issue_number
    if pull_request_number is not None or clear_pull_request_number:
        updates["pullRequestNumber"] = resolved_pr_number
    if blocked_reason is not None or unblock_when is not None or clear_blocked:
        updates["blockedReason"] = resolved_blocked_reason
        updates["unblockWhen"] = resolved_unblock_when

    if not updates:
        raise BridgeError("Provide at least one metadata field to set or clear.")

    _set_thread_metadata_fields(target.id, updates)

    refreshed = _load_thread_metadata_map().get(target.id, {})
    summary_parts: list[str] = []
    if isinstance(refreshed.get("issueNumber"), int):
        summary_parts.append(f"issue=#{refreshed['issueNumber']}")
    if isinstance(refreshed.get("pullRequestNumber"), int):
        summary_parts.append(f"pr=#{refreshed['pullRequestNumber']}")
    blocked_value = _normalize_text(refreshed.get("blockedReason")) if isinstance(refreshed.get("blockedReason"), str) else None
    unblock_value = _normalize_text(refreshed.get("unblockWhen")) if isinstance(refreshed.get("unblockWhen"), str) else None
    if blocked_value:
        blocked_summary = f"blocked={_quoted(blocked_value)}"
        if unblock_value:
            blocked_summary += f" until={_quoted(unblock_value)}"
        summary_parts.append(blocked_summary)

    summary = ", ".join(summary_parts) if summary_parts else "cleared"
    return f"Updated {_quoted(target.display_name)} ({target.id}): {summary}"


@mcp.tool
def robdex_unarchive_agent(
    from_thread_id: str,
    name: str,
    prompt: str = "",
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Unarchive a named thread; if already unarchived, no-op. Optionally send a steering/start message."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    name_value = _normalize_text(name)
    if not name_value:
        raise BridgeError("name is required")

    all_unarchived = _list_threads(resolved_context, archived=False)
    all_archived = _list_threads(resolved_context, archived=True)
    all_threads = all_unarchived + all_archived
    target_project = _resolve_project_scope(resolved_context, project_path)
    scoped_threads = all_threads
    if target_project:
        scoped_threads = [thread for thread in all_threads if thread.project_path == target_project]
    target = _resolve_thread_by_name(name_value, scoped_threads)

    _ensure_orchestrator_for_project(resolved_context, target.project_path or target_project, "unarchive agents")

    was_archived = any(thread.id == target.id for thread in all_archived)
    if was_archived:
        _run_instance_command(
            resolved_context.host,
            resolved_context.port,
            resolved_context.token,
            name="threadUnarchive",
            payload={"instanceId": resolved_context.instance_id, "threadId": target.id},
        )

    if prompt.strip():
        message = _compose_message(resolved_context, prompt)
        _send_text_to_thread(resolved_context, target.id, message)

    status = "Unarchived" if was_archived else "Already unarchived"
    return f"{status} {_quoted(target.display_name)} ({target.id})"


@mcp.tool
def robdex_rename_agent(
    from_thread_id: str,
    new_name: str,
    to_thread_id: str | None = None,
    name: str | None = None,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Rename a thread display name with uniqueness and orchestrator checks."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    new_name_value = _normalize_text(new_name)
    if not new_name_value:
        raise BridgeError("new_name is required")

    all_threads = _list_threads(resolved_context, archived=False) + _list_threads(resolved_context, archived=True)
    target = _resolve_thread_target(
        to_thread_id=to_thread_id,
        name=name,
        project_path=_resolve_project_scope(resolved_context, project_path),
        threads=all_threads,
    )

    _ensure_orchestrator_for_project(resolved_context, target.project_path, "rename agents")
    _ensure_unique_title(resolved_context, new_name_value, excluding_thread_id=target.id)

    _run_instance_command(
        resolved_context.host,
        resolved_context.port,
        resolved_context.token,
        name="threadNameSet",
        payload={"instanceId": resolved_context.instance_id, "threadId": target.id, "name": new_name_value},
    )
    return f"Renamed {_quoted(target.display_name)} ({target.id}) -> {_quoted(new_name_value)}"


@mcp.tool
def robdex_send_message(
    from_thread_id: str,
    text: str,
    to_thread_id: str | None = None,
    name: str | None = None,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Send steering/user input to a target thread with project-boundary guardrails."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    all_unarchived = _list_threads(resolved_context, archived=False)
    all_archived = _list_threads(resolved_context, archived=True)
    all_threads = all_unarchived + all_archived

    target = _resolve_send_target(
        ctx=resolved_context,
        to_thread_id=to_thread_id,
        name=name,
        project_path=project_path,
        threads=all_unarchived,
    )

    if not target.project_path:
        raise BridgeError("Target thread is outside configured Robdex project scope.")

    if target.project_path not in resolved_context.orchestrator_by_project:
        raise BridgeError(f"No orchestrator is configured for target project: {target.project_path}")

    sender_thread = next(
        (thread for thread in all_threads if thread.id == resolved_context.current_thread_id),
        None,
    )
    sender_project = sender_thread.project_path if sender_thread else resolved_context.current_project_path
    if not sender_project:
        raise BridgeError(
            "Unable to resolve sender project for this thread identity; refusing cross-project-ambiguous send."
        )

    same_project = bool(sender_project and sender_project == target.project_path)
    target_is_orchestrator = resolved_context.orchestrator_by_project.get(target.project_path) == target.id

    if resolved_context.current_is_orchestrator:
        if not same_project and not target_is_orchestrator:
            raise BridgeError("Orchestrators can only message worker threads inside their own project.")
    else:
        if not same_project:
            raise BridgeError("Workers can only message threads inside their own project.")

    message = _compose_message(resolved_context, text)
    _send_text_to_thread(resolved_context, target.id, message)
    return f"Sent to {_quoted(target.display_name)} ({target.id})"


def main() -> None:
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
