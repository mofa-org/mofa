import os
import sys
from unittest.mock import patch, MagicMock

# Add current directory to sys.path
sys.path.append(os.path.abspath('.'))

# We need to mock agents_dir_path from mofa BEFORE importing vibe.py if possible,
# or just check how it's imported in vibe.py.
# In vibe.py it does: from mofa import agents_dir_path, flows_dir_path, project_root

import mofa
# Mock the default
mofa.agents_dir_path = "/default/agents"

from mofa.commands.vibe import register_vibe_commands
import click
from click.testing import CliRunner

def test_repro():
    runner = CliRunner()
    
    @click.group()
    def cli():
        pass
    register_vibe_commands(cli)
    
    # 1. Setup .env with a custom agents output
    env_file = os.path.join(mofa.project_root, ".env")
    with open(env_file, 'w') as f:
        f.write("MOFA_VIBE_AGENTS_OUTPUT=/custom/agents\n")
        f.write("MOFA_VIBE_MODEL=gpt-4\n")
        f.write("OPENAI_API_KEY=fake-key\n")

    # 2. Mock the FlowGenerator to see what arguments it receives
    captured_args = {}
    class MockFlowGenerator:
        def __init__(self, agents_dir, flows_dir, llm_model, api_key):
            captured_args['agents_dir'] = agents_dir
            captured_args['flows_dir'] = flows_dir
        def generate_flow(self, requirement):
            return "/tmp/flow"

    with patch('mofa.vibe.flow_generator.FlowGenerator', MockFlowGenerator), \
         patch('mofa.commands.vibe._check_and_setup_api_key', return_value="fake-key"):
        
        # Run 'mofa vibe' and select option 2 (flow)
        # Inputs:
        # "2" (vibe type)
        # "gpt-4" (llm model prompt)
        # "/tmp/flows" (output dir prompt)
        # "my requirement" (requirement prompt)
        result = runner.invoke(cli, ["vibe"], input="2\ngpt-4\n/tmp/flows\nmy requirement\n")
        
    print(f"Exit code: {result.exit_code}")
    print(f"Output: {result.output}")
    
    print(f"\nCaptured agents_dir: {captured_args.get('agents_dir')}")
    print(f"Default agents_dir: {mofa.agents_dir_path}")
    
    if captured_args.get('agents_dir') == mofa.agents_dir_path:
        print("\n[BUG REPRODUCED] FlowGenerator used the default agents_dir instead of the custom one from .env!")
    elif captured_args.get('agents_dir') == "/custom/agents":
        print("\n[FIXED] FlowGenerator correctly used the custom agents_dir.")
    else:
        print("\n[UNKNOWN] Something else happened.")

if __name__ == "__main__":
    test_repro()
