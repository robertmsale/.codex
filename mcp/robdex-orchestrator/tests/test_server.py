from __future__ import annotations

import copy
import os
import unittest
from unittest.mock import patch

from robdex_orchestrator_mcp import server


class _ToolContext:
    def __init__(self, session_id: str) -> None:
        self.session_id = session_id


class RobdexInstanceFallbackTests(unittest.TestCase):
    def setUp(self) -> None:
        server.SESSION_THREAD_LOCKS.clear()

    def test_resolve_context_prefers_live_management_instance(self) -> None:
        with (
            patch.dict(
                os.environ,
                {
                    "CODEX_THREAD_ID": "orch-ezra",
                    "ROBDEX_INSTANCE_ID": "agent-1773012125-4e55",
                },
                clear=False,
            ),
            patch.object(server, "_run_command", return_value={"type": "instances", "payload": [{"id": "mgmt-global"}]}),
            patch.object(server, "_load_state", return_value=({}, {"/tmp/ezra": "orch-ezra"}, None)),
        ):
            ctx = server._resolve_context(
                from_thread_id="orch-ezra",
                tool_context=_ToolContext("test-session-resolve-context"),
            )

        self.assertEqual(ctx.instance_id, "mgmt-global")

    def test_current_role_uses_resolved_context(self) -> None:
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="mgmt-global",
            current_thread_id="orch-ezra",
            current_project_path="/tmp/ezra",
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-ezra": "Ezra Orchestrator"},
            orchestrator_by_project={"/tmp/ezra": "orch-ezra"},
        )

        with patch.object(server, "_resolve_context", return_value=resolved_context):
            role = server.robdex_current_role(from_thread_id="orch-ezra", ctx=None)
            whoami = server.robdex_whoami(from_thread_id="orch-ezra", ctx=None)

        self.assertEqual(role, "orchestrator")
        self.assertIn("role=orchestrator", whoami)
        self.assertIn("thread_id=orch-ezra", whoami)
        self.assertIn("project_path=/tmp/ezra", whoami)

    def test_current_role_returns_worker_when_sender_is_not_orchestrator(self) -> None:
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="mgmt-global",
            current_thread_id="worker-thread",
            current_project_path="/tmp/ezra",
            current_is_orchestrator=False,
            titles_by_thread_id={"worker-thread": "Worker Thread"},
            orchestrator_by_project={"/tmp/ezra": "orch-ezra"},
        )

        with patch.object(server, "_resolve_context", return_value=resolved_context):
            role = server.robdex_current_role(from_thread_id="worker-thread", ctx=None)

        self.assertEqual(role, "worker")

    def test_list_threads_recovers_from_stale_instance_id(self) -> None:
        ctx = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="agent-1773012125-4e55",
            current_thread_id="orch-ezra",
            current_project_path="/tmp/ezra",
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-ezra": "Ezra Orchestrator"},
            orchestrator_by_project={"/tmp/ezra": "orch-ezra"},
        )

        thread_list_result = {
            "type": "threadList",
            "payload": {
                "data": [
                    {
                        "id": "worker-thread",
                        "cwd": "/tmp/ezra/worker",
                        "preview": "Worker Thread",
                    }
                ]
            },
        }

        def fake_run_command(host: str, port: int, token: str | None, *, name: str, payload=None):
            if name == "threadList" and payload["instanceId"] == "agent-1773012125-4e55":
                raise server.BridgeError('instanceNotFound("agent-1773012125-4e55")')
            if name == "listInstances":
                return {"type": "instances", "payload": [{"id": "mgmt-global"}]}
            if name == "threadList" and payload["instanceId"] == "mgmt-global":
                return thread_list_result
            raise AssertionError(f"Unexpected call: {name} {payload}")

        with (
            patch.object(server, "_run_command", side_effect=fake_run_command),
            patch.object(server, "_load_thread_metadata_map", return_value={}),
        ):
            threads = server._list_threads(ctx, archived=False)

        self.assertEqual([thread.id for thread in threads], ["worker-thread"])


