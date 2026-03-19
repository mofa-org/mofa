import os
import sys

# Standalone reproduction of the logic in mofa/mofa/commands/vibe.py
def mock_check_and_setup_api_key(api_key, lines):
    updated = False
    for i, line in enumerate(lines):
        if '=' in line and not line.strip().startswith('#'):
            if line.split('=')[0].strip() == "OPENAI_API_KEY":
                lines[i] = f"OPENAI_API_KEY={api_key}\n"
                updated = True
                break # <--- THE BUG IS HERE
    
    if not updated:
        if lines and not lines[-1].endswith('\n'):
            lines.append('\n')
        lines.append(f"OPENAI_API_KEY={api_key}\n")
    return lines

def test_repro():
    print("Testing _check_and_setup_api_key duplicate handling...")
    
    # 1. Input lines with duplicates
    initial_lines = [
        "OPENAI_API_KEY=old_key_1\n",
        "OTHER_VAR=value\n",
        "OPENAI_API_KEY=old_key_2\n"
    ]
    
    new_key = "new_key"
    print(f"Initial lines:\n{''.join(initial_lines)}")
    print(f"Updating to: {new_key}")
    
    # 2. Run the mock logic
    result_lines = mock_check_and_setup_api_key(new_key, initial_lines.copy())
    result_content = ''.join(result_lines)
    print(f"\nResult lines:\n{result_content}")
    
    # 3. Verify the bug
    # If the bug exists, the first line is "OPENAI_API_KEY=new_key" and the third is "OPENAI_API_KEY=old_key_2"
    if result_lines[0] == "OPENAI_API_KEY=new_key\n" and result_lines[2] == "OPENAI_API_KEY=old_key_2\n":
        print("\n[CONFIRMED] BUG: Only the first occurrence was updated!")
        print("Since .env loaders take the LAST value, this update is EFFECTIVELY IGNORED.")
    else:
        print("\n[FAILED] Behavior different than expected.")

if __name__ == "__main__":
    test_repro()
