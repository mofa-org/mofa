"""
MoFA SDK Python Bindings

This module provides Python bindings for the MoFA (Modular Framework for Agents) SDK.
"""

import subprocess
import sys
import tempfile
import os
from pathlib import Path
from typing import Optional

__version__ = "0.1.0"


def check_rust_installed() -> bool:
    """Check if Rust is installed on the system."""
    try:
        result = subprocess.run(["rustc", "--version"], capture_output=True, text=True, timeout=10)
        return result.returncode == 0
    except (subprocess.SubprocessError, FileNotFoundError):
        return False


def install_rust_silent() -> bool:
    """
    Install Rust using rustup with secure subprocess execution.

    This function downloads and executes the rustup installer script
    without using shell=True to prevent command injection vulnerabilities.

    Returns:
        bool: True if installation succeeded, False otherwise.
    """
    if check_rust_installed():
        return True

    script_path = None
    try:
        result = subprocess.run(
            ["curl", "--proto", "=https", "--tlsv1.2", "-sSf", "https://sh.rustup.rs"],
            capture_output=True,
            text=True,
            check=True,
            timeout=60,
        )

        with tempfile.NamedTemporaryFile(mode="w", suffix=".sh", delete=False) as f:
            f.write(result.stdout)
            script_path = f.name

        subprocess.run(
            ["sh", script_path, "-y"], check=True, capture_output=True, text=True, timeout=300
        )

        return check_rust_installed()

    except subprocess.SubprocessError as e:
        print(f"Failed to install Rust: {e}", file=sys.stderr)
        return False
    except Exception as e:
        print(f"Unexpected error during Rust installation: {e}", file=sys.stderr)
        return False
    finally:
        if script_path and os.path.exists(script_path):
            try:
                os.unlink(script_path)
            except OSError:
                pass


def ensure_rust_available(auto_install: bool = False) -> bool:
    """
    Ensure Rust is available for building native extensions.

    Args:
        auto_install: If True, automatically install Rust if not present.
                     If False, prompt the user for confirmation.

    Returns:
        bool: True if Rust is available, False otherwise.
    """
    if check_rust_installed():
        return True

    if auto_install:
        return install_rust_silent()

    response = (
        input("Rust is not installed. Would you like to install it now? [y/N]: ").strip().lower()
    )

    if response in ("y", "yes"):
        return install_rust_silent()

    print(
        "Rust is required for MoFA SDK. Please install it manually from https://rustup.rs",
        file=sys.stderr,
    )
    return False


try:
    from mofa._mofa import *
    from mofa._mofa import __all__ as _mofa_all
except ImportError:
    _mofa_all = []


__all__ = [
    "__version__",
    "check_rust_installed",
    "install_rust_silent",
    "ensure_rust_available",
    *_mofa_all,
]
