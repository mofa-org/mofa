import os
import shutil
import time
import uuid
import shlex
import subprocess
import tempfile
from pathlib import Path
from typing import List, Optional
from mofa import agents_dir_path, flows_dir_path, cli_dir_path

import click
import sys
from mofa.debug.actor import execute_unit_tests
from mofa.debug.gen_reporter import generate_test_report
from mofa.debug.iteractive import collect_interactive_input
from mofa.debug.load_node import load_node_module
from mofa.debug.parse_test_case import parse_test_cases
from mofa.utils.files.dir import get_subdirectories
from mofa.utils.files.read import read_yaml
from mofa.utils.process.util import (
    stop_process,
    stop_dora_dataflow,
    destroy_dora_daemon,
)
from mofa.registry import HubClient
from mofa.commands.init import init_command
from mofa.commands.run_flow import run_flow
from mofa.commands.vibe import register_vibe_commands
from mofa.commands.search import register_search_commands
from mofa.commands.config import register_config_commands

import cookiecutter
from cookiecutter.main import cookiecutter


class OrderedGroup(click.Group):
    def list_commands(self, ctx):
        return [
            "init",
            "run-flow",
            "run-node",
            "unit-test",
            "create-node",
            "vibe",
            "list",
            "search",
            "download",
            "config",
        ]

    def format_help(self, ctx, formatter):
        """Custom help formatter with usage hints"""
        # Call parent to get standard formatting
        super().format_help(ctx, formatter)

        # Check if full mode is requested
        show_full = ctx.obj.get("show_full", False) if ctx.obj else False

        if show_full:
            # Add full command reference
            formatter.write_paragraph()
            formatter.write_text("Command Reference (All Available Commands):")
            with formatter.indentation():
                formatter.write_text("\nSetup:")
                formatter.write_text(
                    "  mofa init [DIR]                             Initialize workspace"
                )
                formatter.write_text(
                    "  mofa init --skip-rust                       Skip Rust installation"
                )

                formatter.write_text("\nCore Commands:")
                formatter.write_text(
                    "  mofa run-flow <dataflow.yml>                Run a dataflow"
                )
                formatter.write_text(
                    "  mofa create-node                            Create node (TUI)"
                )
                formatter.write_text(
                    "  mofa unit-test <path> [test.yml]          Debug an agent"
                )
                formatter.write_text(
                    "  mofa unit-test <path> --interactive       Debug interactively"
                )

                formatter.write_text("\nAI Generation:")
                formatter.write_text(
                    "  mofa vibe                                   AI generator (TUI)"
                )
                formatter.write_text(
                    "  mofa vibe agent [--llm MODEL] [--max-rounds N] [-o DIR]"
                )
                formatter.write_text("  mofa vibe flow [--llm MODEL] [-o DIR]")

                formatter.write_text("\nList & Browse:")
                formatter.write_text(
                    "  mofa list                                   List all (TUI)"
                )
                formatter.write_text(
                    "  mofa list agents [--remote|--all]           List agents"
                )
                formatter.write_text(
                    "  mofa list flows [--remote|--all]            List flows"
                )

                formatter.write_text("\nSearch:")
                formatter.write_text(
                    "  mofa search                                 Search + download (TUI)"
                )
                formatter.write_text("  mofa search agent <keyword> [--local|--remote]")
                formatter.write_text("  mofa search flow <keyword> [--local|--remote]")

                formatter.write_text("\nDownload:")
                formatter.write_text(
                    "  mofa download                               Download with search (TUI)"
                )
                formatter.write_text(
                    "  mofa download agent <name> [-o DIR]         Download agent"
                )
                formatter.write_text(
                    "  mofa download flow <name> [-o DIR]          Download flow"
                )

                formatter.write_text("\nConfiguration:")
                formatter.write_text(
                    "  mofa config                                 Config manager (TUI)"
                )
                formatter.write_text(
                    "  mofa config show                            Show current config"
                )
                formatter.write_text(
                    "  mofa config set <KEY> <VALUE>               Set config value"
                )
                formatter.write_text(
                    "  mofa config reset                           Reset to defaults"
                )

        # Add usage tips
        formatter.write_paragraph()
        formatter.write_text("Tips:")
        with formatter.indentation():
            formatter.write_text("* Most commands support both TUI and CLI modes")
            formatter.write_text(
                "* Run without args for interactive mode (e.g., 'mofa list')"
            )
            if not show_full:
                formatter.write_text(
                    "* Use 'mofa --full' or 'mofa -v' to see all available commands"
                )
            formatter.write_text(
                "* Use --help on any command for details (e.g., 'mofa search --help')"
            )


