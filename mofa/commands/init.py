"""
MoFA init command - Initialize MoFA workspace with examples
"""
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Optional

import click
from rich.console import Console
from rich.panel import Panel
from rich.progress import Progress, SpinnerColumn, TextColumn

console = Console()


def check_rust_installed() -> bool:
    """Check if Rust/Cargo is installed"""
    try:
        result = subprocess.run(
            ["cargo", "--version"],
            capture_output=True,
            text=True,
            timeout=5
        )
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def install_rust() -> bool:
    """Install Rust using rustup"""
    console.print("\n[yellow]Rust is not installed. Installing Rust...[/yellow]")
    console.print("[dim]This may take a few minutes...[/dim]\n")

    try:
        # Download and run rustup installer
        install_cmd = 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'

        result = subprocess.run(
            install_cmd,
            shell=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )

        if result.returncode != 0:
            console.print(f"[red]Failed to install Rust: {result.stderr}[/red]")
            return False

        console.print("[green]✓[/green] Rust installed successfully!")

        # Source cargo env
        cargo_env = Path.home() / ".cargo" / "env"
        if cargo_env.exists():
            console.print("[dim]Please run: source $HOME/.cargo/env[/dim]")

        return True

    except Exception as e:
        console.print(f"[red]Error installing Rust: {e}[/red]")
        return False


def download_examples_from_github(target_dir: Path) -> bool:
    """Download examples from GitHub using sparse checkout"""
    import tempfile

    try:
        console.print("[yellow]Trying to download examples from GitHub...[/yellow]")

        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)

            # Initialize sparse checkout
            commands = [
                ["git", "init"],
                ["git", "remote", "add", "origin", "https://github.com/mofa-org/mofa.git"],
                ["git", "config", "core.sparseCheckout", "true"],
            ]

            for cmd in commands:
                result = subprocess.run(
                    cmd,
                    cwd=temp_path,
                    capture_output=True,
                    text=True,
                    timeout=30
                )
                if result.returncode != 0:
                    console.print(f"[red]Git command failed: {' '.join(cmd)}[/red]")
                    return False

            # Configure sparse checkout patterns
            sparse_checkout_file = temp_path / ".git" / "info" / "sparse-checkout"
            sparse_checkout_file.parent.mkdir(parents=True, exist_ok=True)
            sparse_checkout_file.write_text("agents/\nflows/\n")

            # Pull the specified directories
            result = subprocess.run(
                ["git", "pull", "origin", "main"],
                cwd=temp_path,
                capture_output=True,
                text=True,
                timeout=120
            )

            if result.returncode != 0:
                console.print(f"[red]Failed to pull from GitHub: {result.stderr}[/red]")
                return False

            # Copy the downloaded directories
            source_agents = temp_path / "agents"
            source_flows = temp_path / "flows"

            if not source_agents.exists() or not source_flows.exists():
                console.print("[red]Downloaded content missing agents or flows[/red]")
                return False

            return copy_examples(target_dir, source_agents, source_flows)

    except subprocess.TimeoutExpired:
        console.print("[red]GitHub download timed out[/red]")
        return False
    except Exception as e:
        console.print(f"[red]Error downloading from GitHub: {e}[/red]")
        return False


def copy_examples(target_dir: Path, source_agents: Path, source_flows: Path) -> bool:
    """Copy agents and flows directories to target directory"""
    try:
        # Copy agents
        target_agents = target_dir / "agents"
        if target_agents.exists():
            console.print(f"[yellow]⚠[/yellow]  agents/ already exists, skipping...")
        else:
            shutil.copytree(source_agents, target_agents)
            console.print(f"[green]✓[/green] Copied agents/ to {target_dir}")

        # Copy flows
        target_flows = target_dir / "flows"
        if target_flows.exists():
            console.print(f"[yellow]⚠[/yellow]  flows/ already exists, skipping...")
        else:
            shutil.copytree(source_flows, target_flows)
            console.print(f"[green]✓[/green] Copied flows/ to {target_dir}")

        return True

    except Exception as e:
        console.print(f"[red]Error copying examples: {e}[/red]")
        return False


