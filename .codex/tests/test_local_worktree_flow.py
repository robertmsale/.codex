#!/usr/bin/env python3

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_SCRIPT = Path(__file__).resolve().parents[1] / "hooks" / "local_worktree_flow.py"


def run(cmd, cwd=None, input_text=None, env=None, check=True):
    result = subprocess.run(
        cmd,
        cwd=cwd,
        input=input_text,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        check=False,
    )
    if check and result.returncode != 0:
        raise AssertionError(f"{cmd} failed\nstdout={result.stdout}\nstderr={result.stderr}")
    return result


def git(root, *args, **kwargs):
    return run(["git", "-C", str(root), *args], **kwargs)


def init_repo(root: Path):
    git(root, "init", "-b", "main")
    git(root, "config", "user.email", "test@example.invalid")
    git(root, "config", "user.name", "Test User")
    (root / "README.md").write_text("root\n")
    (root / ".gitignore").write_text(".worktrees/\nfake-bin/\n")
    (root / ".worktrees").mkdir()
    git(root, "add", "README.md", ".gitignore")
    git(root, "commit", "-m", "initial")


def payload(root: Path, name="Worker One", role="worker"):
    slug = f"{role}-{name.lower().replace(' ', '-')}"
    return {
        "projectRoot": str(root),
        "agent": {"name": name, "role": role},
        "defaults": {
            "branchName": f"codex/{slug}",
            "worktreePath": str(root / ".worktrees" / slug),
        },
    }


def real_git_path():
    if Path("/usr/bin/git").exists():
        return "/usr/bin/git"
    result = run(["/usr/bin/which", "git"])
    return result.stdout.strip()


