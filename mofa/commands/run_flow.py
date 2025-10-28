"""
Run flow command implementation for MoFA CLI.
Handles dataflow execution with isolated virtual environment setup.
"""

import os
import sys
import shlex
import shutil
import subprocess
import tempfile
import time
import uuid
import atexit
import signal
from pathlib import Path
from typing import List, Optional

import click

from mofa.utils.files.read import read_yaml
from mofa.utils.process.util import (
    stop_process,
    stop_dora_dataflow,
    destroy_dora_daemon,
)

# Global variable to track venv for cleanup
_venv_root_path = None


def _cleanup_venv():
    """Ask user if they want to keep the venv and clean up if needed."""
    global _venv_root_path

    if _venv_root_path and os.path.exists(_venv_root_path):
        try:
            keep_venv = click.confirm("\nDo you want to keep the virtual environment for next run?", default=True)
            if not keep_venv:
                click.echo(f"Removing venv at {_venv_root_path}...")
                shutil.rmtree(_venv_root_path, ignore_errors=True)
            else:
                click.echo(f"Venv preserved at {_venv_root_path}")
        except:
            # If we can't ask (non-interactive), keep it by default
            click.echo(f"Venv preserved at {_venv_root_path}")


def _register_cleanup_handler():
    """Register signal handlers for cleanup on exit."""
    def cleanup_handler(signum=None, frame=None):
        """Clean up venv on interrupt."""
        if signum in (signal.SIGINT, signal.SIGTERM):
            click.echo("\n\nInterrupted by user.")
            _cleanup_venv()
            sys.exit(0)

    # Register cleanup on Ctrl+C
    signal.signal(signal.SIGINT, cleanup_handler)
    signal.signal(signal.SIGTERM, cleanup_handler)


def find_existing_venv(working_dir: str):
    """Find existing mofa_run_* directories in working_dir."""
    existing_venvs = []
    for item in os.listdir(working_dir):
        if item.startswith("mofa_run_") and os.path.isdir(os.path.join(working_dir, item)):
            venv_path = os.path.join(working_dir, item, "venv")
            if os.path.exists(venv_path):
                existing_venvs.append(os.path.join(working_dir, item))
    return existing_venvs