class RobdexListAgentsScopeTests(unittest.TestCase):
    def test_parse_thread_list_requires_real_path_boundaries(self) -> None:
        result = {
            "type": "threadList",
            "payload": {
                "data": [
                    {
                        "id": "thread-1",
                        "cwd": "/tmp/ezra-other/task",
                        "preview": "foreign thread",
                    }
                ]
            },
        }

        threads = server._parse_thread_list(
            result,
            titles_by_thread_id={},
            orchestrator_by_project={"/tmp/ezra": "orch-ezra"},
            current_project="/tmp/ezra",
        )

        self.assertEqual(len(threads), 1)
        self.assertIsNone(threads[0].project_path)

    def test_list_threads_filters_hidden_threads_by_default(self) -> None:
        ctx = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="instance",
            current_thread_id="orch-ezra",
            current_project_path="/tmp/ezra",
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-ezra": "Ezra Orchestrator"},
            orchestrator_by_project={"/tmp/ezra": "orch-ezra"},
        )
        result = {
            "type": "threadList",
            "payload": {
                "data": [
                    {
                        "id": "visible-thread",
                        "cwd": "/tmp/ezra/visible",
                        "preview": "Visible Thread",
                    },
                    {
                        "id": "hidden-thread",
                        "cwd": "/tmp/ezra/hidden",
                        "preview": "Hidden Thread",
                    },
                ]
            },
        }

        with (
            patch.object(server, "_run_command", return_value=result),
            patch.object(
                server,
                "_load_thread_metadata_map",
                return_value={
                    "hidden-thread": {
                        "displayName": "Hidden Thread",
                        "hidden": True,
                    }
                },
            ),
        ):
            threads = server._list_threads(ctx, archived=False)

        self.assertEqual([thread.id for thread in threads], ["visible-thread"])

    def test_list_agents_uses_scoped_bridge_endpoint(self) -> None:
        project_path = server._normalized_path("/tmp/ezra") or "/tmp/ezra"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token="bridge-token",
            instance_id="instance",
            current_thread_id="orchestrator-thread",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orchestrator-thread": "Ezra Orchestrator"},
            orchestrator_by_project={project_path: "orchestrator-thread"},
        )
        requests: list[dict] = []
        payload = {
            "items": [
                {
                    "id": "orchestrator-thread",
                    "displayName": "Ezra Orchestrator",
                    "projectPath": project_path,
                    "cwd": project_path,
                    "isOrchestrator": True,
                    "isRunning": True,
                    "issueNumber": None,
                    "pullRequestNumber": None,
                    "blockedReason": None,
                    "unblockWhen": None,
                    "updatedAt": 1234567890,
                },
                {
                    "id": "worker-thread",
                    "displayName": "Accounting Command Center Completion",
                    "projectPath": project_path,
                    "cwd": f"{project_path}/.worktrees/accounting",
                    "isOrchestrator": False,
                    "isRunning": True,
                    "issueNumber": 624,
                    "pullRequestNumber": 712,
                    "blockedReason": "waiting on merge",
                    "unblockWhen": "after sync",
                    "updatedAt": 1234567891,
                },
            ]
        }

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(
                server,
                "_http_json_request",
                side_effect=lambda host, port, token, **kwargs: requests.append(
                    {"host": host, "port": port, "token": token, **kwargs}
                )
                or payload,
            ),
        ):
            result = server.robdex_list_agents(from_thread_id="orchestrator-thread", ctx=None)

        self.assertEqual(
            requests,
            [
                {
                    "host": "127.0.0.1",
                    "port": 42080,
                    "token": "bridge-token",
                    "method": "GET",
                    "path": "/orchestrator/agents",
                    "query": {
                        "senderThreadId": "orchestrator-thread",
                        "includeArchived": "0",
                    },
                }
            ],
        )
        self.assertIn('**YOU** "Ezra Orchestrator" (orchestrator-thread) [orchestrator]', result)
        self.assertIn('"Accounting Command Center Completion" (worker-thread)', result)
        self.assertIn('issue=#624', result)
        self.assertIn('pr=#712', result)
        self.assertIn('blocked="waiting on merge" until="after sync"', result)

    def test_list_agents_passes_include_archived_to_bridge(self) -> None:
        project_path = server._normalized_path("/tmp/ezra") or "/tmp/ezra"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="instance",
            current_thread_id="orchestrator-thread",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orchestrator-thread": "Ezra Orchestrator"},
            orchestrator_by_project={project_path: "orchestrator-thread"},
        )
        requests: list[dict] = []

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(
                server,
                "_http_json_request",
                side_effect=lambda host, port, token, **kwargs: requests.append(kwargs) or {"items": []},
            ),
        ):
            result = server.robdex_list_agents(
                from_thread_id="orchestrator-thread",
                include_archived=True,
                ctx=None,
            )

        self.assertEqual(result, "(no matching threads)")
        self.assertEqual(
            requests,
            [
                {
                    "method": "GET",
                    "path": "/orchestrator/agents",
                    "query": {
                        "senderThreadId": "orchestrator-thread",
                        "includeArchived": "1",
                    },
                }
            ],
        )

    def test_list_agents_compacts_and_clips_blob_like_display_names(self) -> None:
        project_path = server._normalized_path("/tmp/ezra") or "/tmp/ezra"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="instance",
            current_thread_id="orchestrator-thread",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orchestrator-thread": "Ezra Orchestrator"},
            orchestrator_by_project={project_path: "orchestrator-thread"},
        )
        payload = {
            "items": [
                {
                    "id": "archived-worker-thread",
                    "displayName": "Spawned by Ezra Orchestrator\n\nThis is an extremely long historical prompt blob that should never be rendered verbatim in archived stewardship listings because it makes cleanup unreadable.",
                    "projectPath": project_path,
                    "cwd": f"{project_path}/.worktrees/archive-smoke",
                    "isOrchestrator": False,
                    "isRunning": False,
                    "issueNumber": 777,
                    "pullRequestNumber": 901,
                    "blockedReason": None,
                    "unblockWhen": None,
                    "updatedAt": 1234567899,
                }
            ]
        }

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(server, "_http_json_request", return_value=payload),
        ):
            result = server.robdex_list_agents(
                from_thread_id="orchestrator-thread",
                include_archived=True,
                ctx=None,
            )

        self.assertIn('"Spawned by Ezra Orchestrator This is an extremely long historical prompt blob', result)
        self.assertNotIn("\n\nThis is an extremely long historical prompt blob", result)
        self.assertIn("...", result)
        self.assertIn("issue=#777", result)
        self.assertIn("pr=#901", result)