@click.group(cls=OrderedGroup, invoke_without_command=True)
@click.option("--full", "-v", is_flag=True, help="Show full command reference")
@click.pass_context
def mofa_cli_group(ctx, full):
    """Main CLI for MoFA"""
    # Store full flag in context for help formatting
    ctx.ensure_object(dict)
    ctx.obj["show_full"] = full

    # If no subcommand is provided, show help
    if ctx.invoked_subcommand is None:
        click.echo(ctx.get_help())
        ctx.exit()


# ============ Init Command ============
mofa_cli_group.add_command(init_command)


# ============ Run Flow Command ============
@mofa_cli_group.command(name="run-flow")
@click.argument("dataflow_file", required=True)
@click.option("--vibe-test", is_flag=True, hidden=True, help="Vibe test mode with automated decisions")
def run_flow_command(dataflow_file: str, vibe_test: bool):
    """Use run <path-to-dataflow.yml> to run in venv"""
    run_flow(dataflow_file, vibe_test_mode=vibe_test)


# ============ List Command Group ============
@mofa_cli_group.group(invoke_without_command=True)
@click.pass_context
def list(ctx):
    """List all nodes and flows (local and remote)"""
    if ctx.invoked_subcommand is None:
        # No subcommand, show everything
        _list_all()


def _list_all():
    """List all nodes and flows (local and remote)"""
    # List agents
    local_agents = set()
    if os.path.exists(agents_dir_path):
        local_agents = set(get_subdirectories(agents_dir_path))

    if local_agents:
        click.echo(f"Local agents ({len(local_agents)}):")
        for name in sorted(local_agents):
            click.echo(f"  - {name}")
    else:
        click.echo(f"Local agents (0):")
        click.echo(f"  No agents found in: {agents_dir_path}")

    try:
        hub = HubClient()
        remote_agents = hub.list_agents()
        remote_only_agents = [
            a for a in remote_agents if a.get("name") not in local_agents
        ]
        if remote_only_agents:
            click.echo(f"\nRemote agents ({len(remote_only_agents)}):")
            for agent in remote_only_agents:
                name = agent.get("name", "unknown")
                click.echo(f"  - {name}")
    except Exception as e:
        click.echo(f"\nError fetching remote agents: {e}", err=True)

    # List flows
    click.echo()
    local_flows = set()
    if os.path.exists(flows_dir_path):
        local_flows = set(get_subdirectories(flows_dir_path))

    if local_flows:
        click.echo(f"Local flows ({len(local_flows)}):")
        for name in sorted(local_flows):
            click.echo(f"  - {name}")
    else:
        click.echo(f"Local flows (0):")
        click.echo(f"  No flows found in: {flows_dir_path}")

    try:
        hub = HubClient()
        remote_flows = hub.list_flows()
        remote_only_flows = [
            f for f in remote_flows if f.get("name") not in local_flows
        ]
        if remote_only_flows:
            click.echo(f"\nRemote flows ({len(remote_only_flows)}):")
            for flow in remote_only_flows:
                name = flow.get("name", "unknown")
                click.echo(f"  - {name}")
    except Exception as e:
        click.echo(f"\nError fetching remote flows: {e}", err=True)


