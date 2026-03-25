from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from command_parser_mcp import server


class _StaticTemporaryDirectory:
    def __init__(self, path: Path) -> None:
        self._path = path

    def __enter__(self) -> str:
        return str(self._path)

    def __exit__(self, exc_type, exc, tb) -> bool:
        return False


class ParseOutputWithCodexTests(unittest.TestCase):
    def test_parse_uses_staged_codex_home_and_no_agents_md(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            temp_root = Path(tmp_dir)
            source_codex_home = temp_root / "source-codex-home"
            source_role = source_codex_home / "roles" / "command-parser.md"
            source_config = source_codex_home / "config.toml"
            source_role.parent.mkdir(parents=True, exist_ok=True)
            source_role.write_text("role contract\n", encoding="utf-8")
            source_config.write_text(
                """
[profiles.command-parser]
model = "gpt-5.1-codex-mini"
model_base_instructions = "/codex-home/roles/command-parser.md"
""".strip()
                + "\n",
                encoding="utf-8",
            )

            run_call: dict[str, object] = {}
            parse_temp_dir = temp_root / "parse-runtime"
            parse_temp_dir.mkdir()

            def fake_run(cmd: list[str], *, text: bool, capture_output: bool, env: dict[str, str]):
                self.assertTrue(text)
                self.assertTrue(capture_output)
                response_path = Path(cmd[cmd.index("-o") + 1])
                response_path.write_text("No errors!\n", encoding="utf-8")
                run_call["cmd"] = cmd
                run_call["env"] = env
                return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")

            with (
                patch.object(server, "CODEX_CONFIG_FILE", source_config),
                patch.object(server.tempfile, "TemporaryDirectory", return_value=_StaticTemporaryDirectory(parse_temp_dir)),
                patch.object(server.subprocess, "run", side_effect=fake_run),
            ):
                result = server._parse_output_with_codex(
                    raw_command=["pytest", "-q"],
                    outcome=server.ExecutionOutcome(
                        argv=["pytest", "-q"],
                        cwd=str(temp_root),
                        exit_code=1,
                        stdout="failure\n",
                        stderr="",
                    ),
                    include_warnings=False,
                    additional_request=None,
                    profile="command-parser",
                )

            self.assertEqual(result, "No errors!")
            self.assertFalse((parse_temp_dir / "AGENTS.md").exists())
            self.assertEqual(run_call["env"]["CODEX_HOME"], str(parse_temp_dir / "codex-home"))
            self.assertEqual(run_call["env"]["HOME"], str(parse_temp_dir / "home"))
            self.assertEqual(
                (parse_temp_dir / "codex-home" / "roles" / "command-parser.md").read_text(encoding="utf-8"),
                "role contract\n",
            )
            self.assertEqual(
                (parse_temp_dir / "codex-home" / "config.toml").read_text(encoding="utf-8"),
                source_config.read_text(encoding="utf-8"),
            )
            self.assertNotIn("AGENTS.md", run_call["cmd"][-1])


if __name__ == "__main__":
    unittest.main()
