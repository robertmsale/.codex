#!/usr/bin/env python3
from __future__ import annotations

import argparse
import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import robdex


COMPOSABLES_RESPONSE = {
    "items": [
        {
            "id": "review-evidence",
            "title": "Review Evidence",
            "scope": "global",
            "requirementCount": 1,
            "description": "Concrete review evidence.",
            "requirements": [
                {
                    "key": "reviewableArtifacts",
                    "statement": "Completion proof must include exact evidence.",
                    "severity": "high",
                    "verificationMethod": "manualEvidence",
                }
            ],
        }
    ]
}


class RobdexComposableRequirementsTests(unittest.TestCase):
    def test_list_composables_prints_available_items(self) -> None:
        args = argparse.Namespace(to_thread_id="worker-1", name=None, project_path=None)
        output = io.StringIO()
        with patch.object(robdex, "_request_json", return_value=COMPOSABLES_RESPONSE), contextlib.redirect_stdout(output):
            robdex._cmd_requirements_composables_list("orch-1", args)

        self.assertIn("review-evidence | global | count=1", output.getvalue())

    def test_show_composable_prints_details(self) -> None:
        args = argparse.Namespace(
            composable_id="review-evidence",
            to_thread_id="worker-1",
            name=None,
            project_path=None,
        )
        output = io.StringIO()
        with patch.object(robdex, "_request_json", return_value=COMPOSABLES_RESPONSE), contextlib.redirect_stdout(output):
            robdex._cmd_requirements_composables_show("orch-1", args)

        payload = json.loads(output.getvalue())
        self.assertEqual(payload["id"], "review-evidence")
        self.assertEqual(payload["requirements"][0]["key"], "reviewableArtifacts")

    def test_compose_merges_composables_with_task_requirements(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            requirements_file = Path(tmp) / "requirements.json"
            requirements_file.write_text(
                json.dumps(
                    {
                        "id": "task",
                        "title": "Task",
                        "requirements": [
                            {
                                "key": "taskRequirement",
                                "statement": "Task statement.",
                                "severity": "high",
                                "verificationMethod": "manualEvidence",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            args = argparse.Namespace(
                title="Task",
                requirements_file=str(requirements_file),
                include_composable=["review-evidence"],
                attach=False,
                to_thread_id="worker-1",
                name=None,
                project_path=None,
            )
            output = io.StringIO()
            with patch.object(robdex, "_request_json", return_value=COMPOSABLES_RESPONSE), contextlib.redirect_stdout(output):
                robdex._cmd_requirements_compose("orch-1", args)

        payload = json.loads(output.getvalue())
        keys = [item["key"] for item in payload["requirements"]]
        self.assertEqual(keys, ["reviewableArtifacts", "taskRequirement"])
        self.assertEqual(payload["includeComposables"], ["review-evidence"])

    def test_compose_attach_uses_set_requirements_route(self) -> None:
        calls: list[tuple[str, str, dict]] = []

        def fake_request(method: str, path: str, *, query=None, body=None):
            calls.append((method, path, body or {}))
            if path == "/orchestrator/requirements/composables":
                return COMPOSABLES_RESPONSE
            if path == "/orchestrator/requirements/set":
                return {
                    "displayName": "Worker",
                    "threadId": "worker-1",
                    "requirementCount": len(body["requirementSet"]["requirements"]),
                    "enforceOnTurns": True,
                }
            raise AssertionError(path)

        with tempfile.TemporaryDirectory() as tmp:
            requirements_file = Path(tmp) / "requirements.json"
            requirements_file.write_text(
                json.dumps(
                    {
                        "id": "task",
                        "title": "Task",
                        "requirements": [
                            {
                                "key": "taskRequirement",
                                "statement": "Task statement.",
                                "severity": "high",
                                "verificationMethod": "manualEvidence",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            args = argparse.Namespace(
                title="Task",
                requirements_file=str(requirements_file),
                include_composable=["review-evidence"],
                attach=True,
                to_thread_id="worker-1",
                name=None,
                project_path=None,
            )
            with patch.object(robdex, "_request_json", side_effect=fake_request), contextlib.redirect_stdout(io.StringIO()):
                robdex._cmd_requirements_compose("orch-1", args)

        set_call = [call for call in calls if call[1] == "/orchestrator/requirements/set"][0]
        requirement_set = set_call[2]["requirementSet"]
        self.assertEqual(set_call[2]["senderThreadId"], "orch-1")
        self.assertEqual(set_call[2]["recipientThreadId"], "worker-1")
        self.assertEqual(
            [item["key"] for item in requirement_set["requirements"]],
            ["reviewableArtifacts", "taskRequirement"],
        )


if __name__ == "__main__":
    unittest.main()