class LocalWorktreeFlowTests(unittest.TestCase):
    def test_create_hook_creates_idempotent_worktree_and_artifacts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            init_repo(root)
            body = json.dumps(payload(root))
            first = run([str(REPO_SCRIPT), "create", "worker"], cwd=root, input_text=body)
            first_json = json.loads(first.stdout)
            self.assertTrue(first_json["ok"])
            self.assertEqual(first_json["artifacts"]["branchName"], "codex/worker-worker-one")
            self.assertEqual(first_json["artifacts"]["projectRoot"], str(root.resolve()))
            worktree = Path(first_json["artifacts"]["worktreePath"])
            self.assertTrue(worktree.is_dir())
            self.assertEqual(git(worktree, "branch", "--show-current").stdout.strip(), "codex/worker-worker-one")

            second = run([str(REPO_SCRIPT), "create", "worker"], cwd=root, input_text=body)
            self.assertEqual(json.loads(second.stdout)["artifacts"]["worktreePath"], str(worktree))

    def test_create_hook_refuses_unrelated_directory_and_branch(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            init_repo(root)
            bad = payload(root)
            Path(bad["defaults"]["worktreePath"]).mkdir()
            result = run([str(REPO_SCRIPT), "create", "worker"], cwd=root, input_text=json.dumps(bad), check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("top-level mismatch", result.stdout + result.stderr)

            branch_conflict = payload(root, "Other Worker")
            git(root, "branch", branch_conflict["defaults"]["branchName"])
            result = run([str(REPO_SCRIPT), "create", "worker"], cwd=root, input_text=json.dumps(branch_conflict), check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("already exists", result.stdout)

    def test_archive_cleanup_refuses_path_escape_dirty_and_mismatch_then_is_idempotent(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            init_repo(root)
            create = json.loads(run([str(REPO_SCRIPT), "create", "worker"], cwd=root, input_text=json.dumps(payload(root))).stdout)
            lifecycle = {
                "branchName": create["artifacts"]["branchName"],
                "worktreePath": create["artifacts"]["worktreePath"],
            }
            archive_payload = {"projectRoot": str(root), "lifecycle": lifecycle, "agent": {"role": "worker"}}
            Path(lifecycle["worktreePath"], "dirty.txt").write_text("dirty")
            dirty = run([str(REPO_SCRIPT), "archive", "worker"], cwd=root, input_text=json.dumps(archive_payload), check=False)
            self.assertNotEqual(dirty.returncode, 0)
            self.assertIn("dirty worktree", dirty.stdout)
            Path(lifecycle["worktreePath"], "dirty.txt").unlink()

            mismatch = dict(archive_payload)
            mismatch["lifecycle"] = dict(lifecycle, branchName="codex/other")
            branch_mismatch = run([str(REPO_SCRIPT), "archive", "worker"], cwd=root, input_text=json.dumps(mismatch), check=False)
            self.assertNotEqual(branch_mismatch.returncode, 0)
            self.assertIn("branch mismatch", branch_mismatch.stdout)

            escape = dict(archive_payload)
            escape["lifecycle"] = dict(lifecycle, worktreePath=str(root.parent))
            path_escape = run([str(REPO_SCRIPT), "archive", "worker"], cwd=root, input_text=json.dumps(escape), check=False)
            self.assertNotEqual(path_escape.returncode, 0)
            self.assertIn("escapes", path_escape.stdout)

            missing_metadata = run([str(REPO_SCRIPT), "archive", "worker"], cwd=root, input_text=json.dumps({"projectRoot": str(root), "lifecycle": {}}), check=False)
            self.assertNotEqual(missing_metadata.returncode, 0)
            self.assertIn("missing lifecycle", missing_metadata.stdout)

            removed = json.loads(run([str(REPO_SCRIPT), "archive", "worker"], cwd=root, input_text=json.dumps(archive_payload)).stdout)
            self.assertEqual(removed["artifacts"]["cleanupState"], "removed")
            again = json.loads(run([str(REPO_SCRIPT), "archive", "worker"], cwd=root, input_text=json.dumps(archive_payload)).stdout)
            self.assertEqual(again["artifacts"]["cleanupState"], "alreadyClean")

    def test_archive_cleanup_refuses_repository_mismatch(self):
        with tempfile.TemporaryDirectory() as tmp, tempfile.TemporaryDirectory() as other_tmp:
            root = Path(tmp).resolve()
            other = Path(other_tmp).resolve()
            init_repo(root)
            init_repo(other)
            outside = json.loads(run([str(REPO_SCRIPT), "create", "worker"], cwd=other, input_text=json.dumps(payload(other))).stdout)
            expected_path = root / ".worktrees" / "worker-worker-one"
            expected_path.parent.mkdir(exist_ok=True)
            run(["cp", "-R", outside["artifacts"]["worktreePath"], str(expected_path)])
            archive_payload = {
                "projectRoot": str(root),
                "lifecycle": {
                    "branchName": outside["artifacts"]["branchName"],
                    "worktreePath": str(expected_path),
                },
            }
            result = run([str(REPO_SCRIPT), "archive", "worker"], cwd=root, input_text=json.dumps(archive_payload), check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("different repository", result.stdout)

    def test_local_integration_rebases_fast_forwards_main_and_cleans_up(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            init_repo(root)
            created = json.loads(run([str(REPO_SCRIPT), "create", "worker"], cwd=root, input_text=json.dumps(payload(root))).stdout)
            worktree = Path(created["artifacts"]["worktreePath"])
            (worktree / "feature.txt").write_text("feature\n")
            git(worktree, "add", "feature.txt")
            git(worktree, "commit", "-m", "feature")
            fake_bin = root / "fake-bin"
            fake_bin.mkdir()
            robdex = fake_bin / "robdex"
            robdex.write_text("#!/bin/sh\nprintf '%s\\n' status=passed\n")
            robdex.chmod(0o755)
            env = dict(os.environ, PATH=f"{fake_bin}:{os.environ['PATH']}")
            result = run(
                [str(REPO_SCRIPT), "integrate", str(worktree), "--thread-id", "thread-worker", "--root", str(root)],
                cwd=root,
                env=env,
            )
            self.assertIn("Integrated codex/worker-worker-one into main", result.stdout)
            self.assertFalse(worktree.exists())
            self.assertFalse(git(root, "show-ref", "--verify", "--quiet", "refs/heads/codex/worker-worker-one", check=False).returncode == 0)
            self.assertTrue((root / "feature.txt").exists())

    def test_local_integration_refuses_unclear_requirements_dirty_missing_and_no_ahead(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            init_repo(root)
            created = json.loads(run([str(REPO_SCRIPT), "create", "worker"], cwd=root, input_text=json.dumps(payload(root))).stdout)
            worktree = Path(created["artifacts"]["worktreePath"])
            fake_bin = root / "fake-bin"
            fake_bin.mkdir()
            robdex = fake_bin / "robdex"
            robdex.write_text("#!/bin/sh\nprintf '%s\\n' status=failed\n")
            robdex.chmod(0o755)
            env = dict(os.environ, PATH=f"{fake_bin}:{os.environ['PATH']}")
            unclear = run([str(REPO_SCRIPT), "integrate", str(worktree), "--thread-id", "thread-worker", "--root", str(root)], cwd=root, env=env, check=False)
            self.assertIn("Requirements Review is not clear", unclear.stderr + unclear.stdout)
            robdex.write_text("#!/bin/sh\nprintf '%s\\n' status=inReview active_requirements=1\n")
            active = run([str(REPO_SCRIPT), "integrate", str(worktree), "--thread-id", "thread-worker", "--root", str(root)], cwd=root, env=env, check=False)
            self.assertIn("Requirements Review is not clear", active.stderr + active.stdout)
            robdex.write_text("#!/bin/sh\nprintf '%s\\n' status=passed\n")
            no_ahead = run([str(REPO_SCRIPT), "integrate", str(worktree), "--thread-id", "thread-worker", "--root", str(root)], cwd=root, env=env, check=False)
            self.assertIn("no commits ahead", no_ahead.stderr + no_ahead.stdout)
            (worktree / "dirty.txt").write_text("dirty")
            dirty = run([str(REPO_SCRIPT), "integrate", str(worktree), "--thread-id", "thread-worker", "--root", str(root)], cwd=root, env=env, check=False)
            self.assertIn("dirty", dirty.stderr + dirty.stdout)

    def test_local_integration_refuses_missing_branch_and_rebase_conflict(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            init_repo(root)
            fake_bin = root / "fake-bin"
            fake_bin.mkdir()
            robdex = fake_bin / "robdex"
            robdex.write_text("#!/bin/sh\nprintf '%s\\n' status=passed\n")
            robdex.chmod(0o755)
            env = dict(os.environ, PATH=f"{fake_bin}:{os.environ['PATH']}")

            created = json.loads(run([str(REPO_SCRIPT), "create", "worker"], cwd=root, input_text=json.dumps(payload(root, "Missing Branch"))).stdout)
            missing_worktree = Path(created["artifacts"]["worktreePath"])
            git(root, "update-ref", "-d", f"refs/heads/{created['artifacts']['branchName']}")
            missing = run([str(REPO_SCRIPT), "integrate", str(missing_worktree), "--thread-id", "thread-worker", "--root", str(root)], cwd=root, env=env, check=False)
            self.assertIn("missing branch", missing.stderr + missing.stdout)

            conflict_payload = payload(root, "Conflict Worker")
            created = json.loads(run([str(REPO_SCRIPT), "create", "worker"], cwd=root, input_text=json.dumps(conflict_payload)).stdout)
            worktree = Path(created["artifacts"]["worktreePath"])
            (worktree / "README.md").write_text("worker change\n")
            git(worktree, "add", "README.md")
            git(worktree, "commit", "-m", "worker conflict")
            (root / "README.md").write_text("main change\n")
            git(root, "add", "README.md")
            git(root, "commit", "-m", "main conflict")
            conflict = run([str(REPO_SCRIPT), "integrate", str(worktree), "--thread-id", "thread-worker", "--root", str(root)], cwd=root, env=env, check=False)
            self.assertNotEqual(conflict.returncode, 0)
            self.assertIn("CONFLICT", conflict.stderr + conflict.stdout)

    def test_local_integration_refuses_main_movement_after_rebase(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            init_repo(root)
            created = json.loads(run([str(REPO_SCRIPT), "create", "worker"], cwd=root, input_text=json.dumps(payload(root, "Main Moved"))).stdout)
            worktree = Path(created["artifacts"]["worktreePath"])
            (worktree / "feature.txt").write_text("feature\n")
            git(worktree, "add", "feature.txt")
            git(worktree, "commit", "-m", "feature")
            fake_bin = root / "fake-bin"
            fake_bin.mkdir()
            robdex = fake_bin / "robdex"
            robdex.write_text("#!/bin/sh\nprintf '%s\\n' status=passed\n")
            robdex.chmod(0o755)
            wrapper = fake_bin / "git"
            wrapper.write_text(
                "#!/bin/sh\n"
                f"REAL_GIT='{real_git_path()}'\n"
                "\"$REAL_GIT\" \"$@\"\n"
                "status=$?\n"
                "if [ \"$status\" -eq 0 ] && [ \"$1\" = '-C' ] && [ \"$3\" = 'rebase' ]; then\n"
                f"  printf '%s\\n' 'main moved during integration' > '{root}/.codex-local-integrate-main-moved'\n"
                f"  \"$REAL_GIT\" -C '{root}' add .codex-local-integrate-main-moved >/dev/null\n"
                f"  \"$REAL_GIT\" -C '{root}' commit -m 'simulate main movement during integration' >/dev/null\n"
                "fi\n"
                "exit \"$status\"\n"
            )
            wrapper.chmod(0o755)
            env = dict(os.environ, PATH=f"{fake_bin}:{os.environ['PATH']}")
            result = run([str(REPO_SCRIPT), "integrate", str(worktree), "--thread-id", "thread-worker", "--root", str(root)], cwd=root, env=env, check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("local main moved during integration", result.stderr + result.stdout)

if __name__ == "__main__":
    unittest.main()
