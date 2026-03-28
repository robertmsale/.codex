from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from command_execution_mcp import server


class CommandExecutionWaitTests(unittest.TestCase):
    def test_missing_marker_returns_immediately(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            with patch.object(server, "JOB_DIR", Path(tmpdir)):
                result = server.command_execution_wait(
                    "12345678-1234-1234-1234-1234567890ab"
                )

        self.assertEqual(result, "all done")

    def test_existing_marker_waits_until_removed(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            marker_dir = Path(tmpdir)
            job_id = "12345678-1234-1234-1234-1234567890ab"
            marker = marker_dir / f"{job_id}.job"
            marker.write_text("job_id=test\n", encoding="utf-8")
            sleep_calls = 0

            def fake_sleep(_: float) -> None:
                nonlocal sleep_calls
                sleep_calls += 1
                marker.unlink()

            with patch.object(server, "JOB_DIR", marker_dir):
                with patch.object(server.time, "sleep", side_effect=fake_sleep):
                    result = server.command_execution_wait(job_id)

        self.assertEqual(result, "all done")
        self.assertEqual(sleep_calls, 1)


if __name__ == "__main__":
    unittest.main()