class RobdexSendMessageTests(unittest.TestCase):
    def test_send_message_posts_to_scoped_bridge_endpoint_by_thread_id(self) -> None:
        project_path = server._normalized_path("/tmp/ezra") or "/tmp/ezra"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token="bridge-token",
            instance_id="instance",
            current_thread_id="orch-ezra",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-ezra": "Ezra Orchestrator"},
            orchestrator_by_project={project_path: "orch-ezra"},
        )
        requests: list[dict] = []

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(
                server,
                "_http_json_request",
                side_effect=lambda host, port, token, **kwargs: requests.append(
                    {"host": host, "port": port, "token": token, **kwargs}
                )
                or {
                    "recipientThreadId": "worker-thread",
                    "recipientDisplayName": "Accounting Command Center Completion",
                    "turnId": "turn-123",
                },
            ),
        ):
            result = server.robdex_send_message(
                from_thread_id="orch-ezra",
                to_thread_id="worker-thread",
                text="Please sync to main and rerun validation.",
                ctx=None,
            )

        self.assertEqual(result, 'Sent to "Accounting Command Center Completion" (worker-thread)')
        self.assertEqual(len(requests), 1)
        self.assertEqual(requests[0]["method"], "POST")
        self.assertEqual(requests[0]["path"], "/orchestrator/agent-message")
        self.assertEqual(
            requests[0]["body"],
            {
                "senderThreadId": "orch-ezra",
                "recipientThreadId": "worker-thread",
                "recipientName": None,
                "text": "[Ezra Orchestrator]: Please sync to main and rerun validation.\n\n"
                + server.CONTINUATION_SUFFIX,
            },
        )

    def test_send_message_resolves_name_within_scoped_project_agents(self) -> None:
        project_path = server._normalized_path("/tmp/ezra") or "/tmp/ezra"
        other_project_path = server._normalized_path("/tmp/other") or "/tmp/other"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="instance",
            current_thread_id="orch-ezra",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-ezra": "Ezra Orchestrator"},
            orchestrator_by_project={project_path: "orch-ezra"},
        )
        scoped_agents_payload = {
            "items": [
                {
                    "id": "worker-thread",
                    "displayName": "Issue 624 Accounting UI Alignment",
                    "projectPath": project_path,
                    "cwd": f"{project_path}/.worktrees/accounting-ui",
                    "isOrchestrator": False,
                    "isRunning": True,
                    "issueNumber": 624,
                    "pullRequestNumber": None,
                    "blockedReason": None,
                    "unblockWhen": None,
                    "updatedAt": 1234567892,
                },
                {
                    "id": "other-thread",
                    "displayName": "Issue 624 Accounting UI Alignment",
                    "projectPath": other_project_path,
                    "cwd": f"{other_project_path}/.worktrees/accounting-ui",
                    "isOrchestrator": False,
                    "isRunning": True,
                    "issueNumber": 624,
                    "pullRequestNumber": None,
                    "blockedReason": None,
                    "unblockWhen": None,
                    "updatedAt": 1234567893,
                },
            ]
        }
        requests: list[dict] = []

        def fake_http_json_request(host: str, port: int, token: str | None, **kwargs):
            requests.append({"host": host, "port": port, "token": token, **kwargs})
            if kwargs["method"] == "GET":
                return scoped_agents_payload
            return {
                "recipientThreadId": "worker-thread",
                "recipientDisplayName": "Issue 624 Accounting UI Alignment",
                "turnId": "turn-456",
            }

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(server, "_http_json_request", side_effect=fake_http_json_request),
        ):
            result = server.robdex_send_message(
                from_thread_id="orch-ezra",
                name="Issue 624 Accounting UI Alignment",
                project_path=project_path,
                text="Status check.",
                ctx=None,
            )

        self.assertEqual(result, 'Sent to "Issue 624 Accounting UI Alignment" (worker-thread)')
        self.assertEqual(len(requests), 2)
        self.assertEqual(requests[0]["method"], "GET")
        self.assertEqual(requests[0]["path"], "/orchestrator/agents")
        self.assertEqual(requests[1]["method"], "POST")
        self.assertEqual(
            requests[1]["body"],
            {
                "senderThreadId": "orch-ezra",
                "recipientThreadId": "worker-thread",
                "recipientName": None,
                "text": "[Ezra Orchestrator]: Status check.\n\n" + server.CONTINUATION_SUFFIX,
            },
        )

    def test_unarchive_agent_prompt_uses_scoped_bridge_message_endpoint(self) -> None:
        project_path = server._normalized_path("/tmp/ezra") or "/tmp/ezra"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="mgmt-global",
            current_thread_id="orch-ezra",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-ezra": "Ezra Orchestrator"},
            orchestrator_by_project={project_path: "orch-ezra"},
        )
        archived_thread = server.ThreadEntry(
            id="worker-thread",
            cwd="/tmp/ezra/.worktrees/worker",
            preview="Issue 624 Accounting UI Alignment",
            display_name="Issue 624 Accounting UI Alignment",
            project_path=project_path,
            has_custom_title=True,
        )
        requests: list[dict] = []

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(server, "_list_threads", side_effect=[[], [archived_thread]]),
            patch.object(server, "_run_instance_command", return_value={}),
            patch.object(
                server,
                "_http_json_request",
                side_effect=lambda host, port, token, **kwargs: requests.append(kwargs)
                or {"recipientThreadId": "worker-thread", "recipientDisplayName": archived_thread.display_name, "turnId": "turn-789"},
            ),
        ):
            result = server.robdex_unarchive_agent(
                from_thread_id="orch-ezra",
                name=archived_thread.display_name,
                prompt="Resume work.",
                ctx=None,
            )

        self.assertEqual(result, f'Unarchived "{archived_thread.display_name}" ({archived_thread.id})')
        self.assertEqual(
            requests,
            [
                {
                    "method": "POST",
                    "path": "/orchestrator/agent-message",
                    "body": {
                        "senderThreadId": "orch-ezra",
                        "recipientThreadId": "worker-thread",
                        "recipientName": None,
                        "text": "[Ezra Orchestrator]: Resume work.\n\n" + server.CONTINUATION_SUFFIX,
                    },
                }
            ],
        )


