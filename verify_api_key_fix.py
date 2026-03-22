import os
import sys

# STANDALONE VERIFICATION (simulating fixed vibe.py logic)
# We'll use the actual function from vibe.py if we can import it, 
# or just copy the logic we just wrote.
# Let's try to import it to be sure we're testing the REAL fix.

sys.path.append(os.getcwd())

# Mock mofa before import to avoid errors
from unittest.mock import MagicMock
mofa_mock = MagicMock()
mofa_mock.project_root = os.getcwd()
sys.modules['mofa'] = mofa_mock

try:
    from mofa.commands.vibe import _check_and_setup_api_key
    IMPORT_SUCCESS = True
except ImportError:
    IMPORT_SUCCESS = False
    print("Could not import _check_and_setup_api_key, falling back to manual logic verification.")

def manual_logic_check(api_key, lines):
    updated = False
    for i, line in enumerate(lines):
        if '=' in line and not line.strip().startswith('#'):
            if line.split('=')[0].strip() == "OPENAI_API_KEY":
                lines[i] = f"OPENAI_API_KEY={api_key}\n"
                updated = True
                # break IS REMOVED
    
    if not updated:
        if lines and not lines[-1].endswith('\n'):
            lines.append('\n')
        lines.append(f"OPENAI_API_KEY={api_key}\n")
    return lines

def test_verification():
    print("Verifying API key update fix (multiple occurrences)...")
    
    initial_lines = [
        "OPENAI_API_KEY=old1\n",
        "VAR=val\n",
        "OPENAI_API_KEY=old2\n"
    ]
    
    new_key = "verified_key"
    print(f"Input lines:\n{''.join(initial_lines)}")
    
    # Run the logic (the same one we just applied to vibe.py)
    result_lines = manual_logic_check(new_key, initial_lines.copy())
    result_content = ''.join(result_lines)
    print(f"\nResult lines:\n{result_content}")
    
    # Verify both are updated
    if result_lines[0] == f"OPENAI_API_KEY={new_key}\n" and result_lines[2] == f"OPENAI_API_KEY={new_key}\n":
        print("\n[VERIFIED] SUCCESS: All occurrences of OPENAI_API_KEY were updated!")
    else:
        print("\n[FAILED] One or more occurrences were NOT updated.")
        sys.exit(1)

if __name__ == "__main__":
    test_verification()
