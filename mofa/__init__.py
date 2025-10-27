import os
from pathlib import Path
from dotenv import load_dotenv

# Load .env from current working directory
cwd_env_file = os.path.join(os.getcwd(), '.env')
if os.path.exists(cwd_env_file):
    load_dotenv(cwd_env_file)

# Package root is where this __init__.py is located
package_root = os.path.abspath(os.path.dirname(__file__))

# Project root is the current working directory (user's project)
project_root = os.getcwd()

# Path configuration
cli_dir_path = package_root

# Default to current working directory for agents and flows
# Can be overridden by environment variables
agents_dir_path = os.getenv('MOFA_AGENTS_DIR', os.path.join(project_root, 'agents'))
flows_dir_path = os.getenv('MOFA_FLOWS_DIR', os.path.join(project_root, 'flows'))

# Legacy compatibility
agent_dir_path = agents_dir_path  # deprecated, use agents_dir_path
