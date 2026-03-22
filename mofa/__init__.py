import os
from pathlib import Path

# Base paths - dynamically resolved to ensure portability
project_root = str(Path(__file__).parent.parent.absolute())
agents_dir_path = os.path.join(project_root, "agents")
flows_dir_path = os.path.join(project_root, "flows")