class RobdexApprovalRoutingTests(unittest.TestCase):
    def test_list_pending_approvals_reads_snapshot_and_filters_visible_threads(self) -> None:
        project_path = server._normalized_path("/tmp/codex") or "/tmp/codex"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token="bridge-token",
            instance_id="mgmt-global",
            current_thread_id="orch-codex",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-codex": "Codex Config Orchestrator"},
            orchestrator_by_project={project_path: "orch-codex"},
        )
        requests: list[dict] = []
        visible_worker = {
            "id": "worker-thread",
            "displayName": "Route Approval E2E Smoke",
            "projectPath": project_path,
            "cwd": f"{project_path}/.worktrees/approval-smoke",
            "isOrchestrator": False,
            "isRunning": True,
            "issueNumber": None,
            "pullRequestNumber": None,
            "blockedReason": None,
            "unblockWhen": None,
            "updatedAt": 1234567890,
        }
        hidden_foreign = {
            "id": "foreign-thread",
            "displayName": "Foreign Worker",
            "projectPath": server._normalized_path("/tmp/other") or "/tmp/other",
            "cwd": "/tmp/other/.worktrees/foreign",
            "isOrchestrator": False,
            "isRunning": True,
            "issueNumber": None,
            "pullRequestNumber": None,
            "blockedReason": None,
            "unblockWhen": None,
            "updatedAt": 1234567891,
        }

        def fake_http_json_request(host: str, port: int, token: str | None, **kwargs):
            requests.append({"host": host, "port": port, "token": token, **kwargs})
            if kwargs["path"] == "/state/snapshot":
                return {
                    "pendingApprovals": [
                        {
                            "id": "agent-1773162405-dec9:0",
                            "instanceID": "agent-1773162405-dec9",
                            "requestID": 0,
                            "threadID": "worker-thread",
                            "turnID": "turn-1",
                            "itemID": "item-1",
                            "kind": {"commandExecution": {}},
                            "title": "Command approval is pending.",
                            "detail": "Do you want to allow the single smoke-test fetch?",
                            "approvalReason": "Needed to verify the worktree can sync cleanly.",
                            "command": "git -C /tmp/codex fetch origin main",
                            "commandCWD": project_path,
                            "fileGrantRoot": None,
                            "fileChanges": [],
                        },
                        {
                            "id": "agent-foreign:1",
                            "instanceID": "agent-foreign",
                            "requestID": 1,
                            "threadID": "foreign-thread",
                            "turnID": "turn-2",
                            "itemID": "item-2",
                            "kind": {"commandExecution": {}},
                            "title": "Foreign approval",
                            "detail": None,
                            "approvalReason": None,
                            "command": "git status",
                            "commandCWD": "/tmp/other",
                            "fileGrantRoot": None,
                            "fileChanges": [],
                        },
                    ]
                }
            if kwargs["path"] == "/orchestrator/agents":
                return {"items": [visible_worker, hidden_foreign]}
            raise AssertionError(f"Unexpected request: {kwargs}")

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(server, "_http_json_request", side_effect=fake_http_json_request),
        ):
            result = server.robdex_list_pending_approvals(from_thread_id="orch-codex", ctx=None)

        self.assertIn("agent-1773162405-dec9:0", result)
        self.assertIn('thread="Route Approval E2E Smoke" (worker-thread)', result)
        self.assertIn('reason="Needed to verify the worktree can sync cleanly."', result)
        self.assertNotIn("agent-foreign:1", result)
        self.assertEqual(requests[0]["path"], "/state/snapshot")
        self.assertEqual(requests[0]["query"], {"includeMessageCache": "0"})

    def test_list_pending_approvals_prefers_file_change_summary(self) -> None:
        project_path = server._normalized_path("/tmp/codex") or "/tmp/codex"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token="bridge-token",
            instance_id="mgmt-global",
            current_thread_id="orch-codex",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-codex": "Codex Config Orchestrator"},
            orchestrator_by_project={project_path: "orch-codex"},
        )
        approval = server.PendingApprovalEntry(
            id="agent-1773162405-dec9:file-7",
            instance_id="agent-1773162405-dec9",
            request_id="file-7",
            request_id_display="file-7",
            thread_id="worker-thread",
            turn_id="turn-2",
            item_id="item-2",
            kind="fileChange",
            title="File approval is pending.",
            detail="Grant write access to a temp directory.",
            approval_reason="Please narrow this to AppState.swift first.",
            command=None,
            command_cwd=None,
            file_grant_root="/tmp/codex",
            file_changes=[
                server.PendingApprovalFileChangeEntry(
                    path="AppState.swift",
                    kind="update",
                    diff="@@ -1 +1 @@",
                ),
                server.PendingApprovalFileChangeEntry(
                    path="Cache.swift",
                    kind="create",
                    diff=None,
                ),
            ],
        )
        visible_thread = server.ThreadEntry(
            id="worker-thread",
            cwd=f"{project_path}/.worktrees/approval-smoke",
            preview="Route Approval E2E Smoke",
            display_name="Route Approval E2E Smoke",
            project_path=project_path,
            has_custom_title=True,
        )

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(server, "_list_pending_approvals", return_value=[approval]),
            patch.object(server, "_list_scoped_agents", return_value=[visible_thread]),
        ):
            result = server.robdex_list_pending_approvals(from_thread_id="orch-codex", ctx=None)

        self.assertIn('detail="update AppState.swift, create Cache.swift"', result)
        self.assertIn('reason="Please narrow this to AppState.swift first."', result)
        self.assertNotIn('/tmp/codex"', result)

    def test_approve_approval_accepts_short_request_id_for_command_approval(self) -> None:
        project_path = server._normalized_path("/tmp/codex") or "/tmp/codex"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token="bridge-token",
            instance_id="mgmt-global",
            current_thread_id="orch-codex",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-codex": "Codex Config Orchestrator"},
            orchestrator_by_project={project_path: "orch-codex"},
        )
        approval = server.PendingApprovalEntry(
            id="agent-1773162405-dec9:0",
            instance_id="agent-1773162405-dec9",
            request_id=0,
            request_id_display="0",
            thread_id="worker-thread",
            turn_id="turn-1",
            item_id="item-1",
            kind="commandExecution",
            title="Command approval is pending.",
            detail="Allow the fetch.",
            approval_reason=None,
            command="git -C /tmp/codex fetch origin main",
            command_cwd=project_path,
            file_grant_root=None,
            file_changes=[],
        )
        visible_thread = server.ThreadEntry(
            id="worker-thread",
            cwd=f"{project_path}/.worktrees/approval-smoke",
            preview="Route Approval E2E Smoke",
            display_name="Route Approval E2E Smoke",
            project_path=project_path,
            has_custom_title=True,
        )

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(server, "_list_pending_approvals", return_value=[approval]),
            patch.object(server, "_list_scoped_agents", return_value=[visible_thread]),
            patch.object(server, "_run_command", return_value={"type": "empty"}) as run_command,
        ):
            result = server.robdex_approve_approval(
                from_thread_id="orch-codex",
                approval_id="0",
                ctx=None,
            )

        self.assertEqual(result, 'Approved "agent-1773162405-dec9:0" for "Route Approval E2E Smoke" (worker-thread)')
        run_command.assert_called_once_with(
            "127.0.0.1",
            42080,
            "bridge-token",
            name="commandApproval",
            payload={
                "instanceId": "agent-1773162405-dec9",
                "requestId": 0,
                "decision": "accept",
            },
        )

    def test_decline_approval_uses_file_approval_command(self) -> None:
        project_path = server._normalized_path("/tmp/codex") or "/tmp/codex"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="mgmt-global",
            current_thread_id="orch-codex",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-codex": "Codex Config Orchestrator"},
            orchestrator_by_project={project_path: "orch-codex"},
        )
        approval = server.PendingApprovalEntry(
            id="agent-1773162405-dec9:file-7",
            instance_id="agent-1773162405-dec9",
            request_id="file-7",
            request_id_display="file-7",
            thread_id="worker-thread",
            turn_id="turn-2",
            item_id="item-2",
            kind="fileChange",
            title="File approval is pending.",
            detail="Grant write access to a temp directory.",
            approval_reason="Please narrow this to AppState.swift first.",
            command=None,
            command_cwd=None,
            file_grant_root="/tmp/codex",
            file_changes=[],
        )
        visible_thread = server.ThreadEntry(
            id="worker-thread",
            cwd=f"{project_path}/.worktrees/approval-smoke",
            preview="Route Approval E2E Smoke",
            display_name="Route Approval E2E Smoke",
            project_path=project_path,
            has_custom_title=True,
        )

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(server, "_list_pending_approvals", return_value=[approval]),
            patch.object(server, "_list_scoped_agents", return_value=[visible_thread]),
            patch.object(
                server,
                "_run_command",
                return_value={
                    "type": "approvalResult",
                    "payload": {
                        "followUpMessageRequested": True,
                        "followUpMessageSent": False,
                        "followUpError": "worker thread not reachable",
                    },
                },
            ) as run_command,
        ):
            result = server.robdex_decline_approval(
                from_thread_id="orch-codex",
                approval_id="agent-1773162405-dec9:file-7",
                message="Please narrow this to AppState.swift first.",
                ctx=None,
            )

        self.assertEqual(
            result,
            'Declined "agent-1773162405-dec9:file-7" for "Route Approval E2E Smoke" (worker-thread) | follow-up error="worker thread not reachable" | approval decision already applied',
        )
        run_command.assert_called_once_with(
            "127.0.0.1",
            42080,
            None,
            name="fileApproval",
            payload={
                "instanceId": "agent-1773162405-dec9",
                "requestId": "file-7",
                "decision": "decline",
                "message": "Please narrow this to AppState.swift first.",
            },
        )


