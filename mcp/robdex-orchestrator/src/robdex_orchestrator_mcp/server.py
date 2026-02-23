from __future__ import annotations

import asyncio
import json
import os
import subprocess
import threading
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import websockets
from fastmcp import Context, FastMCP

CONTINUATION_SUFFIX = "Continue working unless told explicitly to stop, and respond only if necessary."
DEFAULT_INSTANCE_ID = "mgmt-global"
ROBDEX_STATE_FILE = Path.home() / ".codex" / "robdex.json"
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


@dataclass(frozen=True)
class AgentEntry:
    id: str
    thread_id: str | None
    status: str
    project_path: str | None


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


def _resolve_session_thread_id(
    *,
    agent_thread_id: str,
    tool_context: Context,
) -> str:
    provided_thread_id = _normalize_text(agent_thread_id)
    if not provided_thread_id:
        raise BridgeError("agent_thread_id is required. Use `echo \"$CODEX_THREAD_ID\"` and pass that value.")

    environment_thread_id = _normalize_text(os.getenv("CODEX_THREAD_ID"))
    if environment_thread_id and environment_thread_id != provided_thread_id:
        raise BridgeError(
            f"agent_thread_id {_quoted(provided_thread_id)} does not match this thread identity {_quoted(environment_thread_id)}."
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
                    f"Session is locked to agent_thread_id {_quoted(locked_thread_id)}; refusing {_quoted(provided_thread_id)}."
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
                thread_id=_normalize_text(entry.get("threadId")),
                status=_normalize_text(entry.get("status")) or "unknown",
                project_path=_normalized_path(entry.get("projectPath")),
            )
        )
    return agents


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

        project_path: str | None = None
        if cwd:
            matches = [candidate for candidate in known_projects if cwd.startswith(candidate)]
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
            )
        )
    return threads


def _resolve_context(agent_thread_id: str, tool_context: Context) -> Context:
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

    instance_id = _normalize_text(os.getenv("ROBDEX_INSTANCE_ID")) or DEFAULT_INSTANCE_ID
    current_thread_id = _resolve_session_thread_id(
        agent_thread_id=agent_thread_id,
        tool_context=tool_context,
    )

    titles_by_thread_id, orchestrator_by_project, global_orchestrator = _load_state()

    current_project = None
    for project_path, thread_id in orchestrator_by_project.items():
        if thread_id == current_thread_id:
            current_project = project_path
            break

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


def _list_threads(ctx: Context, archived: bool) -> list[ThreadEntry]:
    payload = {
        "instanceId": ctx.instance_id,
        "archived": archived,
        "limit": 1000,
        "modelProviders": [],
        "sortKey": "updated_at",
    }
    result = _run_command(ctx.host, ctx.port, ctx.token, name="threadList", payload=payload)
    return _parse_thread_list(result, ctx.titles_by_thread_id, ctx.orchestrator_by_project, ctx.current_project_path)


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


def _format_agent_line(thread_id: str, display_name: str, orchestrator_ids: set[str], project_path: str | None = None, show_project: bool = False) -> str:
    suffix = " [orchestrator]" if thread_id in orchestrator_ids else ""
    base = f"{_quoted(display_name)} ({thread_id}){suffix}"
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
        raise BridgeError(f"Multiple threads match {_quoted(name)}; use thread_id instead.")
    return matches[0]


def _resolve_thread_target(
    *,
    thread_id: str | None,
    name: str | None,
    project_path: str | None,
    threads: list[ThreadEntry],
) -> ThreadEntry:
    if thread_id:
        normalized_id = _normalize_text(thread_id)
        if not normalized_id:
            raise BridgeError("thread_id cannot be empty")
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

    raise BridgeError("Provide thread_id or name")


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
        _run_command(ctx.host, ctx.port, ctx.token, name="sendAgentInput", payload=payload)
        return

    payload = {
        "instanceId": ctx.instance_id,
        "threadId": target_thread_id,
        "text": text,
    }
    _run_command(ctx.host, ctx.port, ctx.token, name="turnStart", payload=payload)


@mcp.tool
def robdex_list_projects(agent_thread_id: str, ctx: Context) -> str:
    """List known project paths and their configured orchestrator thread IDs."""
    resolved_context = _resolve_context(agent_thread_id=agent_thread_id, tool_context=ctx)
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
    agent_thread_id: str,
    include_archived: bool = False,
    include_all_projects: bool = False,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """List thread identities in robdex format; default is current project, unarchived only."""
    resolved_context = _resolve_context(agent_thread_id=agent_thread_id, tool_context=ctx)
    target_project = _resolve_project_scope(resolved_context, project_path)

    if include_all_projects and not resolved_context.current_is_orchestrator:
        raise BridgeError("Only orchestrator threads can list agents across projects.")

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
        )
        for thread in sorted_threads
    ]
    return "\n".join(lines)


