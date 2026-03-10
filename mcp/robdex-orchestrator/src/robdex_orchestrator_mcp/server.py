from __future__ import annotations

import asyncio
import json
import os
import subprocess
import threading
import time
import uuid
import urllib.error
import urllib.parse
import urllib.request
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
    is_orchestrator: bool = False
    is_running: bool = False
    updated_at: float | None = None


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


@dataclass(frozen=True)
class ThreadGroupEntry:
    id: str
    title: str
    thread_ids: list[str]
    is_collapsed: bool
    created_at: float
    updated_at: float


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


def _coerce_timestamp(value: Any, default: float) -> float:
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        try:
            return float(value)
        except ValueError:
            return default
    return default


def _quoted(value: str) -> str:
    return json.dumps(value)


def _http_json_request(
    host: str,
    port: int,
    token: str | None,
    *,
    method: str,
    path: str,
    query: dict[str, str] | None = None,
    body: dict[str, Any] | None = None,
    timeout_seconds: float = 30.0,
) -> dict[str, Any]:
    query_string = urllib.parse.urlencode(query or {})
    url = f"http://{host}:{port}{path}"
    if query_string:
        url = f"{url}?{query_string}"

    payload_bytes: bytes | None = None
    headers = {"Accept": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    if body is not None:
        payload_bytes = json.dumps(body, separators=(",", ":"), sort_keys=True).encode("utf-8")
        headers["Content-Type"] = "application/json"

    request = urllib.request.Request(url, data=payload_bytes, headers=headers, method=method.upper())
    try:
        with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
            raw = response.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as exc:
        try:
            detail = exc.read().decode("utf-8", errors="replace").strip()
        except Exception:
            detail = ""
        message = detail or exc.reason or f"HTTP {exc.code}"
        raise BridgeError(message) from exc
    except Exception as exc:  # noqa: BLE001
        raise BridgeError(f"HTTP request failed for {method.upper()} {path}: {exc}") from exc

    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise BridgeError(f"Bridge HTTP response was not valid JSON for {method.upper()} {path}") from exc
    if not isinstance(payload, dict):
        raise BridgeError(f"Bridge HTTP response was not an object for {method.upper()} {path}")
    return payload


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


def _normalized_thread_group(raw: Any) -> ThreadGroupEntry | None:
    if not isinstance(raw, dict):
        return None

    raw_group_id = raw.get("id")
    raw_title = raw.get("title")
    group_id = _normalize_text(raw_group_id if isinstance(raw_group_id, str) else None if raw_group_id is None else str(raw_group_id))
    title = _normalize_text(raw_title if isinstance(raw_title, str) else None if raw_title is None else str(raw_title))
    if not group_id or not title:
        return None

    raw_thread_ids = raw.get("threadIDs")
    thread_ids: list[str] = []
    seen_thread_ids: set[str] = set()
    if isinstance(raw_thread_ids, list):
        for entry in raw_thread_ids:
            thread_id = _normalize_text(str(entry))
            if not thread_id or thread_id in seen_thread_ids:
                continue
            seen_thread_ids.add(thread_id)
            thread_ids.append(thread_id)

    now = time.time()
    created_at = _coerce_timestamp(raw.get("createdAt"), now)
    updated_at = _coerce_timestamp(raw.get("updatedAt"), created_at)
    return ThreadGroupEntry(
        id=group_id,
        title=title,
        thread_ids=thread_ids,
        is_collapsed=bool(raw.get("isCollapsed")),
        created_at=created_at,
        updated_at=updated_at,
    )


def _sort_thread_groups(groups: list[ThreadGroupEntry]) -> list[ThreadGroupEntry]:
    return sorted(
        groups,
        key=lambda group: (_normalized_title(group.title), -group.updated_at, group.id),
    )


def _load_thread_groups_by_project() -> dict[str, list[ThreadGroupEntry]]:
    payload = _load_state_payload()
    raw_groups_by_project = payload.get("threadGroupsByProjectPath")
    if not isinstance(raw_groups_by_project, dict):
        return {}

    result: dict[str, list[ThreadGroupEntry]] = {}
    for raw_project_path, raw_groups in raw_groups_by_project.items():
        project_path = _normalized_path(str(raw_project_path))
        if not project_path or not isinstance(raw_groups, list):
            continue
        groups = [group for group in (_normalized_thread_group(entry) for entry in raw_groups) if group]
        if groups:
            result[project_path] = _sort_thread_groups(groups)
    return result


def _write_thread_groups_by_project(groups_by_project: dict[str, list[ThreadGroupEntry]]) -> None:
    payload = _load_state_payload()
    serialized: dict[str, list[dict[str, Any]]] = {}
    for project_path, groups in groups_by_project.items():
        normalized_project_path = _normalized_path(project_path)
        if not normalized_project_path or not groups:
            continue
        serialized[normalized_project_path] = [
            {
                "id": group.id,
                "title": group.title,
                "threadIDs": group.thread_ids,
                "isCollapsed": group.is_collapsed,
                "createdAt": group.created_at,
                "updatedAt": group.updated_at,
            }
            for group in _sort_thread_groups(groups)
        ]

    if serialized:
        payload["threadGroupsByProjectPath"] = serialized
    else:
        payload.pop("threadGroupsByProjectPath", None)
    payload["updatedAt"] = time.time()
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


def _parse_scoped_agents_payload(payload: dict[str, Any]) -> list[ThreadEntry]:
    items = payload.get("items")
    if not isinstance(items, list):
        raise BridgeError("Scoped agents payload missing items list")

    agents: list[ThreadEntry] = []
    for entry in items:
        if not isinstance(entry, dict):
            continue
        thread_id = _normalize_text(entry.get("id"))
        if not thread_id:
            continue
        display_name = _normalize_text(entry.get("displayName")) or thread_id
        issue_number = entry.get("issueNumber") if isinstance(entry.get("issueNumber"), int) else None
        pull_request_number = entry.get("pullRequestNumber") if isinstance(entry.get("pullRequestNumber"), int) else None
        blocked_reason = _normalize_text(entry.get("blockedReason")) if isinstance(entry.get("blockedReason"), str) else None
        unblock_when = _normalize_text(entry.get("unblockWhen")) if isinstance(entry.get("unblockWhen"), str) else None
        updated_at = _coerce_timestamp(entry.get("updatedAt"), 0.0)
        agents.append(
            ThreadEntry(
                id=thread_id,
                cwd=_normalized_path(entry.get("cwd")),
                preview=display_name,
                display_name=display_name,
                project_path=_normalized_path(entry.get("projectPath")),
                has_custom_title=True,
                issue_number=issue_number,
                pull_request_number=pull_request_number,
                blocked_reason=blocked_reason,
                unblock_when=unblock_when,
                hidden=False,
                is_orchestrator=bool(entry.get("isOrchestrator")),
                is_running=bool(entry.get("isRunning")),
                updated_at=updated_at,
            )
        )
    return agents


def _list_scoped_agents(ctx: Context, *, include_archived: bool) -> list[ThreadEntry]:
    payload = _http_json_request(
        ctx.host,
        ctx.port,
        ctx.token,
        method="GET",
        path="/orchestrator/agents",
        query={
            "senderThreadId": ctx.current_thread_id or "",
            "includeArchived": "1" if include_archived else "0",
        },
    )
    return _parse_scoped_agents_payload(payload)


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


def _format_thread_group_line(group: ThreadGroupEntry) -> str:
    thread_summary = ", ".join(group.thread_ids) if group.thread_ids else "(empty)"
    collapsed = "collapsed" if group.is_collapsed else "expanded"
    return f"{_quoted(group.title)} ({group.id}) | {collapsed}; threads={thread_summary}"


def _resolve_project_scope(ctx: Context, requested_project_path: str | None) -> str | None:
    if requested_project_path:
        return _normalized_path(requested_project_path)
    return ctx.current_project_path


def _resolve_group_project_scope(ctx: Context, requested_project_path: str | None, action: str) -> str:
    target_project = _resolve_project_scope(ctx, requested_project_path)
    _ensure_orchestrator_for_project(ctx, target_project, action)
    if not target_project:
        raise BridgeError(f"Unable to resolve target project for {action}.")
    return target_project


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


def _require_thread_group(
    groups: list[ThreadGroupEntry],
    group_id: str,
    *,
    action: str,
) -> tuple[int, ThreadGroupEntry]:
    normalized_group_id = _normalize_text(group_id)
    if not normalized_group_id:
        raise BridgeError("group_id is required")
    for index, group in enumerate(groups):
        if group.id == normalized_group_id:
            return index, group
    raise BridgeError(f"Thread group not found for {action}: {normalized_group_id}")


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
    """List bridge-scoped visible agents for the current sender thread."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    threads = _list_scoped_agents(resolved_context, include_archived=include_archived)

    target_project = _resolve_project_scope(resolved_context, project_path)
    if target_project:
        threads = [thread for thread in threads if thread.project_path == target_project]

    if not threads:
        return "(no matching threads)"

    orchestrator_ids = {thread.id for thread in threads if thread.is_orchestrator}
    sorted_threads = sorted(
        threads,
        key=lambda item: (_normalized_title(item.display_name), _normalized_path(item.project_path) or "", item.id),
    )
    visible_projects = {thread.project_path for thread in threads if thread.project_path}
    show_project = include_all_projects or len(visible_projects) > 1 or bool(project_path and project_path != resolved_context.current_project_path)

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
def robdex_list_thread_groups(
    from_thread_id: str,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """List persisted thread groups for a project."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    target_project = _resolve_group_project_scope(resolved_context, project_path, "list thread groups")
    groups = _load_thread_groups_by_project().get(target_project, [])
    if not groups:
        return "(no thread groups)"
    return "\n".join(_format_thread_group_line(group) for group in groups)


@mcp.tool
def robdex_create_thread_group(
    from_thread_id: str,
    title: str,
    project_path: str | None = None,
    seed_thread_id: str | None = None,
    ctx: Context = None,
) -> str:
    """Create a new thread group for a project."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    target_project = _resolve_group_project_scope(resolved_context, project_path, "create thread groups")
    title_value = _normalize_text(title)
    if not title_value:
        raise BridgeError("title is required")

    seed_thread_id_value = _normalize_text(seed_thread_id)
    groups_by_project = _load_thread_groups_by_project()
    groups = list(groups_by_project.get(target_project, []))
    if seed_thread_id_value:
        now = time.time()
        for index, group in enumerate(groups):
            if seed_thread_id_value in group.thread_ids:
                groups[index] = ThreadGroupEntry(
                    id=group.id,
                    title=group.title,
                    thread_ids=[thread_id for thread_id in group.thread_ids if thread_id != seed_thread_id_value],
                    is_collapsed=group.is_collapsed,
                    created_at=group.created_at,
                    updated_at=now,
                )

    now = time.time()
    group = ThreadGroupEntry(
        id=str(uuid.uuid4()),
        title=title_value,
        thread_ids=[seed_thread_id_value] if seed_thread_id_value else [],
        is_collapsed=False,
        created_at=now,
        updated_at=now,
    )
    groups.append(group)
    groups_by_project[target_project] = _sort_thread_groups(groups)
    _write_thread_groups_by_project(groups_by_project)
    return f"Created {_format_thread_group_line(group)}"


@mcp.tool
def robdex_update_thread_group(
    from_thread_id: str,
    group_id: str,
    title: str | None = None,
    is_collapsed: bool | None = None,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Update thread group title and/or collapsed state."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    target_project = _resolve_group_project_scope(resolved_context, project_path, "update thread groups")
    if title is None and is_collapsed is None:
        raise BridgeError("Provide at least one of title or collapsed state to update.")
    groups_by_project = _load_thread_groups_by_project()
    groups = list(groups_by_project.get(target_project, []))
    index, current_group = _require_thread_group(groups, group_id, action="update")

    next_title = current_group.title
    next_updated_at = current_group.updated_at
    if title is not None:
        next_title_value = _normalize_text(title)
        if not next_title_value:
            raise BridgeError("title cannot be empty")
        next_title = next_title_value
        next_updated_at = time.time()

    next_collapsed = current_group.is_collapsed if is_collapsed is None else bool(is_collapsed)
    updated_group = ThreadGroupEntry(
        id=current_group.id,
        title=next_title,
        thread_ids=list(current_group.thread_ids),
        is_collapsed=next_collapsed,
        created_at=current_group.created_at,
        updated_at=next_updated_at,
    )
    groups[index] = updated_group
    groups_by_project[target_project] = _sort_thread_groups(groups)
    _write_thread_groups_by_project(groups_by_project)
    return f"Updated {_format_thread_group_line(updated_group)}"


@mcp.tool
def robdex_move_thread_to_group(
    from_thread_id: str,
    thread_id: str,
    group_id: str | None = None,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Move a thread into a group, or remove it from any current group."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    target_project = _resolve_group_project_scope(resolved_context, project_path, "move threads between thread groups")
    thread_id_value = _normalize_text(thread_id)
    if not thread_id_value:
        raise BridgeError("thread_id is required")

    groups_by_project = _load_thread_groups_by_project()
    groups = list(groups_by_project.get(target_project, []))
    target_index: int | None = None
    if group_id is not None:
        target_index, _ = _require_thread_group(groups, group_id, action="move")

    now = time.time()
    for index, group in enumerate(groups):
        if thread_id_value in group.thread_ids:
            groups[index] = ThreadGroupEntry(
                id=group.id,
                title=group.title,
                thread_ids=[entry for entry in group.thread_ids if entry != thread_id_value],
                is_collapsed=group.is_collapsed,
                created_at=group.created_at,
                updated_at=now,
            )

    destination_group_id: str | None = None
    if target_index is not None:
        target_group = groups[target_index]
        destination_group_id = target_group.id
        groups[target_index] = ThreadGroupEntry(
            id=target_group.id,
            title=target_group.title,
            thread_ids=target_group.thread_ids + [thread_id_value],
            is_collapsed=target_group.is_collapsed,
            created_at=target_group.created_at,
            updated_at=now,
        )

    groups_by_project[target_project] = _sort_thread_groups(groups)
    _write_thread_groups_by_project(groups_by_project)
    if destination_group_id:
        return f"Moved {thread_id_value} to thread group {destination_group_id}"
    return f"Removed {thread_id_value} from any thread group in {target_project}"


@mcp.tool
def robdex_delete_thread_group(
    from_thread_id: str,
    group_id: str,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Delete a thread group from a project."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    target_project = _resolve_group_project_scope(resolved_context, project_path, "delete thread groups")
    groups_by_project = _load_thread_groups_by_project()
    groups = list(groups_by_project.get(target_project, []))
    _, group = _require_thread_group(groups, group_id, action="delete")
    remaining_groups = [entry for entry in groups if entry.id != group.id]
    if remaining_groups:
        groups_by_project[target_project] = _sort_thread_groups(remaining_groups)
    else:
        groups_by_project.pop(target_project, None)
    _write_thread_groups_by_project(groups_by_project)
    return f"Deleted {_quoted(group.title)} ({group.id})"


@mcp.tool
def robdex_archive_thread_group(
    from_thread_id: str,
    group_id: str,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Archive all active same-project threads in a thread group."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    target_project = _resolve_group_project_scope(resolved_context, project_path, "archive thread groups")
    groups = _load_thread_groups_by_project().get(target_project, [])
    _, group = _require_thread_group(groups, group_id, action="archive")

    active_threads = _list_threads(resolved_context, archived=False, include_hidden=True)
    archived_threads = _list_threads(resolved_context, archived=True, include_hidden=True)
    active_by_id = {thread.id: thread for thread in active_threads}
    archived_by_id = {thread.id: thread for thread in archived_threads}

    archived_thread_ids: list[str] = []
    skipped_thread_ids: list[str] = []
    for thread_id in group.thread_ids:
        if thread_id in archived_by_id:
            skipped_thread_ids.append(thread_id)
            continue
        thread = active_by_id.get(thread_id)
        if not thread or thread.project_path != target_project:
            skipped_thread_ids.append(thread_id)
            continue
        _run_instance_command(
            resolved_context.host,
            resolved_context.port,
            resolved_context.token,
            name="threadArchive",
            payload={"instanceId": resolved_context.instance_id, "threadId": thread_id},
        )
        archived_thread_ids.append(thread_id)

    summary = [f"archived={','.join(archived_thread_ids) if archived_thread_ids else '(none)'}"]
    summary.append(f"skipped={','.join(skipped_thread_ids) if skipped_thread_ids else '(none)'}")
    return f"Archived {_quoted(group.title)} ({group.id}) | {'; '.join(summary)}"


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
def robdex_archive_agent(
    from_thread_id: str,
    to_thread_id: str | None = None,
    name: str | None = None,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Archive a worker thread through the bridge-owned orchestrator archive endpoint."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)

    recipient_thread_id = _normalize_text(to_thread_id)
    recipient_name = _normalize_text(name)
    if not recipient_thread_id and not recipient_name:
        raise BridgeError("Provide to_thread_id or name")

    target_project = _resolve_project_scope(resolved_context, project_path)
    response = _http_json_request(
        resolved_context.host,
        resolved_context.port,
        resolved_context.token,
        method="POST",
        path="/orchestrator/archive-agent",
        body={
            "senderThreadId": resolved_context.current_thread_id or "",
            "recipientThreadId": recipient_thread_id,
            "recipientName": recipient_name,
            "projectPath": target_project,
        },
    )

    resolved_thread_id = _normalize_text(response.get("recipientThreadId")) or recipient_thread_id or "unknown-thread-id"
    resolved_display_name = _normalize_text(response.get("recipientDisplayName")) or recipient_name or resolved_thread_id
    already_archived = bool(response.get("alreadyArchived"))
    status = "Already archived" if already_archived else "Archived"
    return f"{status} {_quoted(resolved_display_name)} ({resolved_thread_id})"


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
        _http_json_request(
            resolved_context.host,
            resolved_context.port,
            resolved_context.token,
            method="POST",
            path="/orchestrator/agent-message",
            body={
                "senderThreadId": resolved_context.current_thread_id or "",
                "recipientThreadId": target.id,
                "recipientName": None,
                "text": message,
            },
        )

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
    """Send a bridge-scoped message using senderThreadId-based authorization."""
    resolved_context = _resolve_context(from_thread_id=from_thread_id, tool_context=ctx)
    message = _compose_message(resolved_context, text)

    recipient_thread_id = _normalize_text(to_thread_id)
    recipient_name = _normalize_text(name)
    if not recipient_thread_id and not recipient_name:
        raise BridgeError("Provide to_thread_id or name")

    if project_path:
        visible_agents = _list_scoped_agents(resolved_context, include_archived=False)
        filtered_agents = visible_agents
        target_project = _resolve_project_scope(resolved_context, project_path)
        if target_project:
            filtered_agents = [thread for thread in visible_agents if thread.project_path == target_project]
        target = _resolve_thread_target(
            to_thread_id=recipient_thread_id,
            name=recipient_name,
            project_path=None,
            threads=filtered_agents,
        )
        recipient_thread_id = target.id
        recipient_name = None

    response = _http_json_request(
        resolved_context.host,
        resolved_context.port,
        resolved_context.token,
        method="POST",
        path="/orchestrator/agent-message",
        body={
            "senderThreadId": resolved_context.current_thread_id or "",
            "recipientThreadId": recipient_thread_id,
            "recipientName": recipient_name,
            "text": message,
        },
    )
    resolved_thread_id = _normalize_text(response.get("recipientThreadId")) or recipient_thread_id or "unknown-thread-id"
    resolved_display_name = _normalize_text(response.get("recipientDisplayName")) or recipient_name or resolved_thread_id
    return f"Sent to {_quoted(resolved_display_name)} ({resolved_thread_id})"


def main() -> None:
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