class RobdexArchiveAgentTests(unittest.TestCase):
    def test_archive_agent_posts_to_scoped_bridge_endpoint(self) -> None:
        project_path = server._normalized_path("/tmp/ezra") or "/tmp/ezra"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token="bridge-token",
            instance_id="instance",
            current_thread_id="orch-ezra",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-ezra": "Ezra Orchestrator"},
            orchestrator_by_project={project_path: "orch-ezra"},
        )
        requests: list[dict] = []

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(
                server,
                "_http_json_request",
                side_effect=lambda host, port, token, **kwargs: requests.append(
                    {"host": host, "port": port, "token": token, **kwargs}
                )
                or {
                    "recipientThreadId": "worker-thread",
                    "recipientDisplayName": "Issue 677 Realtime Backend Hardening",
                    "alreadyArchived": False,
                },
            ),
        ):
            result = server.robdex_archive_agent(
                from_thread_id="orch-ezra",
                name="Issue 677 Realtime Backend Hardening",
                project_path=project_path,
                ctx=None,
            )

        self.assertEqual(result, 'Archived "Issue 677 Realtime Backend Hardening" (worker-thread)')
        self.assertEqual(
            requests,
            [
                {
                    "host": "127.0.0.1",
                    "port": 42080,
                    "token": "bridge-token",
                    "method": "POST",
                    "path": "/orchestrator/archive-agent",
                    "body": {
                        "senderThreadId": "orch-ezra",
                        "recipientThreadId": None,
                        "recipientName": "Issue 677 Realtime Backend Hardening",
                        "projectPath": project_path,
                    },
                }
            ],
        )

    def test_archive_agent_reports_already_archived(self) -> None:
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="instance",
            current_thread_id="orch-ezra",
            current_project_path="/tmp/ezra",
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-ezra": "Ezra Orchestrator"},
            orchestrator_by_project={"/tmp/ezra": "orch-ezra"},
        )

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(
                server,
                "_http_json_request",
                return_value={
                    "recipientThreadId": "worker-thread",
                    "recipientDisplayName": "Completed Worker",
                    "alreadyArchived": True,
                },
            ),
        ):
            result = server.robdex_archive_agent(
                from_thread_id="orch-ezra",
                to_thread_id="worker-thread",
                ctx=None,
            )

        self.assertEqual(result, 'Already archived "Completed Worker" (worker-thread)')


