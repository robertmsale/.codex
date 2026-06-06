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
            "permanent": True,
            "permanentSource": "project",
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
    def _requirements_file(self, tmp: str) -> Path:
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
        return requirements_file

    def test_manual_requirements_review_command_is_not_registered(self) -> None:
        parser, _handoff_parser, _spawn_parser = robdex.build_parser()
        help_text = parser.format_help()
        removed_command = "request-" + "requirements-review"
        self.assertNotIn(removed_command, help_text)
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            parser.parse_args([removed_command, "--name", "Worker"])

    def test_list_composables_prints_available_items(self) -> None:
        args = argparse.Namespace(to_thread_id="worker-1", name=None, project_path=None)
        output = io.StringIO()
        with patch.object(robdex, "_request_json", return_value=COMPOSABLES_RESPONSE), contextlib.redirect_stdout(output):
            robdex._cmd_requirements_composables_list("orch-1", args)

        self.assertIn("review-evidence | global | count=1", output.getvalue())
        self.assertIn("permanent", output.getvalue())

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
        self.assertTrue(payload["permanent"])
        self.assertEqual(payload["permanentSource"], "project")
        self.assertEqual(payload["requirements"][0]["key"], "reviewableArtifacts")

    def test_manifest_activate_posts_file_and_prints_worker(self) -> None:
        calls: list[tuple[str, str, dict]] = []

        def fake_request(method: str, path: str, *, query=None, body=None):
            calls.append((method, path, body or {}))
            return {
                "runId": "run-1",
                "currentPhaseId": "phase-1",
                "workerThreadId": "worker-1",
            }

        args = argparse.Namespace(file="/tmp/project/.codex/manifests/plan.md")
        output = io.StringIO()
        with patch.object(robdex, "_request_json", side_effect=fake_request), contextlib.redirect_stdout(output):
            robdex._cmd_manifest_activate("orch-1", args)

        self.assertEqual(calls[0][1], "/orchestrator/manifest/activate")
        self.assertEqual(calls[0][2]["senderThreadId"], "orch-1")
        self.assertTrue(calls[0][2]["file"].endswith("/tmp/project/.codex/manifests/plan.md"))
        self.assertIn("Activated manifest run-1", output.getvalue())
        self.assertIn("worker=worker-1", output.getvalue())

    def test_manifest_status_prints_phase_timeline(self) -> None:
        args = argparse.Namespace(project_path="/tmp/project", run_id=None)
        output = io.StringIO()
        with patch.object(
            robdex,
            "_request_json",
            return_value={
                "runs": [
                    {
                        "runId": "run-1",
                        "planId": "plan",
                        "status": "active",
                        "currentPhaseId": "phase-1",
                        "phases": [
                            {
                                "phaseId": "phase-1",
                                "status": "running",
                                "workerThreadId": "worker-1",
                                "archiveCleanupState": "notReady",
                            }
                        ],
                    }
                ]
            },
        ), contextlib.redirect_stdout(output):
            robdex._cmd_manifest_status("orch-1", args)

        text = output.getvalue()
        self.assertIn("run-1 | plan=plan | status=active | current=phase-1", text)
        self.assertIn("phase-1: running | worker=worker-1 | cleanup=notReady", text)

    def test_manifest_advance_cancel_and_decision_post_expected_routes(self) -> None:
        calls: list[tuple[str, str, dict]] = []

        def fake_request(method: str, path: str, *, query=None, body=None):
            calls.append((method, path, body or {}))
            if path.endswith("/advance"):
                return {
                    "advancedPhaseId": "phase-1",
                    "archivedWorkerThreadId": "worker-1",
                    "archiveCleanupState": "archived",
                    "nextWorkerThreadId": "worker-2",
                }
            return {"run": {"runId": "run-1", "status": "cancelled"}}

        advance_args = argparse.Namespace(run_id="run-1", handoff_file="/tmp/handoff.md")
        cancel_args = argparse.Namespace(run_id="run-1", reason="stop")
        decision_args = argparse.Namespace(
            run_id="run-1",
            phase_id="phase-1",
            type="blocker",
            text="blocked",
            text_file=None,
            text_stdin=False,
        )
        with patch.object(robdex, "_request_json", side_effect=fake_request), contextlib.redirect_stdout(io.StringIO()):
            robdex._cmd_manifest_advance("orch-1", advance_args)
            robdex._cmd_manifest_cancel("orch-1", cancel_args)
            robdex._cmd_manifest_decision("orch-1", decision_args)

        self.assertEqual(calls[0][1], "/orchestrator/manifest/advance")
        self.assertTrue(calls[0][2]["handoffFile"].endswith("/tmp/handoff.md"))
        self.assertEqual(calls[1][1], "/orchestrator/manifest/cancel")
        self.assertEqual(calls[1][2]["reason"], "stop")
        self.assertEqual(calls[2][1], "/orchestrator/manifest/decision")
        self.assertEqual(calls[2][2]["type"], "blocker")

    def test_manifest_cli_requires_advance_handoff_file_and_valid_decision_type(self) -> None:
        parser, _handoff_parser, _spawn_parser = robdex.build_parser()
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            parser.parse_args(["manifest", "advance", "--run-id", "run-1"])
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            parser.parse_args(
                [
                    "manifest",
                    "decision",
                    "--run-id",
                    "run-1",
                    "--phase-id",
                    "phase-1",
                    "--type",
                    "skip",
                    "--text",
                    "nope",
                ]
            )

    def test_manifest_cli_surfaces_bridge_errors_without_success_output(self) -> None:
        args = argparse.Namespace(run_id="run-1", handoff_file="/tmp/handoff.md")
        output = io.StringIO()
        with patch.object(robdex, "_request_json", side_effect=SystemExit("advance denied: missing handoff")), contextlib.redirect_stdout(output):
            with self.assertRaises(SystemExit) as raised:
                robdex._cmd_manifest_advance("orch-1", args)

        self.assertIn("advance denied", str(raised.exception))
        self.assertEqual(output.getvalue(), "")

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

    def test_requirements_from_prose_accepts_composables_in_parser(self) -> None:
        parser, _handoff_parser, _spawn_parser = robdex.build_parser()
        args = parser.parse_args(
            [
                "requirements-from-prose",
                "--title",
                "Task",
                "--text",
                "Task statement.",
                "--include-composable",
                "review-evidence,no-legacy",
                "--include-composable",
                "review-evidence",
            ]
        )

        self.assertEqual(robdex._selected_composables(args), ["review-evidence", "no-legacy"])

    def test_requirements_from_prose_help_documents_formatting(self) -> None:
        parser, _handoff_parser, _spawn_parser = robdex.build_parser()
        output = io.StringIO()
        with contextlib.redirect_stdout(output), self.assertRaises(SystemExit):
            parser.parse_args(["requirements-from-prose", "--help"])

        help_text = output.getvalue()
        self.assertIn("The parser turns each non-empty bullet", help_text)
        self.assertIn("Put one complete requirement per line", help_text)
        self.assertIn("headings become accidental requirements", help_text)
        self.assertIn("paragraph blobs", help_text)
        self.assertIn("Preview example:", help_text)
        self.assertIn("No-legacy cleanup example:", help_text)
        self.assertIn("--include-composable no-legacy", help_text)
        self.assertIn("Never run --text-stdin without a heredoc", help_text)

    def test_requirements_from_prose_merges_composables_for_preview(self) -> None:
        args = argparse.Namespace(
            title="Task",
            text="Task statement.",
            text_file=None,
            text_stdin=False,
            include_composable=["review-evidence"],
            attach=False,
            interrupt=False,
            to_self=False,
            to_thread_id="worker-1",
            name=None,
            project_path=None,
        )
        output = io.StringIO()
        with patch.object(robdex, "_request_json", return_value=COMPOSABLES_RESPONSE), contextlib.redirect_stdout(output):
            robdex._cmd_requirements_from_prose("orch-1", args, argparse.ArgumentParser())

        payload = json.loads(output.getvalue())
        self.assertEqual(payload["includeComposables"], ["review-evidence"])
        self.assertEqual(
            [item["key"] for item in payload["requirements"]],
            ["reviewableArtifacts", "taskStatement"],
        )

    def test_requirements_from_prose_attach_sends_composed_requirements(self) -> None:
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

        args = argparse.Namespace(
            title="Task",
            text="Task statement.",
            text_file=None,
            text_stdin=False,
            include_composable=["review-evidence"],
            attach=True,
            interrupt=False,
            to_self=False,
            to_thread_id="worker-1",
            name=None,
            project_path=None,
        )
        with patch.object(robdex, "_request_json", side_effect=fake_request), contextlib.redirect_stdout(io.StringIO()):
            robdex._cmd_requirements_from_prose("orch-1", args, argparse.ArgumentParser())

        self.assertEqual(
            [(method, path) for method, path, _body in calls],
            [
                ("POST", "/orchestrator/requirements/composables"),
                ("POST", "/orchestrator/requirements/set"),
            ],
        )
        set_body = calls[1][2]
        self.assertEqual(set_body["senderThreadId"], "orch-1")
        self.assertEqual(set_body["recipientThreadId"], "worker-1")
        self.assertEqual(set_body["requirementSet"]["includeComposables"], ["review-evidence"])
        self.assertEqual(
            [item["key"] for item in set_body["requirementSet"]["requirements"]],
            ["reviewableArtifacts", "taskStatement"],
        )

    def test_requirements_from_prose_interrupts_sets_and_notifies_target_in_order(self) -> None:
        calls: list[tuple[str, str, dict | None]] = []

        def fake_request(method: str, path: str, *, query=None, body=None):
            calls.append((method, path, body))
            if path == "/orchestrator/requirements/composables":
                return COMPOSABLES_RESPONSE
            if path == "/threads/worker-1/interrupt":
                return {"ok": True}
            if path == "/orchestrator/requirements/set":
                return {
                    "displayName": "Worker",
                    "threadId": "worker-1",
                    "requirementCount": len(body["requirementSet"]["requirements"]),
                    "enforceOnTurns": True,
                }
            if path == "/orchestrator/agent-message":
                return {"recipientThreadId": body["recipientThreadId"], "recipientDisplayName": "Worker"}
            raise AssertionError(path)

        args = argparse.Namespace(
            title="Task",
            text="Task statement.",
            text_file=None,
            text_stdin=False,
            include_composable=["review-evidence"],
            attach=False,
            interrupt=True,
            to_self=False,
            to_thread_id="worker-1",
            name=None,
            project_path=None,
        )
        with patch.object(robdex, "_request_json", side_effect=fake_request), contextlib.redirect_stdout(io.StringIO()):
            robdex._cmd_requirements_from_prose("orch-1", args, argparse.ArgumentParser())

        self.assertEqual(
            [(method, path) for method, path, _body in calls],
            [
                ("POST", "/orchestrator/requirements/composables"),
                ("POST", "/threads/worker-1/interrupt"),
                ("POST", "/orchestrator/requirements/set"),
                ("POST", "/orchestrator/agent-message"),
            ],
        )
        set_body = calls[2][2] or {}
        self.assertEqual(set_body["recipientThreadId"], "worker-1")
        self.assertEqual(set_body["requirementSet"]["includeComposables"], ["review-evidence"])
        self.assertEqual((calls[3][2] or {})["text"], "Requirements updated")

    def test_requirements_from_prose_to_self_sets_delays_interrupts_and_begins(self) -> None:
        calls: list[tuple[str, str, dict | None]] = []

        def fake_request(method: str, path: str, *, query=None, body=None):
            calls.append((method, path, body))
            if path == "/orchestrator/requirements/set":
                return {
                    "displayName": "Orchestrator",
                    "threadId": "orch-1",
                    "requirementCount": len(body["requirementSet"]["requirements"]),
                    "enforceOnTurns": True,
                }
            if path == "/threads/orch-1/interrupt":
                return {"ok": True}
            if path == "/orchestrator/agent-message":
                return {"recipientThreadId": body["recipientThreadId"], "recipientDisplayName": "Orchestrator"}
            raise AssertionError(path)

        args = argparse.Namespace(
            title="Task",
            text="Task statement.",
            text_file=None,
            text_stdin=False,
            include_composable=[],
            attach=False,
            interrupt=False,
            to_self=True,
            to_thread_id=None,
            name=None,
            project_path=None,
        )
        with patch.object(robdex, "_request_json", side_effect=fake_request), patch.object(robdex.time, "sleep") as sleep, contextlib.redirect_stdout(io.StringIO()):
            robdex._cmd_requirements_from_prose("orch-1", args, argparse.ArgumentParser())

        sleep.assert_called_once_with(0.25)
        self.assertEqual(
            [(method, path) for method, path, _body in calls],
            [
                ("POST", "/orchestrator/requirements/set"),
                ("POST", "/threads/orch-1/interrupt"),
                ("POST", "/orchestrator/agent-message"),
            ],
        )
        set_body = calls[0][2] or {}
        self.assertEqual(set_body["recipientThreadId"], "orch-1")
        self.assertEqual((calls[2][2] or {})["text"], "Begin")

    def test_requirements_from_prose_attach_interrupt_and_to_self_are_mutually_exclusive(self) -> None:
        parser, _handoff_parser, _spawn_parser = robdex.build_parser()
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            parser.parse_args(
                [
                    "requirements-from-prose",
                    "--title",
                    "Task",
                    "--text",
                    "Task statement.",
                    "--attach",
                    "--interrupt",
                ]
            )

    def test_set_requirements_requires_target_without_to_self(self) -> None:
        parser, _handoff_parser, _spawn_parser = robdex.build_parser()
        with tempfile.TemporaryDirectory() as tmp:
            args = argparse.Namespace(
                requirements_file=str(self._requirements_file(tmp)),
                include_composable=[],
                to_thread_id=None,
                name=None,
                project_path=None,
                interrupt=False,
                to_self=False,
            )
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                robdex._cmd_set_requirements("orch-1", args, parser)

    def test_set_requirements_targeted_without_interrupt_preserves_composables(self) -> None:
        calls: list[tuple[str, str, dict | None]] = []

        def fake_request(method: str, path: str, *, query=None, body=None):
            calls.append((method, path, body))
            if path == "/orchestrator/requirements/set":
                return {
                    "displayName": "Worker",
                    "threadId": "worker-1",
                    "requirementCount": len(body["requirementSet"]["requirements"]),
                    "enforceOnTurns": True,
                }
            raise AssertionError(path)

        with tempfile.TemporaryDirectory() as tmp:
            args = argparse.Namespace(
                requirements_file=str(self._requirements_file(tmp)),
                include_composable=["review-evidence"],
                to_thread_id=None,
                name="Worker",
                project_path="/tmp/project",
                interrupt=False,
                to_self=False,
            )
            with patch.object(robdex, "_request_json", side_effect=fake_request), contextlib.redirect_stdout(io.StringIO()):
                robdex._cmd_set_requirements("orch-1", args, argparse.ArgumentParser())

        self.assertEqual([(method, path) for method, path, _body in calls], [("POST", "/orchestrator/requirements/set")])
        set_body = calls[0][2] or {}
        self.assertEqual(set_body["senderThreadId"], "orch-1")
        self.assertIsNone(set_body["recipientThreadId"])
        self.assertEqual(set_body["recipientName"], "Worker")
        self.assertEqual(set_body["projectPath"], str(Path("/tmp/project").resolve(strict=False)))
        self.assertEqual(set_body["requirementSet"]["includeComposables"], ["review-evidence"])

    def test_set_requirements_interrupts_sets_and_notifies_target_in_order(self) -> None:
        calls: list[tuple[str, str, dict | None]] = []

        def fake_request(method: str, path: str, *, query=None, body=None):
            calls.append((method, path, body))
            if path == "/threads/worker-1/interrupt":
                return {"ok": True}
            if path == "/orchestrator/requirements/set":
                return {
                    "displayName": "Worker",
                    "threadId": "worker-1",
                    "requirementCount": len(body["requirementSet"]["requirements"]),
                    "enforceOnTurns": True,
                }
            if path == "/orchestrator/agent-message":
                return {"recipientThreadId": body["recipientThreadId"], "recipientDisplayName": "Worker"}
            raise AssertionError(path)

        with tempfile.TemporaryDirectory() as tmp:
            args = argparse.Namespace(
                requirements_file=str(self._requirements_file(tmp)),
                include_composable=[],
                to_thread_id="worker-1",
                name=None,
                project_path=None,
                interrupt=True,
                to_self=False,
            )
            with patch.object(robdex, "_request_json", side_effect=fake_request), contextlib.redirect_stdout(io.StringIO()):
                robdex._cmd_set_requirements("orch-1", args, argparse.ArgumentParser())

        self.assertEqual(
            [(method, path) for method, path, _body in calls],
            [
                ("POST", "/threads/worker-1/interrupt"),
                ("POST", "/orchestrator/requirements/set"),
                ("POST", "/orchestrator/agent-message"),
            ],
        )
        set_body = calls[1][2] or {}
        self.assertEqual(set_body["senderThreadId"], "orch-1")
        self.assertEqual(set_body["recipientThreadId"], "worker-1")
        self.assertIsNone(set_body["recipientName"])
        self.assertEqual((calls[2][2] or {})["text"], "Requirements updated")

    def test_set_requirements_interrupt_rejects_self_target(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            args = argparse.Namespace(
                requirements_file=str(self._requirements_file(tmp)),
                include_composable=[],
                to_thread_id="orch-1",
                name=None,
                project_path=None,
                interrupt=True,
                to_self=False,
            )
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                robdex._cmd_set_requirements("orch-1", args, argparse.ArgumentParser())

    def test_set_requirements_to_self_sets_delays_interrupts_and_begins(self) -> None:
        calls: list[tuple[str, str, dict | None]] = []

        def fake_request(method: str, path: str, *, query=None, body=None):
            calls.append((method, path, body))
            if path == "/orchestrator/requirements/set":
                return {
                    "displayName": "Orchestrator",
                    "threadId": "orch-1",
                    "requirementCount": len(body["requirementSet"]["requirements"]),
                    "enforceOnTurns": True,
                }
            if path == "/threads/orch-1/interrupt":
                return {"ok": True}
            if path == "/orchestrator/agent-message":
                return {"recipientThreadId": body["recipientThreadId"], "recipientDisplayName": "Orchestrator"}
            raise AssertionError(path)

        with tempfile.TemporaryDirectory() as tmp:
            args = argparse.Namespace(
                requirements_file=str(self._requirements_file(tmp)),
                include_composable=[],
                to_thread_id=None,
                name=None,
                project_path=None,
                interrupt=False,
                to_self=True,
            )
            with patch.object(robdex, "_request_json", side_effect=fake_request), patch.object(robdex.time, "sleep") as sleep, contextlib.redirect_stdout(io.StringIO()):
                robdex._cmd_set_requirements("orch-1", args, argparse.ArgumentParser())

        sleep.assert_called_once_with(0.25)
        self.assertEqual(
            [(method, path) for method, path, _body in calls],
            [
                ("POST", "/orchestrator/requirements/set"),
                ("POST", "/threads/orch-1/interrupt"),
                ("POST", "/orchestrator/agent-message"),
            ],
        )
        set_body = calls[0][2] or {}
        self.assertEqual(set_body["recipientThreadId"], "orch-1")
        self.assertIsNone(set_body["recipientName"])
        self.assertEqual((calls[2][2] or {})["text"], "Begin")

    def test_set_requirements_interrupt_and_to_self_are_mutually_exclusive(self) -> None:
        parser, _handoff_parser, _spawn_parser = robdex.build_parser()
        with tempfile.TemporaryDirectory() as tmp, contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            parser.parse_args(
                [
                    "set-requirements",
                    "--requirements-file",
                    str(self._requirements_file(tmp)),
                    "--interrupt",
                    "--to-self",
                ]
            )


if __name__ == "__main__":
    unittest.main()
