from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from codex_services_http.bridge import _load_allowed_roots


class BridgePathTests(unittest.TestCase):
    def test_load_allowed_roots_accepts_local_beta_paths(self) -> None:
        with TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            alpha = root / "Users" / "robertsale" / "Code" / "ezra" / "ezra"
            beta = root / "Users" / "robertsale" / "Code" / "ezra" / "qa" / "repo"
            alpha.mkdir(parents=True)
            beta.parent.mkdir(parents=True)

            mutagen_file = root / "mutagen.yml"
            mutagen_file.write_text(
                "\n".join(
                    [
                        "sync:",
                        "  ezra-qa-repo:",
                        f"    alpha: {alpha}",
                        f"    beta: {beta}",
                        "    mode: one-way-replica",
                    ]
                ),
                encoding="utf-8",
            )

            roots = _load_allowed_roots(mutagen_file)
            self.assertIn(alpha.resolve(), roots)
            self.assertIn(beta.resolve(), roots)

    def test_load_allowed_roots_ignores_remote_beta_endpoints(self) -> None:
        with TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            alpha = root / "Users" / "robertsale" / "Code" / "ezra" / "ezra"
            alpha.mkdir(parents=True)

            mutagen_file = root / "mutagen.yml"
            mutagen_file.write_text(
                "\n".join(
                    [
                        "sync:",
                        "  ezra:",
                        f"    alpha: {alpha}",
                        "    beta: syncuser@codex-dev.shared:/home/robertsale/Code/ezra/ezra",
                        "    mode: one-way-replica",
                    ]
                ),
                encoding="utf-8",
            )

            roots = _load_allowed_roots(mutagen_file)
            self.assertIn(alpha.resolve(), roots)
            self.assertEqual(len(roots), 1)


if __name__ == "__main__":
    unittest.main()