def create_env_file(target_dir: Path) -> bool:
    """Create .env file with MoFA configuration"""
    env_file = target_dir / ".env"

    try:
        # Check if .env already exists
        if env_file.exists():
            # Read existing content
            existing_content = env_file.read_text()

            # Check if MoFA variables are already set
            has_agents_dir = "MOFA_AGENTS_DIR" in existing_content
            has_flows_dir = "MOFA_FLOWS_DIR" in existing_content

            if has_agents_dir and has_flows_dir:
                console.print(f"[yellow]⚠[/yellow]  .env already has MoFA configuration, skipping...")
                return True

            # Append to existing .env
            with open(env_file, "a") as f:
                f.write("\n# MoFA Configuration\n")
                if not has_agents_dir:
                    f.write(f"MOFA_AGENTS_DIR={target_dir}/agents\n")
                if not has_flows_dir:
                    f.write(f"MOFA_FLOWS_DIR={target_dir}/flows\n")

            console.print(f"[green]✓[/green] Updated .env with MoFA configuration")
        else:
            # Create new .env
            env_content = f"""# MoFA Configuration
# Agents directory - where your custom agents are stored
MOFA_AGENTS_DIR={target_dir}/agents

# Flows directory - where your dataflow configurations are stored
MOFA_FLOWS_DIR={target_dir}/flows

# Add your API keys here
# OPENAI_API_KEY=your-key-here
"""
            env_file.write_text(env_content)
            console.print(f"[green]✓[/green] Created .env file with MoFA configuration")

        return True

    except Exception as e:
        console.print(f"[red]Error creating .env file: {e}[/red]")
        return False


@click.command(name="init")
@click.option(
    "--skip-rust",
    is_flag=True,
    help="Skip Rust installation check"
)
@click.option(
    "--skip-examples",
    is_flag=True,
    help="Skip copying example agents and flows"
)
@click.argument(
    "directory",
    type=click.Path(),
    default=".",
    required=False
)
def init_command(skip_rust: bool, skip_examples: bool, directory: str):
    """
    Initialize a MoFA workspace

    This command will:
    - Check and optionally install Rust/Cargo (required for dora-rs)
    - Copy example agents and flows to your working directory
    - Create .env file with MoFA_AGENTS_DIR and MOFA_FLOWS_DIR configuration

    Examples:
        mofa init                    # Initialize in current directory
        mofa init ./my-project       # Initialize in specific directory
        mofa init --skip-rust        # Skip Rust installation
        mofa init --skip-examples    # Only install Rust
    """
    console.print(Panel.fit(
        "[bold cyan]MoFA Workspace Initialization[/bold cyan]",
        border_style="cyan"
    ))

    target_dir = Path(directory).resolve()

    # Ensure target directory exists
    if not target_dir.exists():
        target_dir.mkdir(parents=True, exist_ok=True)
        console.print(f"[green]✓[/green] Created directory: {target_dir}")

    console.print(f"\n[bold]Target directory:[/bold] {target_dir}\n")

    # Check/Install Rust
    if not skip_rust:
        if check_rust_installed():
            console.print("[green]✓[/green] Rust/Cargo is already installed")
        else:
            if click.confirm("\nRust is required for dora-rs. Install it now?", default=True):
                if not install_rust():
                    console.print("\n[yellow]Warning: Rust installation failed. You may need to install it manually.[/yellow]")
                    console.print("[dim]Visit: https://rustup.rs/[/dim]")
            else:
                console.print("[yellow]Skipped Rust installation[/yellow]")
    else:
        console.print("[dim]Skipped Rust installation check[/dim]")

    # Copy examples
    if not skip_examples:
        examples_copied = False

        # Try local package first
        try:
            import mofa
            package_root = Path(mofa.__file__).parent.parent
            source_agents = package_root / "agents"
            source_flows = package_root / "flows"

            if source_agents.exists() and source_flows.exists():
                examples_copied = copy_examples(target_dir, source_agents, source_flows)
            else:
                console.print("[yellow]⚠[/yellow]  Example agents/flows not found in installed package")
        except Exception as e:
            console.print(f"[yellow]Warning: Error locating local examples: {e}[/yellow]")

        # Fallback to GitHub if needed
        if not examples_copied:
            examples_copied = download_examples_from_github(target_dir)

        if not examples_copied:
            console.print("[red]Failed to copy examples from both local package and GitHub[/red]")
            console.print("[dim]You can manually download examples from: https://github.com/mofa-org/mofa[/dim]")

    # Create .env file
    create_env_file(target_dir)

    # Final message
    console.print(Panel.fit(
        "[bold green]✓ Initialization complete![/bold green]\n\n"
        f"Your MoFA workspace is ready at:\n[cyan]{target_dir}[/cyan]\n\n"
        "Next steps:\n"
        "  1. cd into your workspace directory\n"
        "  2. Run [bold]mofa list agents[/bold] to see available agents\n"
        "  3. Run [bold]mofa run-flow flows/example.yml[/bold] to test\n",
        border_style="green"
    ))

    # Remind about cargo env if rust was just installed
    if not skip_rust and not check_rust_installed():
        console.print("\n[yellow]Note:[/yellow] If Rust was just installed, run:")
        console.print("[bold]source $HOME/.cargo/env[/bold]\n")