@list.command()
@click.option("--remote", is_flag=True, help="List remote hub agents")
@click.option(
    "--all", "show_all", is_flag=True, help="List both local and remote agents"
)
def agents(remote, show_all):
    """List agents (local by default)"""

    # Get local agents
    local_agents = set(get_subdirectories(agents_dir_path))

    if remote:
        # Remote only
        try:
            hub = HubClient()
            remote_agents = hub.list_agents()
            click.echo(f"Remote agents ({len(remote_agents)}):")
            for agent in remote_agents:
                name = agent.get("name", "unknown")
                desc = agent.get("description", "")
                tags = ", ".join(agent.get("tags", []))
                click.echo(f"  - {name}")
                if desc:
                    click.echo(f"    {desc}")
                if tags:
                    click.echo(f"    Tags: {tags}")
        except Exception as e:
            click.echo(f"Error fetching remote agents: {e}", err=True)
        return

    if show_all:
        # Both local and remote
        click.echo(f"Local agents ({len(local_agents)}):")
        for name in sorted(local_agents):
            click.echo(f"  [local] {name}")

        try:
            hub = HubClient()
            remote_agents = hub.list_agents()
            remote_only = [
                a for a in remote_agents if a.get("name") not in local_agents
            ]
            if remote_only:
                click.echo(f"\nRemote agents ({len(remote_only)}):")
                for agent in remote_only:
                    name = agent.get("name", "unknown")
                    desc = agent.get("description", "")
                    click.echo(f"  [hub] {name}")
                    if desc:
                        click.echo(f"        {desc}")
        except Exception as e:
            click.echo(f"\nError fetching remote agents: {e}", err=True)
        return

    # Local only (default)
    click.echo(f"Local agents ({len(local_agents)}):")
    for name in sorted(local_agents):
        click.echo(f"  - {name}")


@list.command()
@click.option("--remote", is_flag=True, help="List remote hub flows")
@click.option(
    "--all", "show_all", is_flag=True, help="List both local and remote flows"
)
def flows(remote, show_all):
    """List flows (local by default)"""

    # Get local flows
    local_flows = set(get_subdirectories(flows_dir_path))

    if remote:
        # Remote only
        try:
            hub = HubClient()
            remote_flows = hub.list_flows()
            click.echo(f"Remote flows ({len(remote_flows)}):")
            for flow in remote_flows:
                name = flow.get("name", "unknown")
                desc = flow.get("description", "")
                agents = ", ".join(flow.get("agents", []))
                click.echo(f"  - {name}")
                if desc:
                    click.echo(f"    {desc}")
                if agents:
                    click.echo(f"    Agents: {agents}")
        except Exception as e:
            click.echo(f"Error fetching remote flows: {e}", err=True)
        return

    if show_all:
        # Both local and remote
        click.echo(f"Local flows ({len(local_flows)}):")
        for name in sorted(local_flows):
            click.echo(f"  [local] {name}")

        try:
            hub = HubClient()
            remote_flows = hub.list_flows()
            remote_only = [f for f in remote_flows if f.get("name") not in local_flows]
            if remote_only:
                click.echo(f"\nRemote flows ({len(remote_only)}):")
                for flow in remote_only:
                    name = flow.get("name", "unknown")
                    desc = flow.get("description", "")
                    click.echo(f"  [hub] {name}")
                    if desc:
                        click.echo(f"        {desc}")
        except Exception as e:
            click.echo(f"\nError fetching remote flows: {e}", err=True)
        return

    # Local only (default)
    click.echo(f"Local flows ({len(local_flows)}):")
    for name in sorted(local_flows):
        click.echo(f"  - {name}")


