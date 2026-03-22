import os
import sys
import unittest
from unittest.mock import patch, MagicMock

# Setup path
project_root = r"c:\Users\Asus\OneDrive\Desktop\Mofa-org\mofa"
sys.path.insert(0, project_root)

# Import the module
from mofa.commands.vibe import _check_and_setup_api_key, _get_env_file_path

class TestVibeBug(unittest.TestCase):
    def test_api_key_loading_logic_error(self):
        # 1. Create a dummy .env file in the project root with an API key
        env_path = _get_env_file_path()
        with open(env_path, 'w') as f:
            f.write("OPENAI_API_KEY=sk-test-exists\n")
        
        # 2. Ensure the environment variable is NOT set in the shell
        if 'OPENAI_API_KEY' in os.environ:
            del os.environ['OPENAI_API_KEY']
            
        # 3. Call _check_and_setup_api_key. 
        # EXPECTED: It should load the key from .env and NOT prompt.
        # ACTUAL: It checks os.getenv('OPENAI_API_KEY') BEFORE load_dotenv is called in the main loop.
        # (Note: vibe.py's _run_vibe_tui calls load_dotenv AFTER _check_and_setup_api_key)
        
        with patch('click.confirm', return_value=False) as mock_confirm:
            result = _check_and_setup_api_key()
            
            # If it prompts, it means it didn't find it.
            # Since we said "No" to the prompt (return_value=False), it should return None.
            self.assertIsNone(result, "Bug confirmed: _check_and_setup_api_key prompted for key even though it exists in .env")
            mock_confirm.assert_called()

    def test_api_key_appending_bloat(self):
        # 1. Start with an existing .env
        env_path = _get_env_file_path()
        with open(env_path, 'w') as f:
            f.write("OPENAI_API_KEY=old-key\n")
            
        # 2. Simulate user entering a new key and saving it
        if 'OPENAI_API_KEY' in os.environ:
            del os.environ['OPENAI_API_KEY']
            
        with patch('click.confirm', side_effect=[True, True]), \
             patch('click.prompt', return_value="new-key"):
            _check_and_setup_api_key()
            
        # 3. Check .env content
        with open(env_path, 'r') as f:
            content = f.read()
            
        # EXPECTED: .env should ideally have only the new key or update the old one.
        # ACTUAL: It appends with 'a', so it has BOTH.
        self.assertIn("OPENAI_API_KEY=old-key", content)
        self.assertIn("OPENAI_API_KEY=new-key", content)
        print(f"File content after 'save':\n{content}")

if __name__ == "__main__":
    # Clean up before run
    env_path = _get_env_file_path()
    if os.path.exists(env_path):
        os.remove(env_path)
        
    unittest.main()
