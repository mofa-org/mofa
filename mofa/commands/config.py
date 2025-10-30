"""
MoFA config command - Manage configuration
"""

import os
import click
from mofa import agents_dir_path, flows_dir_path, project_root


def register_config_commands(cli_group):
    """Register config command group to the main CLI"""

    @cli_group.group(invoke_without_command=True)
    @click.pass_context
    def config(ctx):
        """Manage MoFA configuration"""
        if ctx.invoked_subcommand is None:
            # No subcommand, run TUI
            ctx.invoke(tui)

    @config.command()
    def show():
        """Display current configuration"""
        click.echo("Current configuration:")
        click.echo(
            f"  OPENAI_API_KEY: {'***' if os.getenv('OPENAI_API_KEY') else '(not set)'}"
        )
        click.echo(f"  OPENAI_API_BASE: {os.getenv('OPENAI_API_BASE', '(default)')}")
        click.echo(
            f"  MOFA_VIBE_MODEL: {os.getenv('MOFA_VIBE_MODEL', 'gpt-4o-mini (default)')}"
        )
        click.echo(
            f"  MOFA_VIBE_MAX_ROUNDS: {os.getenv('MOFA_VIBE_MAX_ROUNDS', '100 (default)')}"
        )
        click.echo(
            f"  MOFA_VIBE_AGENTS_OUTPUT: {os.getenv('MOFA_VIBE_AGENTS_OUTPUT', agents_dir_path + ' (default)')}"
        )
        click.echo(
            f"  MOFA_VIBE_FLOWS_OUTPUT: {os.getenv('MOFA_VIBE_FLOWS_OUTPUT', flows_dir_path + ' (default)')}"
        )
        click.echo(f"  MOFA_AGENTS_DIR: {agents_dir_path}")
        click.echo(f"  MOFA_FLOWS_DIR: {flows_dir_path}")
        click.echo(f"  MOFA_HUB_URL: {os.getenv('MOFA_HUB_URL', '(default)')}")

    @config.command(name="set")
    @click.argument("key", required=True)
    @click.argument("value", required=True)
    def set_config(key, value):
        """Set a configuration value in .env"""
        env_file = os.path.join(project_root, ".env")

        # Read existing .env
        lines = []
        key_found = False

        if os.path.exists(env_file):
            with open(env_file, "r") as f:
                lines = f.readlines()

            # Update existing key
            for i, line in enumerate(lines):
                if line.strip().startswith(f"{key}="):
                    lines[i] = f"{key}={value}\n"
                    key_found = True
                    break

        # Add new key if not found
        if not key_found:
            lines.append(f"{key}={value}\n")

        # Write back
        with open(env_file, "w") as f:
            f.writelines(lines)

        click.echo(f"Set {key}={value}")
        click.echo(f"Updated {env_file}")

    @config.command()
    def tui():
        """Open TUI configuration interface"""
        env_file = os.path.join(project_root, ".env")

        while True:
            # Load current config
            config_values = {}
            if os.path.exists(env_file):
                with open(env_file, "r") as f:
                    for line in f:
                        line = line.strip()
                        if line and not line.startswith("#") and "=" in line:
                            key, value = line.split("=", 1)
                            config_values[key] = value

            # Display current configuration
            click.echo("\n" + "=" * 50)
            click.echo("           MoFA Configuration")
            click.echo("=" * 50 + "\n")

            config_items = [
                ("OPENAI_API_KEY", "OpenAI API Key"),
                ("OPENAI_API_BASE", "API Endpoint (Base URL)"),
                ("MOFA_VIBE_MODEL", "Default Vibe Model"),
                ("MOFA_VIBE_MAX_ROUNDS", "Vibe Max Optimization Rounds"),
                ("MOFA_VIBE_AGENTS_OUTPUT", "Vibe Agents Output Directory"),
                ("MOFA_VIBE_FLOWS_OUTPUT", "Vibe Flows Output Directory"),
                ("MOFA_AGENTS_DIR", "Agents Directory"),
                ("MOFA_FLOWS_DIR", "Flows Directory"),
                ("MOFA_HUB_URL", "Hub URL"),
            ]

            for i, (key, label) in enumerate(config_items, 1):
                current = config_values.get(key, "")
                if "KEY" in key and current:
                    display = "***" + current[-4:] if len(current) > 4 else "***"
                else:
                    display = current or "(not set)"
                click.echo(f"  {i}. {label}: {display}")

            click.echo(f"\n  r. Reset all mofa settings")
            click.echo(f"  q. Quit\n")

            choice = click.prompt("Select option", type=str, default="q")

            if choice.lower() == "q":
                click.echo("\nConfiguration saved!")
                break

            elif choice.lower() == "r":
                if click.confirm("\nAre you sure you want to reset all mofa settings?"):
                    if os.path.exists(env_file):
                        import shutil

                        backup_file = env_file + ".backup"
                        shutil.copy(env_file, backup_file)
                        click.echo(f"Backup saved to {backup_file}")

                        lines = []
                        with open(env_file, "r") as f:
                            for line in f:
                                if not line.strip().startswith(("MOFA_", "# mofa")):
                                    lines.append(line)

                        with open(env_file, "w") as f:
                            f.writelines(lines)

                        click.echo("Reset mofa configuration")

            elif choice.isdigit() and 1 <= int(choice) <= len(config_items):
                key, label = config_items[int(choice) - 1]
                current = config_values.get(key, "")

                new_value = click.prompt(
                    f"\nEnter new value for {label}", default=current, show_default=True
                )

                if new_value:
                    lines = []
                    key_found = False

                    if os.path.exists(env_file):
                        with open(env_file, "r") as f:
                            lines = f.readlines()

                        for i, line in enumerate(lines):
                            if line.strip().startswith(f"{key}="):
                                lines[i] = f"{key}={new_value}\n"
                                key_found = True
                                break

                    if not key_found:
                        lines.append(f"{key}={new_value}\n")

                    with open(env_file, "w") as f:
                        f.writelines(lines)

                    click.echo(f"Updated {key}")
            else:
                click.echo("Invalid option")

    @config.command()
    def reset():
        """Reset configuration to defaults"""
        if click.confirm("Are you sure you want to reset configuration to defaults?"):
            env_file = os.path.join(project_root, ".env")

            if os.path.exists(env_file):
                # Backup
                import shutil

                backup_file = env_file + ".backup"
                shutil.copy(env_file, backup_file)
                click.echo(f"Backup saved to {backup_file}")

                # Remove mofa-specific keys
                lines = []
                with open(env_file, "r") as f:
                    for line in f:
                        if not line.strip().startswith(("MOFA_", "# mofa")):
                            lines.append(line)

                with open(env_file, "w") as f:
                    f.writelines(lines)

                click.echo("Reset mofa configuration to defaults")
            else:
                click.echo("No .env file found")
