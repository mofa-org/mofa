import os
import sys
from unittest.mock import patch, MagicMock

# 1. Mock the 'mofa' module and its contents before importing vibe.py
# This avoids the "module 'mofa' has no attribute 'agents_dir_path'" error if it's not set up right.
mofa_mock = MagicMock()
mofa_mock.agents_dir_path = "/default/agents"
mofa_mock.flows_dir_path = "/default/flows"
mofa_mock.project_root = os.getcwd()

# Inject into sys.modules
sys.modules['mofa'] = mofa_mock

# Now we can import vibe
# We also need to mock mofa.vibe.flow_generator so the import doesn't fail
sys.modules['mofa.vibe'] = MagicMock()
sys.modules['mofa.vibe.flow_generator'] = MagicMock()

import mofa.commands.vibe as vibe
import click
from click.testing import CliRunner

def test_repro():
    runner = CliRunner()
    
    @click.group()
    def cli():
        pass
    vibe.register_vibe_commands(cli)
    
    # 2. Mock _load_vibe_config to return a custom agents_output
    saved_config = {
        "model": "gpt-4",
        "max_rounds": 100,
        "agents_output": "/custom/agents", # <--- CUSTOM PATH
        "flows_output": "/custom/flows",
    }
    
    with patch('mofa.commands.vibe._load_vibe_config', return_value=saved_config), \
         patch('mofa.commands.vibe._save_vibe_config'), \
         patch('mofa.commands.vibe._check_and_setup_api_key', return_value="fake-key"), \
         patch('mofa.vibe.flow_generator.FlowGenerator') as MockFlowGenerator:
        
        # Run 'vibe flow'
        # Inputs for prompt:
        # LLM model (saved-model)
        # Output directory (custom-flows)
        # Requirement (description)
        result = runner.invoke(cli, ["vibe", "flow"], input="gpt-4\n/custom/flows\nmy requirement\n")
        
        print(f"Exit code: {result.exit_code}")
        
        # 3. Verify what was passed to FlowGenerator
        if MockFlowGenerator.called:
            args, kwargs = MockFlowGenerator.call_args
            agents_dir = kwargs.get('agents_dir')
            print(f"\nFlowGenerator received agents_dir: {agents_dir}")
            print(f"Expected (from saved config): {saved_config['agents_output']}")
            print(f"Default (from mofa): {mofa_mock.agents_dir_path}")
            
            if agents_dir == mofa_mock.agents_dir_path:
                print("\n[CONFIRMED] BUG: FlowGenerator used the default agents_dir instead of the custom one from saved_config!")
            elif agents_dir == saved_config['agents_output']:
                print("\n[INFO] Behavior is correct. No bug found in this check.")
            else:
                print(f"\n[UNKNOWN] Received unexpected agents_dir: {agents_dir}")
        else:
            print("\n[ERROR] FlowGenerator was not called!")
            print(f"Output: {result.output}")

if __name__ == "__main__":
    test_repro()
