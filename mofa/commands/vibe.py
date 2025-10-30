"""
MoFA vibe command - Generate agents and flows using AI
"""
import os
import sys
import click
from mofa import agents_dir_path, flows_dir_path, project_root


def _get_env_file_path():
    """Get the path to the .env file"""
    return os.path.join(project_root, ".env")


def _load_vibe_config():
    """Load vibe configuration from .env file"""
    env_file = _get_env_file_path()
    config = {
        "model": "gpt-4o-mini",
        "max_rounds": 100,
        "agents_output": agents_dir_path,
        "flows_output": flows_dir_path,
    }

    if os.path.exists(env_file):
        try:
            from dotenv import dotenv_values
            env_values = dotenv_values(env_file)

            if "MOFA_VIBE_MODEL" in env_values:
                config["model"] = env_values["MOFA_VIBE_MODEL"]
            if "MOFA_VIBE_MAX_ROUNDS" in env_values:
                try:
                    config["max_rounds"] = int(env_values["MOFA_VIBE_MAX_ROUNDS"])
                except ValueError:
                    pass
            if "MOFA_VIBE_AGENTS_OUTPUT" in env_values:
                config["agents_output"] = env_values["MOFA_VIBE_AGENTS_OUTPUT"]
            if "MOFA_VIBE_FLOWS_OUTPUT" in env_values:
                config["flows_output"] = env_values["MOFA_VIBE_FLOWS_OUTPUT"]
        except Exception:
            pass

    return config


def _save_vibe_config(model=None, max_rounds=None, agents_output=None, flows_output=None):
    """Save vibe configuration to .env file"""
    env_file = _get_env_file_path()

    # Read existing .env content
    lines = []
    if os.path.exists(env_file):
        with open(env_file, 'r') as f:
            lines = f.readlines()

    # Track which configs were updated
    updated = {
        "MOFA_VIBE_MODEL": False,
        "MOFA_VIBE_MAX_ROUNDS": False,
        "MOFA_VIBE_AGENTS_OUTPUT": False,
        "MOFA_VIBE_FLOWS_OUTPUT": False,
    }

    # Update existing lines
    for i, line in enumerate(lines):
        stripped = line.strip()
        if model and stripped.startswith("MOFA_VIBE_MODEL="):
            lines[i] = f"MOFA_VIBE_MODEL={model}\n"
            updated["MOFA_VIBE_MODEL"] = True
        elif max_rounds is not None and stripped.startswith("MOFA_VIBE_MAX_ROUNDS="):
            lines[i] = f"MOFA_VIBE_MAX_ROUNDS={max_rounds}\n"
            updated["MOFA_VIBE_MAX_ROUNDS"] = True
        elif agents_output and stripped.startswith("MOFA_VIBE_AGENTS_OUTPUT="):
            lines[i] = f"MOFA_VIBE_AGENTS_OUTPUT={agents_output}\n"
            updated["MOFA_VIBE_AGENTS_OUTPUT"] = True
        elif flows_output and stripped.startswith("MOFA_VIBE_FLOWS_OUTPUT="):
            lines[i] = f"MOFA_VIBE_FLOWS_OUTPUT={flows_output}\n"
            updated["MOFA_VIBE_FLOWS_OUTPUT"] = True

    # Add new configs that weren't found
    new_configs = []
    if model and not updated["MOFA_VIBE_MODEL"]:
        new_configs.append(f"MOFA_VIBE_MODEL={model}\n")
    if max_rounds is not None and not updated["MOFA_VIBE_MAX_ROUNDS"]:
        new_configs.append(f"MOFA_VIBE_MAX_ROUNDS={max_rounds}\n")
    if agents_output and not updated["MOFA_VIBE_AGENTS_OUTPUT"]:
        new_configs.append(f"MOFA_VIBE_AGENTS_OUTPUT={agents_output}\n")
    if flows_output and not updated["MOFA_VIBE_FLOWS_OUTPUT"]:
        new_configs.append(f"MOFA_VIBE_FLOWS_OUTPUT={flows_output}\n")

    if new_configs:
        # Add a section header if adding new configs
        if lines and not lines[-1].endswith('\n'):
            lines.append('\n')
        if not any('# MoFA Vibe Configuration' in line for line in lines):
            lines.append('\n# MoFA Vibe Configuration\n')
        lines.extend(new_configs)

    # Write back to file
    with open(env_file, 'w') as f:
        f.writelines(lines)