@mcp.tool
def robdex_spawn_agent(
    agent_thread_id: str,
    name: str,
    prompt: str = "",
    project_path: str | None = None,
    cwd: str | None = None,
    ctx: Context = None,
) -> str:
    """Spawn a worker agent thread with a unique display name."""
    resolved_context = _resolve_context(agent_thread_id=agent_thread_id, tool_context=ctx)
    name_value = _normalize_text(name)
    if not name_value:
        raise BridgeError("name is required")

    target_project = _resolve_project_scope(resolved_context, project_path)
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
    return f"Spawned {_quoted(display_name)} ({thread_id})"


@mcp.tool
def robdex_resume_agent(
    agent_thread_id: str,
    name: str,
    prompt: str = "",
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Unarchive a named thread and optionally send a steering/start message."""
    resolved_context = _resolve_context(agent_thread_id=agent_thread_id, tool_context=ctx)
    name_value = _normalize_text(name)
    if not name_value:
        raise BridgeError("name is required")

    all_threads = _list_threads(resolved_context, archived=False) + _list_threads(resolved_context, archived=True)
    target_project = _resolve_project_scope(resolved_context, project_path)
    scoped_threads = all_threads
    if target_project:
        scoped_threads = [thread for thread in all_threads if thread.project_path == target_project]
    target = _resolve_thread_by_name(name_value, scoped_threads)

    _ensure_orchestrator_for_project(resolved_context, target.project_path or target_project, "resume agents")

    _run_command(
        resolved_context.host,
        resolved_context.port,
        resolved_context.token,
        name="threadUnarchive",
        payload={"instanceId": resolved_context.instance_id, "threadId": target.id},
    )

    if prompt.strip():
        message = _compose_message(resolved_context, prompt)
        _send_text_to_thread(resolved_context, target.id, message)

    return f"Resumed {_quoted(target.display_name)} ({target.id})"


@mcp.tool
def robdex_rename_agent(
    agent_thread_id: str,
    new_name: str,
    thread_id: str | None = None,
    name: str | None = None,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Rename a thread display name with uniqueness and orchestrator checks."""
    resolved_context = _resolve_context(agent_thread_id=agent_thread_id, tool_context=ctx)
    new_name_value = _normalize_text(new_name)
    if not new_name_value:
        raise BridgeError("new_name is required")

    all_threads = _list_threads(resolved_context, archived=False) + _list_threads(resolved_context, archived=True)
    target = _resolve_thread_target(
        thread_id=thread_id,
        name=name,
        project_path=_resolve_project_scope(resolved_context, project_path),
        threads=all_threads,
    )

    _ensure_orchestrator_for_project(resolved_context, target.project_path, "rename agents")
    _ensure_unique_title(resolved_context, new_name_value, excluding_thread_id=target.id)

    _run_command(
        resolved_context.host,
        resolved_context.port,
        resolved_context.token,
        name="threadNameSet",
        payload={"instanceId": resolved_context.instance_id, "threadId": target.id, "name": new_name_value},
    )
    return f"Renamed {_quoted(target.display_name)} ({target.id}) -> {_quoted(new_name_value)}"


@mcp.tool
def robdex_send_message(
    agent_thread_id: str,
    text: str,
    thread_id: str | None = None,
    name: str | None = None,
    project_path: str | None = None,
    ctx: Context = None,
) -> str:
    """Send steering/user input to a target thread. Cross-project sends require orchestrator identity."""
    resolved_context = _resolve_context(agent_thread_id=agent_thread_id, tool_context=ctx)
    all_unarchived = _list_threads(resolved_context, archived=False)

    target = _resolve_thread_target(
        thread_id=thread_id,
        name=name,
        project_path=None if resolved_context.current_is_orchestrator else _resolve_project_scope(resolved_context, project_path),
        threads=all_unarchived,
    )

    if resolved_context.current_project_path and target.project_path and target.project_path != resolved_context.current_project_path:
        if not resolved_context.current_is_orchestrator:
            raise BridgeError("Only orchestrator threads can send messages across projects.")

    message = _compose_message(resolved_context, text)
    _send_text_to_thread(resolved_context, target.id, message)
    return f"Sent to {_quoted(target.display_name)} ({target.id})"


def main() -> None:
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
