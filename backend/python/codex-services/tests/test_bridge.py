from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from codex_services_http.bridge import BridgePaths
from codex_services_http.bridge import require_allowed_path
from codex_services_http.bridge import sanitize_for_response


class BridgeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.TemporaryDirectory()
        root = Path(self.tmpdir.name)
        self.host_home = root / "Users" / "robertsale"
        self.host_home.mkdir(parents=True)
        repo = self.host_home / "Code" / "robdex"
        repo.mkdir(parents=True)
        self.paths = BridgePaths(
            host_home=self.host_home,
            virtual_home=Path("/home/robertsale"),
            allowed_roots=(repo,),
        )

    def tearDown(self) -> None:
        self.tmpdir.cleanup()

    def test_virtual_path_maps_to_host(self) -> None:
        resolved = require_allowed_path("/home/robertsale/Code/robdex", self.paths)
        self.assertEqual(resolved.resolve(strict=False), (self.host_home / "Code" / "robdex").resolve(strict=False))

    def test_outside_allowed_root_is_rejected(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "outside allowed synced roots"):
            require_allowed_path("/home/robertsale/Code/other", self.paths)

    def test_response_text_maps_host_home_back_to_virtual(self) -> None:
        text = sanitize_for_response(f"{self.host_home}/Code/robdex", self.paths)
        self.assertEqual(text, "/home/robertsale/Code/robdex")


if __name__ == "__main__":
    unittest.main()