def _check_and_setup_api_key():
    """Check for API key and prompt user if not found"""

    # Check if API key is set
    api_key = os.getenv('OPENAI_API_KEY')

    if not api_key:
        click.echo("\n[WARNING] OPENAI_API_KEY not found in environment.")
        click.echo("\nYou need an OpenAI API key to use Vibe.")
        click.echo("Get your API key from: https://platform.openai.com/api-keys\n")

        if click.confirm("Do you want to set it now?", default=True):
            api_key = click.prompt("Enter your OpenAI API key", hide_input=True)

            # Ask if they want to save it
            if click.confirm("\nSave to .env file?", default=True):
                env_file = os.path.join(os.getcwd(), '.env')

                # Append to .env or create new one
                with open(env_file, 'a') as f:
                    f.write(f"\nOPENAI_API_KEY={api_key}\n")

                click.echo(f"✓ API key saved to {env_file}")

                # Set it in current environment
                os.environ['OPENAI_API_KEY'] = api_key
            else:
                # Just set it for this session
                os.environ['OPENAI_API_KEY'] = api_key
                click.echo("✓ API key set for this session only")
        else:
            return None

    return api_key


def register_vibe_commands(cli_group):
    """Register vibe command group to the main CLI"""

    @cli_group.group(invoke_without_command=True)
    @click.pass_context
    def vibe(ctx):
        """Generate agents and flows using AI"""
        if ctx.invoked_subcommand is None:
            # No subcommand, run vibe TUI
            _run_vibe_tui()

    def _run_vibe_tui():
        """Run interactive vibe TUI"""
        click.echo("\n" + "=" * 50)
        click.echo("           MoFA Vibe - Agent & Flow Generator")
        click.echo("=" * 50 + "\n")

        # Check API key first
        api_key = _check_and_setup_api_key()
        if not api_key:
            click.echo("Cannot proceed without API key. Exiting...")
            return

        # Ask what to generate
        vibe_type = click.prompt(
            "What to generate? (1=agent, 2=flow, 3=yolo, q=quit)", type=str, default="1"
        )

        if vibe_type.lower() == "q":
            return

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
        env_file = os.path.join(project_root, ".env")
        if os.path.exists(env_file):
            load_dotenv(env_file)

        if vibe_type == "1":
            # Generate agent
            click.echo("\nGenerating agent...")

            # Load saved config
            saved_config = _load_vibe_config()

            llm = click.prompt("LLM model", default=saved_config["model"])
            max_rounds = click.prompt(
                "Maximum optimization rounds (0 for unlimited)", default=saved_config["max_rounds"], type=int
            )
            output = click.prompt("Output directory", default=saved_config["agents_output"])

            # Save the config for next time
            _save_vibe_config(model=llm, max_rounds=max_rounds, agents_output=output)

            config = VibeConfig(
                llm_model=llm,
                max_optimization_rounds=max_rounds,
                output_dir=output,
                llm_api_key=api_key,
            )

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
            except Exception as e:
                click.echo(f"\nERROR: {e}")
                import traceback

                traceback.print_exc()
                sys.exit(1)

        elif vibe_type == "2":
            # Generate flow
            click.echo("\nGenerating flow...")

            # Load saved config
            saved_config = _load_vibe_config()

            llm = click.prompt("LLM model", default=saved_config["model"])
            output = click.prompt("Output directory", default=saved_config["flows_output"])

            # Save the config for next time
            _save_vibe_config(model=llm, flows_output=output)

            # Get flow requirement
            requirement = click.prompt("\nDescribe the flow (what it should do)")

            try:
                from mofa.vibe.flow_generator import FlowGenerator

                # Initialize flow generator
                generator = FlowGenerator(
                    agents_dir=agents_dir_path,
                    flows_dir=output,
                    llm_model=llm,
                    api_key=api_key,
                )

                # Generate flow
                click.echo("\nScanning agents and generating flow...")
                flow_path = generator.generate_flow(requirement)

                click.echo(f"\n[SUCCESS] Flow created at: {flow_path}")
                click.echo(f"\nNext steps:")
                click.echo(f"  1. Review the flow: {flow_path}")
                click.echo(f"  2. Run: mofa run-flow {flow_path}/*_dataflow.yml")

            except Exception as e:
                click.echo(f"\n[ERROR] Flow generation failed: {e}")
                import traceback

                traceback.print_exc()
                sys.exit(1)

        elif vibe_type == "3":
            # YOLO mode - fast multi-node generation
            click.echo("\nYOLO Mode - Fast multi-node generation\n")

            # Only ask for requirement
            requirement = click.prompt("Describe what the flow should do")

            # Load saved config
            saved_config = _load_vibe_config()

            try:
                from mofa.vibe.yolo_engine import YoloEngine

                # Initialize YOLO engine with minimal config
                engine = YoloEngine(
                    requirement=requirement,
                    llm_model=saved_config["model"],
                    api_key=api_key,
                    agents_output=saved_config["agents_output"],
                    flows_output=saved_config["flows_output"],
                )

                # Run YOLO generation
                result = engine.run()

                if result:
                    click.echo(f"\n[SUCCESS] Generated at: {result['flow_path']}")
                    click.echo(f"Agents: {', '.join(result['agents'])}")
                    sys.exit(0)
                else:
                    sys.exit(1)

            except Exception as e:
                click.echo(f"\n[ERROR] YOLO generation failed: {e}")
                import traceback

                traceback.print_exc()
                sys.exit(1)

    @vibe.command()
    @click.option("--llm", default=None, help="LLM model to use (default: from config)")
    @click.option(
        "--max-rounds",
        default=None,
        type=int,
        help="Maximum optimization rounds (default: from config, use 0 for unlimited)",
    )
    @click.option(
        "--output", "-o", default=None, help="Output directory (default: from config)"
    )
    def agent(llm, max_rounds, output):
        """Generate an agent from natural language description

        Generates MoFA agents from natural language descriptions,
        automatically creates test cases, and iteratively optimizes the code
        until all tests pass.

        Usage:
            mofa vibe agent
            mofa vibe agent --llm gpt-4 --max-rounds 3
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
        env_file = os.path.join(project_root, ".env")
        if os.path.exists(env_file):
            load_dotenv(env_file)

        # Check for API key and prompt user if not found
        api_key = _check_and_setup_api_key()
        if not api_key:
            click.echo("Cannot proceed without API key. Exiting...")
            sys.exit(1)

        # Load saved config for defaults
        saved_config = _load_vibe_config()

        # Use config defaults if not provided
        if llm is None:
            llm = saved_config["model"]
        if max_rounds is None:
            max_rounds = saved_config["max_rounds"]
        if output is None:
            output = saved_config["agents_output"]

        # Save the config for next time
        _save_vibe_config(model=llm, max_rounds=max_rounds, agents_output=output)

        # Create config
        config = VibeConfig(
            llm_model=llm,
            max_optimization_rounds=max_rounds,
            output_dir=output,
            llm_api_key=api_key,
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
                click.echo(
                    "Please set OPENAI_API_KEY environment variable or re-run mofa vibe"
                )
                sys.exit(1)
            raise
        except Exception as e:
            click.echo(f"\nERROR: {e}")
            import traceback

            traceback.print_exc()
            sys.exit(1)

    @vibe.command()
    @click.option("--llm", default="gpt-4", help="LLM model to use (default: gpt-4)")
    @click.option(
        "--output", "-o", default="./flows", help="Output directory (default: ./flows)"
    )
    def flow(llm, output):
        """Generate a dataflow from natural language description

        Usage:
            mofa vibe flow
            mofa vibe flow --llm gpt-4
        """
        click.echo("Vibe flow generation (not implemented yet)")
        click.echo(f"LLM: {llm}")
        click.echo(f"Output: {output}")
        # TODO: Implement flow generation
