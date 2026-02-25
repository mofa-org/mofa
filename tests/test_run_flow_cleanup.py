import unittest
import sys
import types
from unittest.mock import patch

# run_flow imports read.py, which imports pandas at module import time.
# Stub pandas so this unit test can run in minimal environments.
if "pandas" not in sys.modules:
    fake_pandas = types.ModuleType("pandas")
    fake_pandas.ExcelFile = object
    fake_pandas.read_excel = lambda *args, **kwargs: None
    sys.modules["pandas"] = fake_pandas
if "toml" not in sys.modules:
    fake_toml = types.ModuleType("toml")
    fake_toml.dump = lambda *args, **kwargs: None
    sys.modules["toml"] = fake_toml

from mofa.commands import run_flow as run_flow_module


class RunFlowCleanupTests(unittest.TestCase):
    def test_cleanup_uses_destroy_daemon_only(self):
        with patch("mofa.commands.run_flow.destroy_dora_daemon") as destroy_mock, \
             patch("mofa.commands.run_flow.time.sleep") as sleep_mock:
            run_flow_module.cleanup_existing_dora_processes()

        destroy_mock.assert_called_once()
        sleep_mock.assert_called_once_with(1)


if __name__ == "__main__":
    unittest.main()