class RobdexThreadGroupTests(unittest.TestCase):
    def setUp(self) -> None:
        server.SESSION_THREAD_LOCKS.clear()

    def test_create_thread_group_detaches_seed_thread_from_existing_group(self) -> None:
        project_path = server._normalized_path("/tmp/ezra") or "/tmp/ezra"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="mgmt-global",
            current_thread_id="orch-ezra",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-ezra": "Ezra Orchestrator"},
            orchestrator_by_project={project_path: "orch-ezra"},
        )
        initial_payload = {
            "threadGroupsByProjectPath": {
                project_path: [
                    {
                        "id": "group-existing",
                        "title": "Existing",
                        "threadIDs": ["thread-1"],
                        "isCollapsed": False,
                        "createdAt": 1.0,
                        "updatedAt": 1.0,
                    }
                ]
            }
        }
        writes: list[dict] = []

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(server, "_load_state_payload", side_effect=lambda: copy.deepcopy(initial_payload)),
            patch.object(server, "_write_state_payload", side_effect=writes.append),
        ):
            result = server.robdex_create_thread_group(
                from_thread_id="orch-ezra",
                title="API Cleanup",
                seed_thread_id="thread-1",
                ctx=None,
            )

        self.assertIn("Created", result)
        written_groups = writes[-1]["threadGroupsByProjectPath"][project_path]
        existing_group = next(group for group in written_groups if group["id"] == "group-existing")
        created_group = next(group for group in written_groups if group["id"] != "group-existing")
        self.assertEqual(existing_group["threadIDs"], [])
        self.assertEqual(created_group["title"], "API Cleanup")
        self.assertEqual(created_group["threadIDs"], ["thread-1"])

    def test_move_thread_to_group_keeps_single_group_membership(self) -> None:
        project_path = server._normalized_path("/tmp/ezra") or "/tmp/ezra"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="mgmt-global",
            current_thread_id="orch-ezra",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-ezra": "Ezra Orchestrator"},
            orchestrator_by_project={project_path: "orch-ezra"},
        )
        initial_payload = {
            "threadGroupsByProjectPath": {
                project_path: [
                    {
                        "id": "group-a",
                        "title": "Group A",
                        "threadIDs": ["thread-1"],
                        "isCollapsed": False,
                        "createdAt": 1.0,
                        "updatedAt": 1.0,
                    },
                    {
                        "id": "group-b",
                        "title": "Group B",
                        "threadIDs": [],
                        "isCollapsed": False,
                        "createdAt": 2.0,
                        "updatedAt": 2.0,
                    },
                ]
            }
        }
        writes: list[dict] = []

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(server, "_load_state_payload", side_effect=lambda: copy.deepcopy(initial_payload)),
            patch.object(server, "_write_state_payload", side_effect=writes.append),
        ):
            result = server.robdex_move_thread_to_group(
                from_thread_id="orch-ezra",
                thread_id="thread-1",
                group_id="group-b",
                ctx=None,
            )

        self.assertIn("group-b", result)
        written_groups = writes[-1]["threadGroupsByProjectPath"][project_path]
        group_a = next(group for group in written_groups if group["id"] == "group-a")
        group_b = next(group for group in written_groups if group["id"] == "group-b")
        self.assertEqual(group_a["threadIDs"], [])
        self.assertEqual(group_b["threadIDs"], ["thread-1"])

    def test_archive_thread_group_archives_only_active_same_project_threads(self) -> None:
        project_path = server._normalized_path("/tmp/ezra") or "/tmp/ezra"
        other_project_path = server._normalized_path("/tmp/other") or "/tmp/other"
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="mgmt-global",
            current_thread_id="orch-ezra",
            current_project_path=project_path,
            current_is_orchestrator=True,
            titles_by_thread_id={"orch-ezra": "Ezra Orchestrator"},
            orchestrator_by_project={project_path: "orch-ezra"},
        )
        active_thread = server.ThreadEntry(
            id="active-thread",
            cwd="/tmp/ezra/active",
            preview="Active",
            display_name="Active",
            project_path=project_path,
            has_custom_title=True,
        )
        foreign_thread = server.ThreadEntry(
            id="foreign-thread",
            cwd="/tmp/other/foreign",
            preview="Foreign",
            display_name="Foreign",
            project_path=other_project_path,
            has_custom_title=True,
        )
        archived_thread = server.ThreadEntry(
            id="archived-thread",
            cwd="/tmp/ezra/archived",
            preview="Archived",
            display_name="Archived",
            project_path=project_path,
            has_custom_title=True,
        )
        archived_calls: list[tuple[str, dict]] = []

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(
                server,
                "_load_thread_groups_by_project",
                return_value={
                    project_path: [
                        server.ThreadGroupEntry(
                            id="group-1",
                            title="Hotfixes",
                            thread_ids=["active-thread", "archived-thread", "foreign-thread", "missing-thread"],
                            is_collapsed=False,
                            created_at=1.0,
                            updated_at=1.0,
                        )
                    ]
                },
            ),
            patch.object(server, "_list_threads", side_effect=[[active_thread, foreign_thread], [archived_thread]]),
            patch.object(
                server,
                "_run_instance_command",
                side_effect=lambda host, port, token, *, name, payload: archived_calls.append((name, payload)) or {},
            ),
        ):
            result = server.robdex_archive_thread_group(
                from_thread_id="orch-ezra",
                group_id="group-1",
                ctx=None,
            )

        self.assertIn("archived=active-thread", result)
        self.assertIn("skipped=archived-thread,foreign-thread,missing-thread", result)
        self.assertEqual(
            archived_calls,
            [("threadArchive", {"instanceId": "mgmt-global", "threadId": "active-thread"})],
        )


if __name__ == "__main__":
    unittest.main()
