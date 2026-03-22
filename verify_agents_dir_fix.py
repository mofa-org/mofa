import os
import sys
from unittest.mock import patch, MagicMock

# 1. Setup mock mofa environment
sys.path.append(os.getcwd())
mofa_mock = MagicMock()
mofa_mock.agents_dir_path = "/default/agents"
mofa_mock.flows_dir_path = "/default/flows"
mofa_mock.project_root = os.getcwd()
sys.modules['mofa'] = mofa_mock

# Mock vibe engine/generator to avoid import errors
sys.modules['mofa.vibe'] = MagicMock()
sys.modules['mofa.vibe.engine'] = MagicMock()
sys.modules['mofa.vibe.models'] = MagicMock()
sys.modules['mofa.vibe.flow_generator'] = MagicMock()

import mofa.commands.vibe as vibe
from click.testing import CliRunner
import click

def test_verify_agents_dir():
    print("Verifying agents_dir fix in FlowGenerator calls...")
    runner = CliRunner()
    
    @click.group()
    def cli():
        pass
    vibe.register_vibe_commands(cli)
    
    # Custom config to be returned by _load_vibe_config
    saved_config = {
        "model": "gpt-4",
        "max_rounds": 100,
        "agents_output": "/custom/agents",
        "flows_output": "/custom/flows",
    }
    
    # Mock dependencies
    with patch('mofa.commands.vibe._load_vibe_config', return_value=saved_config), \
         patch('mofa.commands.vibe._save_vibe_config'), \
         patch('mofa.commands.vibe._check_and_setup_api_key', return_value="fake-key"), \
         patch('mofa.vibe.flow_generator.FlowGenerator') as MockFlowGenerator:
        
        # Test 1: 'mofa vibe flow' (subcommand)
        print("\nTesting 'mofa vibe flow' subcommand...")
        result = runner.invoke(cli, ["vibe", "flow"], input="gpt-4\n/custom/flows\nmy requirement\n")
        
        if MockFlowGenerator.called:
            args, kwargs = MockFlowGenerator.call_args
            agents_dir = kwargs.get('agents_dir')
            print(f"Subcommand FlowGenerator received agents_dir: {agents_dir}")
            if agents_dir == saved_config['agents_output']:
                print("[SUCCESS] Subcommand correctly used custom agents_dir.")
            else:
                print(f"[FAILED] Subcommand used: {agents_dir}")
        
        MockFlowGenerator.reset_mock()
        
        # Test 2: 'mofa vibe' -> option 2 (TUI)
        print("\nTesting 'mofa vibe' TUI (option 2)...")
        result = runner.invoke(cli, ["vibe"], input="2\ngpt-4\n/custom/flows\nmy requirement\n")
        
        if MockFlowGenerator.called:
            args, kwargs = MockFlowGenerator.call_args
            agents_dir = kwargs.get('agents_dir')
            print(f"TUI FlowGenerator received agents_dir: {agents_dir}")
            if agents_dir == saved_config['agents_output']:
                print("[SUCCESS] TUI correctly used custom agents_dir.")
            else:
                print(f"[FAILED] TUI used: {agents_dir}")

if __name__ == "__main__":
    test_verify_agents_dir()
