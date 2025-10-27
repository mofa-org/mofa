import os
import shutil
import time
import uuid
import shlex
import subprocess
import tempfile
from mofa import agent_dir_path, cli_dir_path

import click
import sys
from mofa.debug.actor import execute_unit_tests
from mofa.debug.gen_reporter import generate_test_report
from mofa.debug.iteractive import collect_interactive_input
from mofa.debug.load_node import load_node_module
from mofa.debug.parse_test_case import parse_test_cases
from mofa.utils.files.dir import get_subdirectories
from mofa.utils.files.read import read_yaml
from mofa.utils.process.util import stop_process, stop_dora_dataflow, destroy_dora_daemon

import cookiecutter
from cookiecutter.main import cookiecutter


@click.group()
def mofa_cli_group():
    """Main CLI for MAE"""
    pass


@mofa_cli_group.command()
def agent_list():
    """List all agents"""
    print("agent_dir_path ",agent_dir_path)
    agent_names = get_subdirectories(agent_dir_path)
    click.echo(agent_names)
    return agent_names

@mofa_cli_group.command()
@click.argument('node_folder_path', type=click.Path(exists=True))
@click.argument('test_case_yml', type=click.Path(exists=True), required=False)  # YAML可选
@click.option('--interactive', is_flag=True, help='启用交互式输入（无需YAML文件）')
def debug(node_folder_path, test_case_yml, interactive):
    """Run unit tests for a single node/agent"""
    # 1. dynamically load the node module
    node_module = load_node_module(node_folder_path)

    # 2. parse the test cases from the YAML file
    if interactive:
        # 检查是否同时传入了YAML文件（冲突提示）
        if test_case_yml:
            raise click.BadParameter("交互式模式下不需要传入YAML文件，请移除test_case_yml参数")
        test_cases = collect_interactive_input()  # 交互式收集用例
    else:
        # 传统模式：必须传入YAML文件
        if not test_case_yml:
            raise click.BadParameter("非交互式模式下必须传入YAML文件路径")
        test_cases = parse_test_cases(test_case_yml)  # 从YAML解析用例
    # print("==================================")
    # print("Node module loaded:", node_module)
    # print("==================================")
    # print("Test cases loaded:", test_cases)
    # print("==================================")

    # 3. execute tests and generate report
    results = execute_unit_tests(node_module, test_cases)

    # 4. generate and print the test report
    generate_test_report(results)


