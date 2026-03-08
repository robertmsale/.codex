from __future__ import annotations

import unittest
from unittest.mock import patch

from robdex_orchestrator_mcp import server


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
