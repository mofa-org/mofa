import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import mofa.commands.vibe as vibe_module


class VibeApiKeyPathTests(unittest.TestCase):
    def test_api_key_save_uses_project_env_path(self):
        with tempfile.TemporaryDirectory() as td:
            env_path = Path(td) / ".env"

            with patch.dict("os.environ", {}, clear=True), \
                 patch("mofa.commands.vibe._get_env_file_path", return_value=str(env_path)), \
                 patch("mofa.commands.vibe.click.confirm", side_effect=[True, True]), \
                 patch("mofa.commands.vibe.click.prompt", return_value="sk-test-123"):
                api_key = vibe_module._check_and_setup_api_key()

            self.assertEqual(api_key, "sk-test-123")
            self.assertTrue(env_path.exists())
            content = env_path.read_text()
            self.assertIn("OPENAI_API_KEY=sk-test-123", content)


if __name__ == "__main__":
    unittest.main()