# Legacy command (deprecated)
@mofa_cli_group.command(hidden=True)
def agent_list():
    """[Deprecated] Use 'mofa list agents' instead"""
    click.echo("Warning: 'agent-list' is deprecated, use 'mofa list agents' instead")
    agent_names = get_subdirectories(agents_dir_path)
    for name in agent_names:
        click.echo(f"  - {name}")


@mofa_cli_group.command(name="run-node")
@click.argument("node_folder_path", type=click.Path(exists=True))
def run_agent(node_folder_path):
    """Provide values for input parameters to run a node"""
    # 1. dynamically load the node module
    node_module = load_node_module(node_folder_path)
    # 2. interactively collect user input
    test_cases = collect_interactive_input(unit_test=False)
    # 3. execute tests and print outputs
    execute_unit_tests(node_module, test_cases, unit_test=False)


@mofa_cli_group.command(name="unit-test")
@click.argument("node_folder_path", type=click.Path(exists=True))
@click.argument("test_case_yml", type=click.Path(exists=True), required=False)
@click.option("--interactive", is_flag=True, help="Enable interactive input mode")
def debug_agent(node_folder_path, test_case_yml, interactive):
    """Run unit tests for a single node"""
    # 1. dynamically load the node module
    node_module = load_node_module(node_folder_path)

    # 2. parse the test cases from the YAML file
    if interactive:
        # Check for conflicting parameters
        if test_case_yml:
            raise click.BadParameter(
                "Interactive mode does not require YAML file, please remove test_case_yml parameter"
            )
        test_cases = collect_interactive_input()  # Interactively collect test cases
    else:
        # Traditional mode: YAML file required
        if not test_case_yml:
            raise click.BadParameter("Non-interactive mode requires YAML file path")
        test_cases = parse_test_cases(test_case_yml)  # Parse test cases from YAML
    # print("==================================")
    # print("Node module loaded:", node_module)
    # print("==================================")
    # print("Test cases loaded:", test_cases)
    # print("==================================")

    # 3. execute tests and generate report
    results = execute_unit_tests(node_module, test_cases)

    # 4. generate and print the test report
    generate_test_report(results)


# ============ Download Command Group ============
@mofa_cli_group.group(invoke_without_command=True)
@click.pass_context
def download(ctx):
    """Download nodes and flows from hub"""
    if ctx.invoked_subcommand is None:
        # No subcommand, run download TUI
        _run_download_tui()


