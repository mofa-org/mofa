import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from mofa.commands import init as init_module


class InitCommandTests(unittest.TestCase):
    def test_download_examples_falls_back_to_main_python(self):
        pull_attempts = []

        def fake_run(cmd, cwd=None, **kwargs):
            if cmd[:3] == ["git", "pull", "origin"]:
                branch = cmd[3]
                pull_attempts.append(branch)
                if branch == "main":
                    return subprocess.CompletedProcess(cmd, 1, stdout="", stderr="missing path")
                Path(cwd, "agents").mkdir(parents=True, exist_ok=True)
                Path(cwd, "flows").mkdir(parents=True, exist_ok=True)
                return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")
            return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")

        with tempfile.TemporaryDirectory() as td, \
             patch("mofa.commands.init.subprocess.run", side_effect=fake_run), \
             patch("mofa.commands.init.copy_examples", return_value=True) as copy_examples:
            ok = init_module.download_examples_from_github(Path(td))

        self.assertTrue(ok)
        self.assertEqual(pull_attempts, ["main", "main-python"])
        copy_examples.assert_called_once()

    def test_download_examples_fails_when_all_branches_fail(self):
        def fake_run(cmd, cwd=None, **kwargs):
            if cmd[:3] == ["git", "pull", "origin"]:
                return subprocess.CompletedProcess(cmd, 1, stdout="", stderr="fail")
            return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")

        with tempfile.TemporaryDirectory() as td, \
             patch("mofa.commands.init.subprocess.run", side_effect=fake_run):
            ok = init_module.download_examples_from_github(Path(td))

        self.assertFalse(ok)


if __name__ == "__main__":
    unittest.main()