def create_venv(base_python: str, working_dir: str):
    """Create a virtual environment for running the dataflow."""
    global _venv_root_path

    # Check for existing venvs
    existing_venvs = find_existing_venv(working_dir)
    if existing_venvs:
        # Found existing venv(s), ask user if they want to reuse
        click.echo(f"Found existing virtual environment: {existing_venvs[0]}")
        try:
            reuse = click.confirm("Do you want to reuse it?", default=True)
            if reuse:
                temp_root = existing_venvs[0]
                venv_dir = os.path.join(temp_root, "venv")
                bin_dir = os.path.join(venv_dir, "Scripts" if os.name == "nt" else "bin")
                python_bin = os.path.join(bin_dir, "python.exe" if os.name == "nt" else "python")
                pip_bin = os.path.join(bin_dir, "pip.exe" if os.name == "nt" else "pip")

                try:
                    site_packages = subprocess.check_output(
                        [
                            python_bin,
                            "-c",
                            'import site,sys; paths = getattr(site, "getsitepackages", lambda: [])(); '
                            "print((paths[-1] if paths else site.getusersitepackages()).strip())",
                        ],
                        text=True,
                    ).strip()
                except subprocess.CalledProcessError as exc:
                    click.echo("Warning: Existing venv seems corrupted, creating new one...")
                    shutil.rmtree(temp_root, ignore_errors=True)
                else:
                    _venv_root_path = temp_root
                    click.echo(f"Reusing venv at {temp_root}")
                    return {
                        "root": temp_root,
                        "venv": venv_dir,
                        "bin": bin_dir,
                        "python": python_bin,
                        "pip": pip_bin,
                        "site_packages": site_packages,
                    }
            else:
                # User chose not to reuse, delete old ones
                for old_venv in existing_venvs:
                    click.echo(f"Removing old venv at {old_venv}...")
                    shutil.rmtree(old_venv, ignore_errors=True)
        except:
            pass

    # Create new venv
    temp_root = tempfile.mkdtemp(prefix="mofa_run_", dir=working_dir)
    _venv_root_path = temp_root
    venv_dir = os.path.join(temp_root, "venv")

    create_cmd = [base_python, "-m", "venv", venv_dir]
    create_proc = subprocess.run(create_cmd, capture_output=True, text=True)
    if create_proc.returncode != 0:
        shutil.rmtree(temp_root, ignore_errors=True)
        error_msg = create_proc.stderr.strip() or create_proc.stdout.strip() or "Failed to create virtual environment"

        # Check if it's the ensurepip issue on Ubuntu/Debian
        if "ensurepip is not available" in error_msg or "python3-venv" in error_msg:
            python_version = sys.version_info
            raise RuntimeError(
                f"Python venv module is not available.\n\n"
                f"On Ubuntu/Debian systems, please install:\n"
                f"  sudo apt install python3.{python_version.minor}-venv\n"
                f"or\n"
                f"  sudo apt install python3-venv\n\n"
                f"Then run the command again."
            )
        else:
            raise RuntimeError(error_msg)

    bin_dir = os.path.join(venv_dir, "Scripts" if os.name == "nt" else "bin")
    python_bin = os.path.join(bin_dir, "python.exe" if os.name == "nt" else "python")
    pip_bin = os.path.join(bin_dir, "pip.exe" if os.name == "nt" else "pip")

    try:
        site_packages = subprocess.check_output(
            [
                python_bin,
                "-c",
                'import site,sys; paths = getattr(site, "getsitepackages", lambda: [])(); '
                "print((paths[-1] if paths else site.getusersitepackages()).strip())",
            ],
            text=True,
        ).strip()
    except subprocess.CalledProcessError as exc:
        shutil.rmtree(temp_root, ignore_errors=True)
        raise RuntimeError(
            exc.stderr
            or exc.stdout
            or "Failed to locate site-packages in virtual environment"
        )

    return {
        "root": temp_root,
        "venv": venv_dir,
        "bin": bin_dir,
        "python": python_bin,
        "pip": pip_bin,
        "site_packages": site_packages,
    }


def extract_editable_path(build_command: str):
    """Extract the editable package path from a pip install command."""
    try:
        parts = shlex.split(build_command)
    except ValueError:
        return None

    if len(parts) < 3 or parts[0] != "pip" or parts[1] != "install":
        return None

    for idx, token in enumerate(parts):
        if token in ("-e", "--editable") and idx + 1 < len(parts):
            return parts[idx + 1]
    return None


def collect_editable_packages(dataflow_path: str, working_dir: str):
    """Collect all editable package paths from the dataflow YAML."""
    data = read_yaml(dataflow_path)
    nodes = data.get("nodes", []) if isinstance(data, dict) else []
    editable_paths = []
    for node in nodes:
        if not isinstance(node, dict):
            continue
        build_cmd = node.get("build")
        if isinstance(build_cmd, str):
            editable = extract_editable_path(build_cmd)
            if editable:
                abs_path = os.path.abspath(os.path.join(working_dir, editable))
                editable_paths.append(abs_path)
    return list(dict.fromkeys(editable_paths))