def _run_download_tui():
    """Run interactive download TUI"""
    click.echo("\n" + "=" * 50)
    click.echo("           MoFA Download")
    click.echo("=" * 50 + "\n")

    # Ask what to download
    download_type = click.prompt(
        "What to download? (1=agent, 2=flow, q=quit)", type=str, default="1"
    )

    if download_type.lower() == "q":
        return

    # Search first
    keyword = click.prompt(
        "Search keyword (or press Enter to list all)", type=str, default=""
    )

    hub = HubClient()

    try:
        if download_type == "1":
            # Download agent
            if keyword:
                agents = hub.search_agents(keyword)
                click.echo(f"\nFound {len(agents)} agent(s) matching '{keyword}':")
            else:
                agents = hub.list_agents()
                click.echo(f"\nAvailable agents ({len(agents)}):")

            if not agents:
                click.echo("No agents found")
                return

            for idx, agent in enumerate(agents, 1):
                name = agent.get("name", "unknown")
                desc = agent.get("description", "")
                click.echo(f"  {idx}. {name}")
                if desc:
                    click.echo(f"     {desc}")

            choice = click.prompt("\nSelect agent number (or 'q' to quit)", type=str)
            if choice.lower() == "q":
                return

            try:
                agent_idx = int(choice) - 1
                if 0 <= agent_idx < len(agents):
                    selected_agent = agents[agent_idx]["name"]
                    output_dir = click.prompt(
                        "Output directory", default=agents_dir_path
                    )

                    click.echo(f"\nDownloading '{selected_agent}'...")
                    hub.download_agent(selected_agent, output_dir)
                    click.echo(
                        f"Successfully downloaded to {output_dir}/{selected_agent}"
                    )
                else:
                    click.echo("Invalid selection")
            except ValueError:
                click.echo("Invalid input")
            except Exception as e:
                click.echo(f"Error: {e}", err=True)

        elif download_type == "2":
            # Download flow
            if keyword:
                flows = hub.search_flows(keyword)
                click.echo(f"\nFound {len(flows)} flow(s) matching '{keyword}':")
            else:
                flows = hub.list_flows()
                click.echo(f"\nAvailable flows ({len(flows)}):")

            if not flows:
                click.echo("No flows found")
                return

            for idx, flow in enumerate(flows, 1):
                name = flow.get("name", "unknown")
                desc = flow.get("description", "")
                click.echo(f"  {idx}. {name}")
                if desc:
                    click.echo(f"     {desc}")

            choice = click.prompt("\nSelect flow number (or 'q' to quit)", type=str)
            if choice.lower() == "q":
                return

            try:
                flow_idx = int(choice) - 1
                if 0 <= flow_idx < len(flows):
                    selected_flow = flows[flow_idx]["name"]
                    output_dir = click.prompt(
                        "Output directory", default=flows_dir_path
                    )

                    click.echo(f"\nDownloading '{selected_flow}'...")
                    hub.download_flow(selected_flow, output_dir)
                    click.echo(
                        f"Successfully downloaded to {output_dir}/{selected_flow}"
                    )
                else:
                    click.echo("Invalid selection")
            except ValueError:
                click.echo("Invalid input")
            except Exception as e:
                click.echo(f"Error: {e}", err=True)

    except Exception as e:
        click.echo(f"Error: {e}", err=True)


@download.command()
@click.argument("name", required=True)
@click.option(
    "--output", "-o", default=None, help="Output directory (default: ./agents)"
)
def agent(name, output):
    """Download an agent from remote hub"""
    output_dir = output or agents_dir_path

    click.echo(f"Downloading agent '{name}' from hub...")
    try:
        hub = HubClient()
        hub.download_agent(name, output_dir)
        click.echo(f"Successfully downloaded to {output_dir}/{name}")
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


@download.command()
@click.argument("name", required=True)
@click.option(
    "--output", "-o", default=None, help="Output directory (default: ./flows)"
)
def flow(name, output):
    """Download a flow from remote hub"""
    output_dir = output or flows_dir_path

    click.echo(f"Downloading flow '{name}' from hub...")
    try:
        hub = HubClient()
        hub.download_flow(name, output_dir)
        click.echo(f"Successfully downloaded to {output_dir}/{name}")
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


# ============ Helper: Check API Key ============


