"""
MoFA vibe command - Generate agents and flows using AI
"""
import os
import sys
import click
from mofa import agents_dir_path, flows_dir_path


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
            "What to generate? (1=agent, 2=flow, q=quit)", type=str, default="1"
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
        env_file = os.path.join(os.getcwd(), ".env")
        if os.path.exists(env_file):
            load_dotenv(env_file)

        if vibe_type == "1":
            # Generate agent
            click.echo("\nGenerating agent...")

            # Get saved config
            saved_model = os.getenv("MOFA_VIBE_MODEL", "gpt-4o-mini")

            llm = click.prompt("LLM model", default=saved_model)
            max_rounds = click.prompt(
                "Maximum optimization rounds (0 for unlimited)", default=100, type=int
            )
            output = click.prompt("Output directory", default=agents_dir_path)

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

            # Get saved config
            saved_model = os.getenv("MOFA_VIBE_MODEL", "gpt-4o-mini")

            llm = click.prompt("LLM model", default=saved_model)
            output = click.prompt("Output directory", default=flows_dir_path)

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

    @vibe.command()
    @click.option("--llm", default=None, help="LLM model to use (default: from config)")
    @click.option(
        "--max-rounds",
        default=100,
        help="Maximum optimization rounds (default: 100, use 0 for unlimited)",
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
        env_file = os.path.join(os.getcwd(), ".env")
        if os.path.exists(env_file):
            load_dotenv(env_file)

        # Check for API key and prompt user if not found
        api_key = _check_and_setup_api_key()
        if not api_key:
            click.echo("Cannot proceed without API key. Exiting...")
            sys.exit(1)

        # Use config defaults if not provided
        if llm is None:
            llm = os.getenv("MOFA_VIBE_MODEL", "gpt-4o-mini")
        if output is None:
            output = agents_dir_path

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
