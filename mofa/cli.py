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
@click.argument('test_case_yml', type=click.Path(exists=True))
def debug(node_folder_path, test_case_yml):
    """Run unit tests for a single node/agent"""
    # 1. dynamically load the node module
    node_module = load_node_module(node_folder_path)
    
    # 2. parse the test cases from the YAML file
    test_cases = parse_test_cases(test_case_yml)
 
    # print("==================================")
    # print("Node module loaded:", node_module)
    # print("==================================")
    # print("Test cases loaded:", test_cases)
    # print("==================================")

    # 3. execute tests and generate report
    results = execute_unit_tests(node_module, test_cases)

    # 4. generate and print the test report
    generate_test_report(results)

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
    # First install pip tools to avoid conflicts
    subprocess.run([pip_executable, 'install', '--upgrade', 'pip', 'setuptools', 'wheel'], capture_output=True)

    # Remove pathlib if it exists (conflicts with Python 3.11 built-in pathlib)
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
    base_packages = [
        'numpy==1.26.4',
        'pyarrow==17.0.0',
        'dora-rs-cli',
        'python-dotenv',
        'pyyaml'
    ]
    for package in base_packages:
        install_cmd = [pip_executable, 'install', package]
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
        # Use --no-build-isolation to avoid pathlib conflicts
        install_cmd = [pip_executable, 'install', '--no-build-isolation', '-e', mofa_root]
        proc = subprocess.run(install_cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            raise RuntimeError(f"Failed to install development mofa: {proc.stderr}")
    else:
        # Fallback to PyPI version if we can't find the development version
        install_cmd = [pip_executable, 'install', 'mofa-ai']
        proc = subprocess.run(install_cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            raise RuntimeError(f"Failed to install mofa-ai: {proc.stderr}")

    # Final cleanup: remove pathlib again in case any dependency reinstalled it
    subprocess.run([pip_executable, 'uninstall', '-y', 'pathlib'], capture_output=True)
    for pathlib_file in pathlib_files:
        if os.path.exists(pathlib_file):
            os.remove(pathlib_file)

def _install_packages(pip_executable: str, package_paths: list[str]):
    for package_path in package_paths:
        if not os.path.exists(package_path):
            click.echo(f"Warning: package path not found: {package_path}")
            continue
        install_cmd = [pip_executable, 'install', '--no-build-isolation', '--editable', package_path]
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

    try:
        env_info = _create_venv(sys.executable, working_dir)
        run_env = _build_env(run_env, env_info)

        click.echo("Installing base requirements...")
        _install_base_requirements(env_info['pip'], working_dir)

        editable_packages = _collect_editable_packages(dataflow_path, working_dir)
        if editable_packages:
            click.echo("Installing node packages into isolated environment...")
            _install_packages(env_info['pip'], editable_packages)
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