# ============ Create Node Command ============
@mofa_cli_group.command(name="create-node")
@click.option("--name", default=None, help="Node name")
@click.option("--version", default=None, help="Version of the new node")
@click.option("--output", default=None, help="Output directory (default: ./agents)")
@click.option("--authors", default=None, help="Authors")
@click.option("--description", default=None, help="Node description")
def create_node(name, version, output, authors, description):
    """Create a new node from template"""

    # Interactive TUI mode
    click.echo("\n" + "=" * 50)
    click.echo("         Create New MoFA Node")
    click.echo("=" * 50 + "\n")

    # Collect inputs
    node_name = name or click.prompt("Node name", type=str)
    node_version = version or click.prompt("Version", default="0.0.1")
    node_description = description or click.prompt(
        "Description", default=f"A {node_name} node"
    )
    node_authors = authors or click.prompt("Authors", default="MoFA Team")
    node_output = output or click.prompt("Output directory", default=agents_dir_path)

    # Confirm
    click.echo("\n" + "-" * 50)
    click.echo("Node Configuration:")
    click.echo(f"  Name: {node_name}")
    click.echo(f"  Version: {node_version}")
    click.echo(f"  Description: {node_description}")
    click.echo(f"  Authors: {node_authors}")
    click.echo(f"  Output: {node_output}")
    click.echo("-" * 50 + "\n")

    if not click.confirm("Create node?", default=True):
        click.echo("Cancelled")
        return

    # Create from template
    template_dir = os.path.join(cli_dir_path, "agent-template")

    # Ensure the template directory exists and contains cookiecutter.json
    if not os.path.exists(template_dir):
        click.echo(f"Error: Template directory not found: {template_dir}", err=True)
        return
    if not os.path.isfile(os.path.join(template_dir, "cookiecutter.json")):
        click.echo(
            f"Error: Template directory must contain cookiecutter.json", err=True
        )
        return

    # Use Cookiecutter to generate the new node from the template
    try:
        result_path = cookiecutter(
            template=template_dir,
            output_dir=node_output,
            no_input=True,
            extra_context={
                "user_agent_dir": node_name,
                "agent_name": node_name,
                "version": node_version,
                "description": node_description,
                "authors": node_authors,
            },
        )
        click.echo(f"\nSuccessfully created node: {result_path}")
        click.echo(f"\nNext steps:")
        click.echo(f"  1. cd {result_path}")
        click.echo(f"  2. Edit {node_name}/main.py to implement your node logic")
        click.echo(f"  3. Test with: mofa unit-test {result_path} tests/test_main.py")
    except Exception as e:
        click.echo(f"\nError: Failed to create node: {e}", err=True)
        import traceback

        traceback.print_exc()


# ============ Legacy Commands (Deprecated) ============
@mofa_cli_group.command(hidden=True)
@click.argument("node_folder_path", type=click.Path(exists=True))
@click.argument("test_case_yml", type=click.Path(exists=True), required=False)
@click.option(
    "--interactive",
    is_flag=True,
    help="Enable interactive input (no YAML file required)",
)
def debug(node_folder_path, test_case_yml, interactive):
    """[Deprecated] Use 'mofa unit-test' instead"""
    click.echo("Warning: 'debug' is deprecated, use 'mofa unit-test' instead")
    from click import Context

    ctx = Context(debug_agent)
    ctx.invoke(
        debug_agent,
        node_folder_path=node_folder_path,
        test_case_yml=test_case_yml,
        interactive=interactive,
    )


@mofa_cli_group.command(hidden=True)
@click.argument("agent_name", required=True)
@click.option("--version", default="0.0.1", help="Version of the new agent")
@click.option("--output", default=None, help="Output directory")
@click.option("--authors", default="Mofa Bot", help="Authors")
def new_agent(agent_name: str, version: str, output: str, authors: str):
    """[Deprecated] Use 'mofa create-node' instead"""
    click.echo("Warning: 'new-agent' is deprecated, use 'mofa create-node' instead")
    if output is None:
        output = agents_dir_path

    template_dir = os.path.join(cli_dir_path, "agent-template")
    if not os.path.exists(template_dir):
        click.echo(f"Template directory not found: {template_dir}")
        return
    if not os.path.isfile(os.path.join(template_dir, "cookiecutter.json")):
        click.echo(
            f"Template directory must contain a cookiecutter.json file: {template_dir}"
        )
        return

    try:
        cookiecutter(
            template=template_dir,
            output_dir=output,
            no_input=True,
            extra_context={
                "user_agent_dir": agent_name,
                "agent_name": agent_name,
                "version": version,
                "authors": authors,
            },
        )
        click.echo(f"Successfully created new agent in {output}{agent_name}")
    except Exception as e:
        click.echo(f"Failed to create new agent: {e}")


# ============ Register Vibe Commands ============
register_vibe_commands(mofa_cli_group)

# ============ Register Search Commands ============
register_search_commands(mofa_cli_group)

# ============ Register Config Commands ============
register_config_commands(mofa_cli_group)


if __name__ == "__main__":
    mofa_cli_group()
