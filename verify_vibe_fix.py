import os
import sys
import unittest
from unittest.mock import patch

# Setup path
project_root = r"c:\Users\Asus\OneDrive\Desktop\Mofa-org\mofa"
sys.path.insert(0, project_root)

# Import the module
import mofa
from mofa.commands.vibe import _check_and_setup_api_key, _get_env_file_path

class TestVibeFix(unittest.TestCase):
    def setUp(self):
        self.env_path = _get_env_file_path()
        if os.path.exists(self.env_path):
            os.remove(self.env_path)
        if 'OPENAI_API_KEY' in os.environ:
            del os.environ['OPENAI_API_KEY']

    def tearDown(self):
        if os.path.exists(self.env_path):
            os.remove(self.env_path)

    def test_api_key_loading_fix(self):
        # 1. Create a .env file with the key
        with open(self.env_path, 'w') as f:
            f.write("OPENAI_API_KEY=sk-test-exists\n")
        
        # 2. Call _check_and_setup_api_key
        # It should load the key and NOT prompt
        with patch('click.confirm') as mock_confirm:
            result = _check_and_setup_api_key()
            
            self.assertEqual(result, "sk-test-exists")
            mock_confirm.assert_not_called()
            print("✓ Success: Key loaded from .env without prompting")

    def test_api_key_update_fix(self):
        # 1. Start with an existing key in .env
        with open(self.env_path, 'w') as f:
            f.write("OPENAI_API_KEY=old-key\n")
            
        # 2. Simulate user wanting to set a NEW key
        # We need to bypass the "exists" check by clearing the env var after load_dotenv internal call
        # Or just test the update logic inside _check_and_setup_api_key
        
        with patch('click.confirm', side_effect=[True, True]), \
             patch('click.prompt', return_value="new-key"):
            # Force it to prompt by having NO env var
            _check_and_setup_api_key()
            
        # 3. Check .env content. Should have ONLY the new key.
        with open(self.env_path, 'r') as f:
            content = f.read()
            
        self.assertIn("OPENAI_API_KEY=new-key", content)
        self.assertNotIn("OPENAI_API_KEY=old-key", content)
        print("✓ Success: Key updated in .env instead of appended")

if __name__ == "__main__":
    unittest.main()