@mofa_cli_group.command()
@click.option('--llm', default='gpt-4', help='LLM model to use (default: gpt-4)')
@click.option('--max-rounds', default=100, help='Maximum optimization rounds (default: 100, use 0 for unlimited)')
@click.option('--output', '-o', default='./agent-hub', help='Output directory (default: ./agent-hub)')
def vibe(llm, max_rounds, output):
    """AI-powered agent generator with automatic testing and optimization

    Vibe generates MoFA agents from natural language descriptions,
    automatically creates test cases, and iteratively optimizes the code
    until all tests pass.

    Usage:
        mofa vibe
        mofa vibe --llm gpt-4 --max-rounds 3
    """
    try:
        from mofa.vibe.engine import VibeEngine
        from mofa.vibe.models import VibeConfig
        from dotenv import load_dotenv
    except ImportError as e:
        click.echo(f"ERROR: Failed to import vibe module: {e}")
        click.echo("Make sure all dependencies are installed:")
        click.echo("  pip install openai rich pyyaml python-dotenv")
        return

    # Load .env file if it exists
    env_file = os.path.join(os.getcwd(), '.env')
    if os.path.exists(env_file):
        load_dotenv(env_file)

    # Check for API key and prompt user if not found
    api_key = os.getenv('OPENAI_API_KEY')
    if not api_key:
        click.echo("\nOpenAI API Key Required")
        click.echo("-" * 50)
        click.echo("Vibe needs an OpenAI API key to generate agents.")
        click.echo("You can get one at: https://platform.openai.com/api-keys")
        click.echo()

        api_key = click.prompt(
            "Please enter your OpenAI API key",
            type=str,
            hide_input=True
        )

        if not api_key or not api_key.startswith('sk-'):
            click.echo("ERROR: Invalid API key format. Should start with 'sk-'")
            sys.exit(1)

        # Set for current session
        os.environ['OPENAI_API_KEY'] = api_key

        # Ask if user wants to save it
        click.echo()
        save_key = click.confirm(
            "Would you like to save this API key to .env file for future use?",
            default=True
        )

        if save_key:
            env_file = os.path.join(os.getcwd(), '.env')
            try:
                # Append to .env file
                with open(env_file, 'a') as f:
                    f.write(f"\n# Added by mofa vibe on {subprocess.check_output(['date'], text=True).strip()}\n")
                    f.write(f"OPENAI_API_KEY={api_key}\n")
                click.echo(f"API key saved to {env_file}")
                click.echo("  (Make sure to add .env to .gitignore!)")
            except Exception as e:
                click.echo(f"WARNING: Could not save to .env file: {e}")
        click.echo()

    # Create config
    config = VibeConfig(
        llm_model=llm,
        max_optimization_rounds=max_rounds,
        output_dir=output,
        llm_api_key=api_key
    )

    # Run vibe engine
    try:
        engine = VibeEngine(config=config)
        result = engine.run_interactive()

        if result and result.success:
            sys.exit(0)
        else:
            sys.exit(1)

    except KeyboardInterrupt:
        click.echo("\n\nVibe exited")
        sys.exit(0)
    except ValueError as e:
        if "API key" in str(e):
            click.echo(f"\nERROR: {e}")
            click.echo("Please set OPENAI_API_KEY environment variable or re-run mofa vibe")
            sys.exit(1)
        raise
    except Exception as e:
        click.echo(f"\nERROR: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

def _create_venv(base_python: str, working_dir: str):
    temp_root = tempfile.mkdtemp(prefix="mofa_run_", dir=working_dir)
    venv_dir = os.path.join(temp_root, "venv")
    create_cmd = [base_python, '-m', 'venv', venv_dir]
    create_proc = subprocess.run(create_cmd, capture_output=True, text=True)
    if create_proc.returncode != 0:
        shutil.rmtree(temp_root, ignore_errors=True)
        raise RuntimeError(create_proc.stderr.strip() or create_proc.stdout.strip() or "Failed to create virtual environment")

    bin_dir = os.path.join(venv_dir, 'Scripts' if os.name == 'nt' else 'bin')
    python_bin = os.path.join(bin_dir, 'python.exe' if os.name == 'nt' else 'python')
    pip_bin = os.path.join(bin_dir, 'pip.exe' if os.name == 'nt' else 'pip')

    try:
        site_packages = subprocess.check_output([
            python_bin,
            '-c',
            'import site,sys; paths = getattr(site, "getsitepackages", lambda: [])(); '
            'print((paths[-1] if paths else site.getusersitepackages()).strip())'
        ], text=True).strip()
    except subprocess.CalledProcessError as exc:
        shutil.rmtree(temp_root, ignore_errors=True)
        raise RuntimeError(exc.stderr or exc.stdout or "Failed to locate site-packages in virtual environment")

    return {
        'root': temp_root,
        'venv': venv_dir,
        'bin': bin_dir,
        'python': python_bin,
        'pip': pip_bin,
        'site_packages': site_packages,
    }


def _extract_editable_path(build_command: str):
    try:
        parts = shlex.split(build_command)
    except ValueError:
        return None

    if len(parts) < 3 or parts[0] != 'pip' or parts[1] != 'install':
        return None

    for idx, token in enumerate(parts):
        if token in ('-e', '--editable') and idx + 1 < len(parts):
            return parts[idx + 1]
    return None


def _collect_editable_packages(dataflow_path: str, working_dir: str):
    data = read_yaml(dataflow_path)
    nodes = data.get('nodes', []) if isinstance(data, dict) else []
    editable_paths = []
    for node in nodes:
        if not isinstance(node, dict):
            continue
        build_cmd = node.get('build')
        if isinstance(build_cmd, str):
            editable = _extract_editable_path(build_cmd)
            if editable:
                abs_path = os.path.abspath(os.path.join(working_dir, editable))
                editable_paths.append(abs_path)
    return list(dict.fromkeys(editable_paths))


def _install_base_requirements(pip_executable: str, working_dir: str):
    # First install uv in the venv for faster package installation
    click.echo("Installing uv in virtual environment...")
    subprocess.run([pip_executable, 'install', '--upgrade', 'pip'], capture_output=True)
    uv_install = subprocess.run([pip_executable, 'install', 'uv'], capture_output=True, text=True)

    # Determine the uv and python executable paths in the venv
    bin_dir = os.path.dirname(pip_executable)
    uv_executable = os.path.join(bin_dir, 'uv.exe' if os.name == 'nt' else 'uv')
    python_executable = os.path.join(bin_dir, 'python.exe' if os.name == 'nt' else 'python')

    # Check if uv was installed successfully
    use_uv = uv_install.returncode == 0 and os.path.exists(uv_executable)

    if use_uv:
        click.echo("✓ Using uv for fast package installation")
        # Use --python to ensure uv installs into the correct venv
        installer = [uv_executable, 'pip', 'install', '--python', python_executable]
    else:
        click.echo("⚠ Using pip (uv installation failed)")
        installer = [pip_executable, 'install']
        # Upgrade pip tools if using pip
        subprocess.run([pip_executable, 'install', '--upgrade', 'setuptools', 'wheel'], capture_output=True)

    # Remove pathlib if it exists (conflicts with Python 3.11 built-in pathlib)
    if use_uv:
        subprocess.run([uv_executable, 'pip', 'uninstall', '--python', python_executable, '-y', 'pathlib'], capture_output=True)
    else:
        subprocess.run([pip_executable, 'uninstall', '-y', 'pathlib'], capture_output=True)

    # Also remove any broken pathlib files manually
    venv_site_packages = os.path.dirname(os.path.dirname(pip_executable)) + '/lib/python3.11/site-packages'
    pathlib_files = [
        os.path.join(venv_site_packages, 'pathlib.py'),
        os.path.join(venv_site_packages, 'pathlib.pyc'),
        os.path.join(venv_site_packages, '__pycache__', 'pathlib.cpython-311.pyc')
    ]
    for pathlib_file in pathlib_files:
        if os.path.exists(pathlib_file):
            os.remove(pathlib_file)

    # Install essential packages needed for dora-rs and basic functionality
    click.echo("Installing base packages...")
    base_packages = [
        'numpy==1.26.4',
        'pyarrow==17.0.0',
        'dora-rs-cli',
        'python-dotenv',
        'pyyaml'
    ]
    for package in base_packages:
        install_cmd = installer + [package]
        proc = subprocess.run(install_cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            raise RuntimeError(f"Failed to install base package {package}: {proc.stderr}")

    # Install current development version of mofa from the project root
    # Find the mofa project root (where setup.py is located)
    current_dir = working_dir
    mofa_root = None
    while current_dir != '/':
        if os.path.exists(os.path.join(current_dir, 'setup.py')):
            setup_content = open(os.path.join(current_dir, 'setup.py')).read()
            if 'mofa-ai' in setup_content:
                mofa_root = current_dir
                break
        current_dir = os.path.dirname(current_dir)

    if mofa_root:
        click.echo("Installing mofa development version...")
        # Use --no-build-isolation to avoid pathlib conflicts
        install_cmd = installer + ['--no-build-isolation', '-e', mofa_root]
        proc = subprocess.run(install_cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            # If development install fails (e.g., permission issues), fall back to PyPI
            if 'Permission denied' in proc.stderr:
                click.echo("⚠ Permission error installing dev version, using PyPI version...")
                install_cmd = installer + ['mofa-core']
                proc = subprocess.run(install_cmd, capture_output=True, text=True)
                if proc.returncode != 0:
                    raise RuntimeError(f"Failed to install mofa-core: {proc.stderr}")
            else:
                raise RuntimeError(f"Failed to install development mofa: {proc.stderr}")
    else:
        # Fallback to PyPI version if we can't find the development version
        install_cmd = installer + ['mofa-core']
        proc = subprocess.run(install_cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            raise RuntimeError(f"Failed to install mofa-core: {proc.stderr}")

    # Final cleanup: remove pathlib again in case any dependency reinstalled it
    if use_uv:
        subprocess.run([uv_executable, 'pip', 'uninstall', '--python', python_executable, '-y', 'pathlib'], capture_output=True)
    else:
        subprocess.run([pip_executable, 'uninstall', '-y', 'pathlib'], capture_output=True)
    for pathlib_file in pathlib_files:
        if os.path.exists(pathlib_file):
            os.remove(pathlib_file)

    # Return the installer command for use in other functions
    return installer if use_uv else None

def _install_packages(pip_executable: str, package_paths: list[str], installer=None):
    """Install packages using uv (if available) or pip."""
    # Use provided installer or fallback to pip
    if installer is None:
        installer = [pip_executable, 'install']

    for package_path in package_paths:
        if not os.path.exists(package_path):
            click.echo(f"Warning: package path not found: {package_path}")
            continue
        install_cmd = installer + ['--no-build-isolation', '--editable', package_path]
        proc = subprocess.run(install_cmd, text=True)
        if proc.returncode != 0:
            raise RuntimeError(f"Failed to install package from {package_path}")


def _build_env(base_env: dict, venv_info: dict):
    env = base_env.copy()
    env_path = env.get('PATH', '')
    env['PATH'] = venv_info['bin'] + os.pathsep + env_path
    env['VIRTUAL_ENV'] = venv_info['venv']
    env['PYTHONNOUSERSITE'] = '1'
    site_packages = venv_info.get('site_packages')
    if site_packages:
        existing_pythonpath = env.get('PYTHONPATH', '')
        combined = site_packages if not existing_pythonpath else site_packages + os.pathsep + existing_pythonpath
        env['PYTHONPATH'] = combined
    env['PIP_NO_BUILD_ISOLATION'] = '1'
    return env
@mofa_cli_group.command()
@click.argument('dataflow_file', required=True)
def run(dataflow_file: str):
    """Use run <path-to-dataflow.yml> to run in venv."""
    dataflow_path = os.path.abspath(dataflow_file)
    if not os.path.exists(dataflow_path):
        click.echo(f"Error: Dataflow file not found: {dataflow_path}")
        return

    if not dataflow_path.endswith('.yml') and not dataflow_path.endswith('.yaml'):
        click.echo(f"Error: File must be a YAML file (.yml or .yaml): {dataflow_path}")
        return

    # Get the directory containing the dataflow file
    working_dir = os.path.dirname(dataflow_path)

    # Clean up any existing dora processes to avoid conflicts
    click.echo("Cleaning up existing dora processes...")
    subprocess.run(['pkill', '-f', 'dora'], capture_output=True)
    time.sleep(1)  # Give processes time to die

    env_info = None
    run_env = os.environ.copy()
    editable_packages = []
    installer = None

    try:
        env_info = _create_venv(sys.executable, working_dir)
        run_env = _build_env(run_env, env_info)

        click.echo("Installing base requirements...")
        installer = _install_base_requirements(env_info['pip'], working_dir)

        editable_packages = _collect_editable_packages(dataflow_path, working_dir)
        if editable_packages:
            click.echo("Installing node packages into isolated environment...")
            _install_packages(env_info['pip'], editable_packages, installer=installer)
    except RuntimeError as runtime_error:
        click.echo(f"Failed to prepare run environment: {runtime_error}")
        if env_info:
            shutil.rmtree(env_info['root'], ignore_errors=True)
        return

    dora_up_process = None
    dora_build_node = None
    dora_dataflow_process = None
    task_input_process = None
    dataflow_name = None

    try:
        dora_up_process = subprocess.Popen(
            ['dora', 'up'],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd=working_dir,
            env=run_env,
        )
        time.sleep(1)

        dora_build_node = subprocess.Popen(
            ['dora', 'build', dataflow_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd=working_dir,
            env=run_env,
        )

        time.sleep(3)
        stdout, stderr = dora_build_node.communicate()
        if dora_build_node.returncode != 0:
            build_error = stderr.strip() if stderr else stdout.strip()
            if build_error:
                click.echo(build_error)
            click.echo("Failed to build dataflow. Aborting run.")
            return

        dataflow_name = str(uuid.uuid4()).replace('-','')
        click.echo(f"Starting dataflow with name: {dataflow_name}")
        dora_dataflow_process = subprocess.Popen(
            ['dora', 'start', dataflow_path,'--name',dataflow_name],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd=working_dir,
            env=run_env,
        )

        time.sleep(2)

        # Check if dataflow started successfully
        if dora_dataflow_process.poll() is not None:
            stdout, stderr = dora_dataflow_process.communicate()
            click.echo(f"Dataflow process terminated early!")
            if stderr:
                click.echo(f"Stderr: {stderr}")
            if stdout:
                click.echo(f"Stdout: {stdout}")
            return

        click.echo("Starting terminal-input process...")
        click.echo("You can now interact directly with the agents. Type 'exit' to quit.")

        # Start terminal-input with direct stdin/stdout connection
        task_input_process = subprocess.Popen(
            ['terminal-input'],
            cwd=working_dir,
            env=run_env
        )

        # Wait for terminal-input to finish (user interaction)
        try:
            task_input_process.wait()
        except KeyboardInterrupt:
            click.echo("\nReceived interrupt signal, shutting down...")
            task_input_process.terminate()
    finally:
        stop_process([task_input_process, dora_dataflow_process, dora_build_node, dora_up_process])
        if dataflow_name:
            stop_dora_dataflow(dataflow_name=dataflow_name)
        destroy_dora_daemon()
        if env_info:
            shutil.rmtree(env_info['root'], ignore_errors=True)
        click.echo("Main process terminated.")

@mofa_cli_group.command()
@click.argument('agent_name', required=True)
@click.option('--version', default='0.0.1', help='Version of the new agent')
@click.option('--output', default=os.getcwd()+"/", help='node output path')
@click.option('--authors', default='Mofa Bot', help='authors')
def new_agent(agent_name: str, version: str, output: str, authors: str):
    """Create a new agent from template."""

    # Define the template directory
    # template_dir = os.path.join(os.path.dirname(agent_dir_path), 'agent-hub', 'agent-template')
    template_dir = os.path.join(cli_dir_path,'agent-template')

    # Ensure the template directory exists and contains cookiecutter.json
    if not os.path.exists(template_dir):
        click.echo(f"Template directory not found: {template_dir}")
        return
    if not os.path.isfile(os.path.join(template_dir, 'cookiecutter.json')):
        click.echo(f"Template directory must contain a cookiecutter.json file: {template_dir}")
        return

    # Use Cookiecutter to generate the new agent from the template
    try:
        cookiecutter(
            template=template_dir,
            output_dir=output,
            no_input=True,  # Enable interactive input
            extra_context={
                'user_agent_dir': agent_name,
                'agent_name': agent_name,  # Use the provided agent_name
                'version': version,  # Use the provided version
                'authors': authors
            }
        )
        click.echo(f"Successfully created new agent in {output}{agent_name}")
    except Exception as e:
        click.echo(f"Failed to create new agent: {e}")
        return

if __name__ == '__main__':
    mofa_cli_group()
