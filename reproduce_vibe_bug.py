import os
import sys
from unittest.mock import patch
import click
from click.testing import CliRunner

# Add the project root to sys.path to import mofa
project_root = r"c:\Users\Asus\OneDrive\Desktop\Mofa-org\mofa"
sys.path.insert(0, project_root)

from mofa.commands.vibe import register_vibe_commands

def test_vibe_tui_prompts_even_with_env():
    @click.group()
    def cli():
        pass
    
    register_vibe_commands(cli)
    runner = CliRunner()
    
    # Create a dummy .env file in the expected project root
    env_path = os.path.join(project_root, ".env")
    old_env_content = ""
    if os.path.exists(env_path):
        with open(env_path, 'r') as f:
            old_env_content = f.read()
            
    try:
        with open(env_path, 'w') as f:
            f.write("OPENAI_API_KEY=existing_key_in_env\n")
        
        # We need to make sure OPENAI_API_KEY is NOT in the actual environment
        if 'OPENAI_API_KEY' in os.environ:
            del os.environ['OPENAI_API_KEY']
            
        print("--- Running 'mofa vibe' (TUI) ---")
        # In the bug, it should prompt for the key because it hasn't loaded .env yet.
        # We'll provide 'new_key' as input and 'n' to not save it (to avoid messing up .env further)
        # or just 'q' to quit after the prompt.
        result = runner.invoke(cli, ["vibe"], input="new_key\nn\nq\n")
        
        print(result.output)
        
        if "OPENAI_API_KEY not found in environment" in result.output:
            print("\n[CONFIRMED] Bug reproduced: User was prompted for API key despite it being in .env")
        else:
            print("\n[FAILED] Bug not reproduced: User was not prompted (or prompted for something else)")

    finally:
        # Restore old .env
        if old_env_content:
            with open(env_path, 'w') as f:
                f.write(old_env_content)
        elif os.path.exists(env_path):
            os.remove(env_path)

if __name__ == "__main__":
    test_vibe_tui_prompts_even_with_env()
