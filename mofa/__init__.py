import os
from pathlib import Path
package_root = os.path.abspath(os.path.dirname(__file__))
cli_dir_path = os.path.join(os.path.abspath(os.path.dirname(__file__)), )
agent_dir_path = str(Path(str(cli_dir_path)).parent / 'examples')
