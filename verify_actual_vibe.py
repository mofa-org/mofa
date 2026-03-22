import os
import sys

# Add mofa to path so imports work
project_root = os.path.abspath(os.path.join(os.path.dirname(__file__)))
sys.path.append(project_root)

# Mock click to provide automated inputs
from unittest.mock import patch
import mofa.commands.vibe as vibe

def test_actual_vibe_fix():
    print("Verifying actual _check_and_setup_api_key behavior with duplicate keys...")
    
    # 1. Create a dummy .env file
    env_path = os.path.join(project_root, ".env")
    
    # Backup existing .env if any
    backup_content = None
    if os.path.exists(env_path):
        with open(env_path, 'r') as f:
            backup_content = f.read()

    try:
        # Write test data with duplicates
        test_content = "OPENAI_API_KEY=old_key_1\nOTHER_VAR=value\nOPENAI_API_KEY=old_key_2\n"
        with open(env_path, 'w') as f:
            f.write(test_content)
            
        print(f"\nInitial .env content:\n{test_content}")
        
        # 2. Run the function with mocks
        # We need to mock os.getenv to return None initially so it prompts
        # but also mock click.prompt and confirm
        with patch('os.getenv', return_value=None), \
             patch('click.confirm', return_value=True), \
             patch('click.prompt', return_value="new_verified_key"):
             
             # Call the function
             vibe._check_and_setup_api_key()
             
        # 3. Read back the .env file and verify
        with open(env_path, 'r') as f:
            final_content = f.read()
            
        print(f"Final .env content:\n{final_content}")
        
        lines = final_content.splitlines()
        
        # Check if the bug is fixed - BOTH occurrences should be updated
        if "OPENAI_API_KEY=new_verified_key" in lines[0] and "OPENAI_API_KEY=new_verified_key" in lines[2]:
            print("\n[SUCCESS] The bug is FIXED! All occurrences were successfully updated in the actual .env file.")
        else:
            print("\n[FAILED] The occurrences were not updated correctly.")
            sys.exit(1)
            
    finally:
        # Restore backup
        if backup_content is not None:
            with open(env_path, 'w') as f:
                f.write(backup_content)
        else:
            os.remove(env_path)

if __name__ == "__main__":
    test_actual_vibe_fix()