def install_base_requirements(pip_executable: str, working_dir: str):
    """Install base requirements into the venv."""
    click.echo("Installing base requirements...")

    # First install pip tools to avoid conflicts
    subprocess.run([pip_executable, "install", "--upgrade", "pip", "setuptools", "wheel"], capture_output=True)

    # Remove pathlib if it exists (conflicts with Python 3.11+ built-in pathlib)
    subprocess.run([pip_executable, "uninstall", "-y", "pathlib"], capture_output=True)

    # Also remove any broken pathlib files manually
    venv_site_packages = (
        os.path.dirname(os.path.dirname(pip_executable))
        + "/lib/python3.*/site-packages"
    )
    import glob
    for site_pkg_dir in glob.glob(venv_site_packages):
        pathlib_files = [
            os.path.join(site_pkg_dir, "pathlib.py"),
            os.path.join(site_pkg_dir, "pathlib.pyc"),
            os.path.join(site_pkg_dir, "__pycache__", "pathlib.cpython-*.pyc"),
        ]
        for pathlib_pattern in pathlib_files:
            for pathlib_file in glob.glob(pathlib_pattern):
                if os.path.exists(pathlib_file):
                    os.remove(pathlib_file)

    # Install essential packages needed for dora-rs and basic functionality
    base_packages = [
        "numpy==1.26.4",
        "pyarrow==17.0.0",
        "dora-rs-cli",
        "python-dotenv",
        "pyyaml",
    ]
    for package in base_packages:
        install_cmd = [pip_executable, "install", package]
        proc = subprocess.run(install_cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            raise RuntimeError(
                f"Failed to install base package {package}: {proc.stderr}"
            )

    # Install current development version of mofa from the project root
    # Find the mofa project root (where setup.py is located)
    current_dir = working_dir
    mofa_root = None
    while current_dir != "/":
        if os.path.exists(os.path.join(current_dir, "setup.py")):
            setup_content = open(os.path.join(current_dir, "setup.py")).read()
            if "mofa-core" in setup_content:
                mofa_root = current_dir
                break
        current_dir = os.path.dirname(current_dir)

    if mofa_root:
        # Use --no-build-isolation to avoid pathlib conflicts
        install_cmd = [pip_executable, "install", "--no-build-isolation", "-e", mofa_root]
        proc = subprocess.run(install_cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            raise RuntimeError(f"Failed to install development mofa: {proc.stderr}")
    else:
        # Fallback to PyPI version if we can't find the development version
        install_cmd = [pip_executable, "install", "mofa-core"]
        proc = subprocess.run(install_cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            raise RuntimeError(f"Failed to install mofa-core: {proc.stderr}")

    # Final cleanup: remove pathlib again in case any dependency reinstalled it
    subprocess.run([pip_executable, "uninstall", "-y", "pathlib"], capture_output=True)
    for site_pkg_dir in glob.glob(venv_site_packages):
        pathlib_files = [
            os.path.join(site_pkg_dir, "pathlib.py"),
            os.path.join(site_pkg_dir, "pathlib.pyc"),
            os.path.join(site_pkg_dir, "__pycache__", "pathlib.cpython-*.pyc"),
        ]
        for pathlib_pattern in pathlib_files:
            for pathlib_file in glob.glob(pathlib_pattern):
                if os.path.exists(pathlib_file):
                    os.remove(pathlib_file)


def install_packages(pip_executable: str, package_paths: List[str]):
    """Install editable packages using pip."""
    for package_path in package_paths:
        if not os.path.exists(package_path):
            click.echo(f"Warning: package path not found: {package_path}")
            continue
        install_cmd = [pip_executable, "install", "--no-build-isolation", "--editable", package_path]
        proc = subprocess.run(install_cmd, text=True)
        if proc.returncode != 0:
            raise RuntimeError(f"Failed to install package from {package_path}")


def build_env(base_env: dict, venv_info: dict):
    """Build environment variables for running in the virtual environment."""
    env = base_env.copy()
    env_path = env.get("PATH", "")
    env["PATH"] = venv_info["bin"] + os.pathsep + env_path
    env["VIRTUAL_ENV"] = venv_info["venv"]
    env["PYTHONNOUSERSITE"] = "1"
    site_packages = venv_info.get("site_packages")
    if site_packages:
        existing_pythonpath = env.get("PYTHONPATH", "")
        combined = (
            site_packages
            if not existing_pythonpath
            else site_packages + os.pathsep + existing_pythonpath
        )
        env["PYTHONPATH"] = combined
    env["PIP_NO_BUILD_ISOLATION"] = "1"
    return env


def run_flow(dataflow_file: str):
    """Execute a dataflow from the given YAML file."""
    # Register cleanup handlers for Ctrl+C and normal exit
    _register_cleanup_handler()

    dataflow_path = os.path.abspath(dataflow_file)
    if not os.path.exists(dataflow_path):
        click.echo(f"Error: Dataflow file not found: {dataflow_path}")
        return

    if not dataflow_path.endswith(".yml") and not dataflow_path.endswith(".yaml"):
        click.echo(f"Error: File must be a YAML file (.yml or .yaml): {dataflow_path}")
        return

    # Get the directory containing the dataflow file
    working_dir = os.path.dirname(dataflow_path)

    # Check if dora is available
    try:
        dora_check = subprocess.run(
            ["dora", "--version"],
            capture_output=True,
            text=True,
            timeout=5
        )
        if dora_check.returncode != 0:
            click.echo("Error: dora command not found or not working properly.")
            click.echo("Please ensure dora-rs is installed correctly.")
            return
    except (FileNotFoundError, subprocess.TimeoutExpired):
        click.echo("Error: dora command not found or timed out.")
        click.echo("Please ensure dora-rs is installed correctly.")
        return

    # Clean up any existing dora processes to avoid conflicts
    click.echo("Cleaning up existing dora processes...")
    try:
        subprocess.run(["pkill", "-f", "dora"], capture_output=True, check=False)
    except FileNotFoundError:
        # pkill might not be available on all systems, try alternative
        try:
            subprocess.run(["killall", "dora"], capture_output=True, check=False)
        except FileNotFoundError:
            # If neither pkill nor killall is available, skip cleanup
            pass
    time.sleep(1)

    env_info = None
    run_env = os.environ.copy()
    editable_packages = []

    try:
        env_info = create_venv(sys.executable, working_dir)
        run_env = build_env(run_env, env_info)

        # Check if base requirements are already installed (for reused venv)
        # Try importing dora_rs to check if base requirements are installed
        check_cmd = [env_info["python"], "-c", "import dora_rs"]
        check_result = subprocess.run(check_cmd, capture_output=True)

        if check_result.returncode != 0:
            # Base requirements not installed, install them
            install_base_requirements(env_info["pip"], working_dir)

        editable_packages = collect_editable_packages(dataflow_path, working_dir)
        if editable_packages:
            click.echo("Installing agent packages...")
            install_packages(env_info["pip"], editable_packages)
    except RuntimeError as runtime_error:
        click.echo(f"Failed to prepare run environment: {runtime_error}")
        _cleanup_venv()
        return

    dora_up_process = None
    dora_build_node = None
    dora_dataflow_process = None
    task_input_process = None
    dataflow_name = None

    try:
        dora_up_process = subprocess.Popen(
            ["dora", "up"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd=working_dir,
            env=run_env,
        )
        time.sleep(1)

        dora_build_node = subprocess.Popen(
            ["dora", "build", dataflow_path],
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

        dataflow_name = str(uuid.uuid4()).replace("-", "")
        click.echo(f"Starting dataflow with name: {dataflow_name}")
        dora_dataflow_process = subprocess.Popen(
            ["dora", "start", dataflow_path, "--name", dataflow_name],
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
            ["terminal-input"], cwd=working_dir, env=run_env
        )

        # Wait for terminal-input to finish (user interaction)
        try:
            task_input_process.wait()
        except KeyboardInterrupt:
            click.echo("\nReceived interrupt signal, shutting down...")
            task_input_process.terminate()
    finally:
        stop_process(
            [
                task_input_process,
                dora_dataflow_process,
                dora_build_node,
                dora_up_process,
            ]
        )
        if dataflow_name:
            stop_dora_dataflow(dataflow_name=dataflow_name)
        destroy_dora_daemon()
        click.echo("Main process terminated.")

        # Ask user if they want to keep the venv
        _cleanup_venv()
