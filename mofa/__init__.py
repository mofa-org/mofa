import os
from pathlib import Path
from dotenv import load_dotenv

# Load .env from project root
package_root = os.path.abspath(os.path.dirname(__file__))
project_root = str(Path(package_root).parent)
env_file = os.path.join(project_root, '.env')
if os.path.exists(env_file):
    load_dotenv(env_file)

# Path configuration - can be overridden by .env
cli_dir_path = package_root
agents_dir_path = os.getenv('MOFA_AGENTS_DIR', str(Path(project_root) / 'agents'))
flows_dir_path = os.getenv('MOFA_FLOWS_DIR', str(Path(project_root) / 'flows'))

# Legacy compatibility
agent_dir_path = agents_dir_path  # deprecated, use agents_dir_path
