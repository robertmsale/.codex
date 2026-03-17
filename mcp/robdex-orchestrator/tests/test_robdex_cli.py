from __future__ import annotations

import importlib.util
import io
import os
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch


SCRIPT_PATH = Path(__file__).resolve().parents[3] / "skills" / "robdex-orchestrator" / "scripts" / "robdex.py"
SPEC = importlib.util.spec_from_file_location("robdex_cli_under_test", SCRIPT_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"Unable to load robdex CLI module from {SCRIPT_PATH}")
robdex_cli = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(robdex_cli)


class RobdexCLITests(unittest.TestCase):
    def test_send_message_keeps_inline_text_flag(self) -> None:
        with (
            patch.dict(os.environ, {"CODEX_THREAD_ID": "orch-ezra"}, clear=False),
            patch.object(
                sys,
                "argv",
                [
                    "robdex",
                    "send-message",
                    "--name",
                    "Issue 624 Accounting UI Alignment",
                    "--text",
                    "Status check.",
                ],
            ),
            patch.object(robdex_cli.robdex_server, "robdex_send_message", return_value="Sent") as send_message,
            redirect_stdout(io.StringIO()),
        ):
            exit_code = robdex_cli.main()

        self.assertEqual(exit_code, 0)
        send_message.assert_called_once()
        self.assertEqual(send_message.call_args.kwargs["text"], "Status check.")

    def test_send_message_reads_text_from_stdin(self) -> None:
        with (
            patch.dict(os.environ, {"CODEX_THREAD_ID": "orch-ezra"}, clear=False),
            patch.object(
                sys,
                "argv",
                ["robdex", "send-message", "--name", "Issue 624 Accounting UI Alignment", "--text-stdin"],
            ),
            patch("sys.stdin", io.StringIO("Use `flutter build macos` instead.\n")),
            patch.object(robdex_cli.robdex_server, "robdex_send_message", return_value="Sent") as send_message,
            redirect_stdout(io.StringIO()),
        ):
            exit_code = robdex_cli.main()

        self.assertEqual(exit_code, 0)
        send_message.assert_called_once()
        self.assertEqual(send_message.call_args.kwargs["text"], "Use `flutter build macos` instead.\n")

    def test_send_message_reads_text_from_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            message_path = Path(tmp) / "message.txt"
            message_path.write_text("Build with `flutter build macos`.\n", encoding="utf-8")

            with (
                patch.dict(os.environ, {"CODEX_THREAD_ID": "orch-ezra"}, clear=False),
                patch.object(
                    sys,
                    "argv",
                    [
                        "robdex",
                        "send-message",
                        "--name",
                        "Issue 624 Accounting UI Alignment",
                        "--text-file",
                        str(message_path),
                    ],
                ),
                patch.object(robdex_cli.robdex_server, "robdex_send_message", return_value="Sent") as send_message,
                redirect_stdout(io.StringIO()),
            ):
                exit_code = robdex_cli.main()

        self.assertEqual(exit_code, 0)
        send_message.assert_called_once()
        self.assertEqual(send_message.call_args.kwargs["text"], "Build with `flutter build macos`.\n")


if __name__ == "__main__":
    unittest.main()
