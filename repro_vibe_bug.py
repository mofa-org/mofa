import os
import sys
from unittest.mock import patch, MagicMock
from dotenv import load_dotenv, dotenv_values

# Add the mofa directory to sys.path so we can import from it
sys.path.append(os.path.abspath('.'))

from mofa.commands.vibe import _check_and_setup_api_key, _get_env_file_path

def test_repro():
    env_file = _get_env_file_path()
    print(f"Testing with .env file: {env_file}")
    
    # 1. Setup .env with duplicates
    with open(env_file, 'w') as f:
        f.write("OPENAI_API_KEY=old_key_1\n")
        f.write("OTHER_VAR=value\n")
        f.write("OPENAI_API_KEY=old_key_2\n")
    
    # 2. Mock click to simulate user setting a new key
    # We want to:
    # - return None for initial os.getenv('OPENAI_API_KEY') check (done by clearing environ)
    # - click.confirm("Do you want to set it now?") -> True
    # - click.prompt("Enter your OpenAI API key") -> "new_key"
    # - click.confirm("Save to .env file?") -> True
    
    if 'OPENAI_API_KEY' in os.environ:
        del os.environ['OPENAI_API_KEY']
        
    with patch('click.confirm', side_effect=[True, True]), \
         patch('click.prompt', return_value="new_key"), \
         patch('click.echo'):
        
        print("Running _check_and_setup_api_key()...")
        _check_and_setup_api_key()
    
    # 3. Check .env content
    with open(env_file, 'r') as f:
        content = f.read()
    print("\n.env content after update:")
    print(content)
    
    # 4. Verify the bug
    lines = content.splitlines()
    if lines[0] == "OPENAI_API_KEY=new_key" and lines[2] == "OPENAI_API_KEY=old_key_2":
        print("\n[CONFIRMED] Only the first occurrence was updated!")
    else:
        print("\n[FAILED] Behavior different than expected.")
        return

    # 5. Verify that load_dotenv loads the WRONG key
    load_dotenv(env_file, override=True)
    loaded_key = os.getenv('OPENAI_API_KEY')
    print(f"\nLoaded API key via load_dotenv: {loaded_key}")
    
    if loaded_key == "old_key_2":
        print("\n[BUG REPRODUCED] The update was lost because the second occurrence (old) took precedence!")
    else:
        print(f"\n[INFO] Loaded key is {loaded_key}. Behavior might depend on dotenv version.")

if __name__ == "__main__":
    test_repro()
