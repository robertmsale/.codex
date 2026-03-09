from __future__ import annotations

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

    def test_list_agents_rejects_unscoped_default_listing(self) -> None:
        resolved_context = server.Context(
            host="127.0.0.1",
            port=42080,
            token=None,
            instance_id="instance",
            current_thread_id="orchestrator-thread",
            current_project_path=None,
            current_is_orchestrator=True,
            titles_by_thread_id={"orchestrator-thread": "Ezra Orchestrator"},
            orchestrator_by_project={"/tmp/ezra": "orchestrator-thread"},
        )

        with (
            patch.object(server, "_resolve_context", return_value=resolved_context),
            patch.object(server, "_list_threads", return_value=[]),
        ):
            with self.assertRaisesRegex(server.BridgeError, "Unable to resolve project scope"):
                server.robdex_list_agents(from_thread_id="orchestrator-thread", ctx=None)


if __name__ == "__main__":
    unittest.main()
