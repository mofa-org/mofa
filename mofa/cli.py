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

import cookiecutter
from cookiecutter.main import cookiecutter


class OrderedGroup(click.Group):
    def list_commands(self, ctx):
        return [
            "run-flow",
            "create-agent",
            "debug-agent",
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
                formatter.write_text("\nCore Commands:")
                formatter.write_text(
                    "  mofa run-flow <dataflow.yml>                Run a dataflow"
                )
                formatter.write_text(
                    "  mofa create-agent                           Create agent (TUI)"
                )
                formatter.write_text(
                    "  mofa debug-agent <path> [test.yml]          Debug an agent"
                )
                formatter.write_text(
                    "  mofa debug-agent <path> --interactive       Debug interactively"
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
    """Main CLI for MAE"""
    # Store full flag in context for help formatting
    ctx.ensure_object(dict)
    ctx.obj["show_full"] = full

    # If no subcommand is provided, show help
    if ctx.invoked_subcommand is None:
        click.echo(ctx.get_help())
        ctx.exit()


# ============ Run Flow Command ============
@mofa_cli_group.command(name="run-flow")
@click.argument("dataflow_file", required=True)
def run_flow_command(dataflow_file: str):
    """Use run <path-to-dataflow.yml> to run in venv."""
    _run_flow_impl(dataflow_file)


# ============ List Command Group ============
@mofa_cli_group.group(invoke_without_command=True)
@click.pass_context
def list(ctx):
    """List all agents and flow (local and remote)"""
    if ctx.invoked_subcommand is None:
        # No subcommand, show everything
        _list_all()


def _list_all():
    """List all agents and flows (local and remote)"""
    # List agents
    local_agents = set(get_subdirectories(agents_dir_path))
    click.echo(f"Local agents ({len(local_agents)}):")
    for name in sorted(local_agents):
        click.echo(f"  - {name}")

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
    local_flows = set(get_subdirectories(flows_dir_path))
    click.echo(f"Local flows ({len(local_flows)}):")
    for name in sorted(local_flows):
        click.echo(f"  - {name}")

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


@mofa_cli_group.command(name="debug-agent")
@click.argument("node_folder_path", type=click.Path(exists=True))
@click.argument("test_case_yml", type=click.Path(exists=True), required=False)
@click.option("--interactive", is_flag=True, help="Enable interactive input mode")
def debug_agent(node_folder_path, test_case_yml, interactive):
    """Run unit tests for a single agent"""
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


# ============ Search Command Group ============
@mofa_cli_group.group(invoke_without_command=True)
@click.pass_context
def search(ctx):
    """Search for agents and flows"""
    if ctx.invoked_subcommand is None:
        # No subcommand, run search TUI
        _run_search_tui()


def _run_search_tui():
    """Run interactive search TUI"""
    click.echo("\n" + "=" * 50)
    click.echo("           MoFA Search")
    click.echo("=" * 50 + "\n")

    # Ask what to search
    search_type = click.prompt(
        "What to search? (1=agents, 2=flows, q=quit)", type=str, default="1"
    )

    if search_type.lower() == "q":
        return

    keyword = click.prompt("Search keyword", type=str)

    # Ask scope
    scope = click.prompt(
        "Search where? (1=local, 2=remote, 3=both)", type=str, default="3"
    )

    local_only = scope == "1"
    remote_only = scope == "2"

    click.echo()

    remote_results = []

    if search_type == "1":
        # Search agents
        local_agents = get_subdirectories(agents_dir_path)
        keyword_lower = keyword.lower()

        if not remote_only:
            matches = [name for name in local_agents if keyword_lower in name.lower()]
            if matches:
                click.echo(f"Local agents matching '{keyword}' ({len(matches)}):")
                for name in sorted(matches):
                    agent_path = os.path.join(agents_dir_path, name)
                    click.echo(f"  [local] {name}")
                    click.echo(f"         {agent_path}")

        if not local_only:
            try:
                hub = HubClient()
                remote_matches = hub.search_agents(keyword)
                if remote_matches:
                    if not remote_only:
                        click.echo()
                    click.echo(
                        f"Remote agents matching '{keyword}' ({len(remote_matches)}):"
                    )
                    for idx, agent in enumerate(remote_matches, 1):
                        name = agent.get("name", "unknown")
                        desc = agent.get("description", "")
                        click.echo(f"  {idx}. [hub] {name}")
                        if desc:
                            click.echo(f"          {desc}")
                    remote_results = remote_matches

                    # Ask if user wants to download
                    if click.confirm("\nDownload any of these agents?", default=False):
                        choice = click.prompt("Select agent number", type=str)
                        try:
                            agent_idx = int(choice) - 1
                            if 0 <= agent_idx < len(remote_matches):
                                selected_agent = remote_matches[agent_idx]["name"]
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
            except Exception as e:
                click.echo(f"Error searching remote: {e}", err=True)

    elif search_type == "2":
        # Search flows
        local_flows = get_subdirectories(flows_dir_path)
        keyword_lower = keyword.lower()

        if not remote_only:
            matches = [name for name in local_flows if keyword_lower in name.lower()]
            if matches:
                click.echo(f"Local flows matching '{keyword}' ({len(matches)}):")
                for name in sorted(matches):
                    flow_path = os.path.join(flows_dir_path, name)
                    click.echo(f"  [local] {name}")
                    click.echo(f"         {flow_path}")

        if not local_only:
            try:
                hub = HubClient()
                remote_matches = hub.search_flows(keyword)
                if remote_matches:
                    if not remote_only:
                        click.echo()
                    click.echo(
                        f"Remote flows matching '{keyword}' ({len(remote_matches)}):"
                    )
                    for idx, flow in enumerate(remote_matches, 1):
                        name = flow.get("name", "unknown")
                        desc = flow.get("description", "")
                        click.echo(f"  {idx}. [hub] {name}")
                        if desc:
                            click.echo(f"          {desc}")
                    remote_results = remote_matches

                    # Ask if user wants to download
                    if click.confirm("\nDownload any of these flows?", default=False):
                        choice = click.prompt("Select flow number", type=str)
                        try:
                            flow_idx = int(choice) - 1
                            if 0 <= flow_idx < len(remote_matches):
                                selected_flow = remote_matches[flow_idx]["name"]
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
                click.echo(f"Error searching remote: {e}", err=True)


@search.command()
@click.argument("keyword", required=True)
@click.option("--local", is_flag=True, help="Search only local agents")
@click.option("--remote", is_flag=True, help="Search only remote hub agents")
def agent(keyword, local, remote):
    """Search for agents (searches both local and remote by default)"""

    # Get local agents
    local_agents = get_subdirectories(agents_dir_path)
    keyword_lower = keyword.lower()

    if local:
        # Local only
        matches = [name for name in local_agents if keyword_lower in name.lower()]
        click.echo(f"Local agents matching '{keyword}' ({len(matches)}):")
        if matches:
            for name in sorted(matches):
                agent_path = os.path.join(agents_dir_path, name)
                click.echo(f"  - {name}")
                click.echo(f"    {agent_path}")
        else:
            click.echo("  No matches found")
        return

    if remote:
        # Remote only
        try:
            hub = HubClient()
            matches = hub.search_agents(keyword)
            click.echo(f"Remote agents matching '{keyword}' ({len(matches)}):")
            if matches:
                for agent in matches:
                    name = agent.get("name", "unknown")
                    desc = agent.get("description", "")
                    tags = ", ".join(agent.get("tags", []))
                    click.echo(f"  - {name}")
                    if desc:
                        click.echo(f"    {desc}")
                    if tags:
                        click.echo(f"    Tags: {tags}")
            else:
                click.echo("  No matches found")
        except Exception as e:
            click.echo(f"Error searching remote agents: {e}", err=True)
        return

    # Both local and remote (default)
    local_matches = [name for name in local_agents if keyword_lower in name.lower()]

    if local_matches:
        click.echo(f"Local agents matching '{keyword}' ({len(local_matches)}):")
        for name in sorted(local_matches):
            agent_path = os.path.join(agents_dir_path, name)
            click.echo(f"  [local] {name}")
            click.echo(f"         {agent_path}")

    try:
        hub = HubClient()
        remote_matches = hub.search_agents(keyword)
        if remote_matches:
            if local_matches:
                click.echo()
            click.echo(f"Remote agents matching '{keyword}' ({len(remote_matches)}):")
            for agent in remote_matches:
                name = agent.get("name", "unknown")
                desc = agent.get("description", "")
                click.echo(f"  [hub] {name}")
                if desc:
                    click.echo(f"       {desc}")
    except Exception as e:
        click.echo(f"\nError searching remote agents: {e}", err=True)

    if not local_matches:
        try:
            hub = HubClient()
            remote_matches = hub.search_agents(keyword)
            if not remote_matches:
                click.echo(f"No agents found matching '{keyword}'")
        except:
            pass


@search.command()
@click.argument("keyword", required=True)
@click.option("--local", is_flag=True, help="Search only local flows")
@click.option("--remote", is_flag=True, help="Search only remote hub flows")
def flow(keyword, local, remote):
    """Search for flows (searches both local and remote by default)"""

    # Get local flows
    local_flows = get_subdirectories(flows_dir_path)
    keyword_lower = keyword.lower()

    if local:
        # Local only
        matches = [name for name in local_flows if keyword_lower in name.lower()]
        click.echo(f"Local flows matching '{keyword}' ({len(matches)}):")
        if matches:
            for name in sorted(matches):
                flow_path = os.path.join(flows_dir_path, name)
                click.echo(f"  - {name}")
                click.echo(f"    {flow_path}")
        else:
            click.echo("  No matches found")
        return

    if remote:
        # Remote only
        try:
            hub = HubClient()
            matches = hub.search_flows(keyword)
            click.echo(f"Remote flows matching '{keyword}' ({len(matches)}):")
            if matches:
                for flow in matches:
                    name = flow.get("name", "unknown")
                    desc = flow.get("description", "")
                    agents = ", ".join(flow.get("agents", []))
                    click.echo(f"  - {name}")
                    if desc:
                        click.echo(f"    {desc}")
                    if agents:
                        click.echo(f"    Agents: {agents}")
            else:
                click.echo("  No matches found")
        except Exception as e:
            click.echo(f"Error searching remote flows: {e}", err=True)
        return

    # Both local and remote (default)
    local_matches = [name for name in local_flows if keyword_lower in name.lower()]

    if local_matches:
        click.echo(f"Local flows matching '{keyword}' ({len(local_matches)}):")
        for name in sorted(local_matches):
            flow_path = os.path.join(flows_dir_path, name)
            click.echo(f"  [local] {name}")
            click.echo(f"         {flow_path}")

    try:
        hub = HubClient()
        remote_matches = hub.search_flows(keyword)
        if remote_matches:
            if local_matches:
                click.echo()
            click.echo(f"Remote flows matching '{keyword}' ({len(remote_matches)}):")
            for flow in remote_matches:
                name = flow.get("name", "unknown")
                desc = flow.get("description", "")
                click.echo(f"  [hub] {name}")
                if desc:
                    click.echo(f"       {desc}")
    except Exception as e:
        click.echo(f"\nError searching remote flows: {e}", err=True)

    if not local_matches:
        try:
            hub = HubClient()
            remote_matches = hub.search_flows(keyword)
            if not remote_matches:
                click.echo(f"No flows found matching '{keyword}'")
        except:
            pass


# ============ Download Command Group ============
@mofa_cli_group.group(invoke_without_command=True)
@click.pass_context
def download(ctx):
    """Download agents and flows from hub"""
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


# ============ Config Command Group ============
@mofa_cli_group.group(invoke_without_command=True)
@click.pass_context
def config(ctx):
    """Manage mofa configuration"""
    if ctx.invoked_subcommand is None:
        # No subcommand, run TUI
        ctx.invoke(tui)


@config.command()
def show():
    """Display current configuration"""
    click.echo("Current configuration:")
    click.echo(f"  OPENAI_API_KEY: {'***' if os.getenv('OPENAI_API_KEY') else '(not set)'}")
    click.echo(f"  OPENAI_API_BASE: {os.getenv('OPENAI_API_BASE', '(default)')}")
    click.echo(f"  MOFA_VIBE_MODEL: {os.getenv('MOFA_VIBE_MODEL', 'gpt-4o-mini (default)')}")
    click.echo(f"  MOFA_AGENTS_DIR: {agents_dir_path}")
    click.echo(f"  MOFA_FLOWS_DIR: {flows_dir_path}")
    click.echo(f"  MOFA_HUB_URL: {os.getenv('MOFA_HUB_URL', '(default)')}")


@config.command(name="set")
@click.argument("key", required=True)
@click.argument("value", required=True)
def set_config(key, value):
    """Set a configuration value in .env"""
    from mofa import project_root

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
    from mofa import project_root

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
        from mofa import project_root

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


# ============ Helper: Check API Key ============
def _check_and_setup_api_key() -> Optional[str]:
    """Check if API key exists, prompt user to configure if not"""
    api_key = os.getenv("OPENAI_API_KEY")

    if api_key:
        return api_key

    # No API key found
    click.echo("\nWarning: OpenAI API Key Not Found")
    click.echo("=" * 50)
    click.echo("The 'vibe' command requires an OpenAI API key.")
    click.echo("You can get one at: https://platform.openai.com/api-keys\n")

    if click.confirm("Would you like to configure it now?", default=True):
        # Jump to config
        click.echo("\nOpening configuration...\n")
        from mofa import project_root

        env_file = os.path.join(project_root, ".env")

        api_key = click.prompt("Enter your OpenAI API key", type=str, hide_input=True)

        if not api_key or not api_key.startswith("sk-"):
            click.echo("ERROR: Invalid API key format. Should start with 'sk-'")
            return None

        # Save to .env
        lines = []
        key_found = False

        if os.path.exists(env_file):
            with open(env_file, "r") as f:
                lines = f.readlines()

            for i, line in enumerate(lines):
                if line.strip().startswith("OPENAI_API_KEY="):
                    lines[i] = f"OPENAI_API_KEY={api_key}\n"
                    key_found = True
                    break

        if not key_found:
            lines.append(f"\n# Added by mofa vibe\n")
            lines.append(f"OPENAI_API_KEY={api_key}\n")

        with open(env_file, "w") as f:
            f.writelines(lines)

        os.environ["OPENAI_API_KEY"] = api_key
        click.echo(f"\nAPI key saved to {env_file}")
        click.echo("  (Make sure to add .env to .gitignore!)\n")

        return api_key
    else:
        click.echo("\nYou can configure it later using: mofa config")
        return None


# ============ Vibe Command Group ============
@mofa_cli_group.group(invoke_without_command=True)
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
        saved_model = os.getenv('MOFA_VIBE_MODEL', 'gpt-4o-mini')

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
        saved_model = os.getenv('MOFA_VIBE_MODEL', 'gpt-4o-mini')

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
                api_key=api_key
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
        llm = os.getenv('MOFA_VIBE_MODEL', 'gpt-4o-mini')
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


# ============ Helper Functions ============
def _create_venv(base_python: str, working_dir: str):
    temp_root = tempfile.mkdtemp(prefix="mofa_run_", dir=working_dir)
    venv_dir = os.path.join(temp_root, "venv")
    create_cmd = [base_python, "-m", "venv", venv_dir]
    create_proc = subprocess.run(create_cmd, capture_output=True, text=True)
    if create_proc.returncode != 0:
        shutil.rmtree(temp_root, ignore_errors=True)
        raise RuntimeError(
            create_proc.stderr.strip()
            or create_proc.stdout.strip()
            or "Failed to create virtual environment"
        )

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


def _extract_editable_path(build_command: str):
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


def _collect_editable_packages(dataflow_path: str, working_dir: str):
    data = read_yaml(dataflow_path)
    nodes = data.get("nodes", []) if isinstance(data, dict) else []
    editable_paths = []
    for node in nodes:
        if not isinstance(node, dict):
            continue
        build_cmd = node.get("build")
        if isinstance(build_cmd, str):
            editable = _extract_editable_path(build_cmd)
            if editable:
                abs_path = os.path.abspath(os.path.join(working_dir, editable))
                editable_paths.append(abs_path)
    return list(dict.fromkeys(editable_paths))


def _install_base_requirements(pip_executable: str, working_dir: str):
    # First install uv in the venv for faster package installation
    click.echo("Installing uv in virtual environment...")
    subprocess.run([pip_executable, "install", "--upgrade", "pip"], capture_output=True)
    uv_install = subprocess.run(
        [pip_executable, "install", "uv"], capture_output=True, text=True
    )

    # Determine the uv and python executable paths in the venv
    bin_dir = os.path.dirname(pip_executable)
    uv_executable = os.path.join(bin_dir, "uv.exe" if os.name == "nt" else "uv")
    python_executable = os.path.join(
        bin_dir, "python.exe" if os.name == "nt" else "python"
    )

    # Check if uv was installed successfully
    use_uv = uv_install.returncode == 0 and os.path.exists(uv_executable)

    if use_uv:
        click.echo("Using uv for fast package installation")
        # Use --python to ensure uv installs into the correct venv
        installer = [uv_executable, "pip", "install", "--python", python_executable]
    else:
        click.echo("Warning: Using pip (uv installation failed)")
        installer = [pip_executable, "install"]
        # Upgrade pip tools if using pip
        subprocess.run(
            [pip_executable, "install", "--upgrade", "setuptools", "wheel"],
            capture_output=True,
        )

    # Remove pathlib if it exists (conflicts with Python 3.11 built-in pathlib)
    if use_uv:
        subprocess.run(
            [
                uv_executable,
                "pip",
                "uninstall",
                "--python",
                python_executable,
                "-y",
                "pathlib",
            ],
            capture_output=True,
        )
    else:
        subprocess.run(
            [pip_executable, "uninstall", "-y", "pathlib"], capture_output=True
        )

    # Also remove any broken pathlib files manually
    venv_site_packages = (
        os.path.dirname(os.path.dirname(pip_executable))
        + "/lib/python3.11/site-packages"
    )
    pathlib_files = [
        os.path.join(venv_site_packages, "pathlib.py"),
        os.path.join(venv_site_packages, "pathlib.pyc"),
        os.path.join(venv_site_packages, "__pycache__", "pathlib.cpython-311.pyc"),
    ]
    for pathlib_file in pathlib_files:
        if os.path.exists(pathlib_file):
            os.remove(pathlib_file)

    # Install essential packages needed for dora-rs and basic functionality
    click.echo("Installing base packages...")
    base_packages = [
        "numpy==1.26.4",
        "pyarrow==17.0.0",
        "dora-rs-cli",
        "python-dotenv",
        "pyyaml",
    ]
    for package in base_packages:
        install_cmd = installer + [package]
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
            if "mofa-ai" in setup_content:
                mofa_root = current_dir
                break
        current_dir = os.path.dirname(current_dir)

    if mofa_root:
        click.echo("Installing mofa development version...")
        # Use --no-build-isolation to avoid pathlib conflicts
        install_cmd = installer + ["--no-build-isolation", "-e", mofa_root]
        proc = subprocess.run(install_cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            # If development install fails (e.g., permission issues), fall back to PyPI
            if "Permission denied" in proc.stderr:
                click.echo(
                    "Warning: Permission error installing dev version, using PyPI version..."
                )
                install_cmd = installer + ["mofa-core"]
                proc = subprocess.run(install_cmd, capture_output=True, text=True)
                if proc.returncode != 0:
                    raise RuntimeError(f"Failed to install mofa-core: {proc.stderr}")
            else:
                raise RuntimeError(f"Failed to install development mofa: {proc.stderr}")
    else:
        # Fallback to PyPI version if we can't find the development version
        install_cmd = installer + ["mofa-core"]
        proc = subprocess.run(install_cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            raise RuntimeError(f"Failed to install mofa-core: {proc.stderr}")

    # Final cleanup: remove pathlib again in case any dependency reinstalled it
    if use_uv:
        subprocess.run(
            [
                uv_executable,
                "pip",
                "uninstall",
                "--python",
                python_executable,
                "-y",
                "pathlib",
            ],
            capture_output=True,
        )
    else:
        subprocess.run(
            [pip_executable, "uninstall", "-y", "pathlib"], capture_output=True
        )
    for pathlib_file in pathlib_files:
        if os.path.exists(pathlib_file):
            os.remove(pathlib_file)

    # Return the installer command for use in other functions
    return installer if use_uv else None


def _install_packages(pip_executable: str, package_paths: List[str], installer=None):
    """Install packages using uv (if available) or pip."""
    # Use provided installer or fallback to pip
    if installer is None:
        installer = [pip_executable, "install"]

    for package_path in package_paths:
        if not os.path.exists(package_path):
            click.echo(f"Warning: package path not found: {package_path}")
            continue
        install_cmd = installer + ["--no-build-isolation", "--editable", package_path]
        proc = subprocess.run(install_cmd, text=True)
        if proc.returncode != 0:
            raise RuntimeError(f"Failed to install package from {package_path}")


def _build_env(base_env: dict, venv_info: dict):
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


def _run_flow_impl(dataflow_file: str):
    """Implementation of run-flow command"""
    dataflow_path = os.path.abspath(dataflow_file)
    if not os.path.exists(dataflow_path):
        click.echo(f"Error: Dataflow file not found: {dataflow_path}")
        return

    if not dataflow_path.endswith(".yml") and not dataflow_path.endswith(".yaml"):
        click.echo(f"Error: File must be a YAML file (.yml or .yaml): {dataflow_path}")
        return

    # Get the directory containing the dataflow file
    working_dir = os.path.dirname(dataflow_path)

    # Clean up any existing dora processes to avoid conflicts
    click.echo("Cleaning up existing dora processes...")
    subprocess.run(["pkill", "-f", "dora"], capture_output=True)
    time.sleep(1)  # Give processes time to die

    env_info = None
    run_env = os.environ.copy()
    editable_packages = []
    installer = None

    try:
        env_info = _create_venv(sys.executable, working_dir)
        run_env = _build_env(run_env, env_info)

        click.echo("Installing base requirements...")
        installer = _install_base_requirements(env_info["pip"], working_dir)

        editable_packages = _collect_editable_packages(dataflow_path, working_dir)
        if editable_packages:
            click.echo("Installing node packages into isolated environment...")
            _install_packages(env_info["pip"], editable_packages, installer=installer)
    except RuntimeError as runtime_error:
        click.echo(f"Failed to prepare run environment: {runtime_error}")
        if env_info:
            shutil.rmtree(env_info["root"], ignore_errors=True)
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
        click.echo(
            "You can now interact directly with the agents. Type 'exit' to quit."
        )

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
        if env_info:
            shutil.rmtree(env_info["root"], ignore_errors=True)
        click.echo("Main process terminated.")


# ============ Create Agent Command ============
@mofa_cli_group.command(name="create-agent")
@click.option("--name", default=None, help="Agent name")
@click.option("--version", default=None, help="Version of the new agent")
@click.option("--output", default=None, help="Output directory (default: ./agents)")
@click.option("--authors", default=None, help="Authors")
@click.option("--description", default=None, help="Agent description")
def create_agent(name, version, output, authors, description):
    """Create a new agent from template"""

    # Interactive TUI mode
    click.echo("\n" + "=" * 50)
    click.echo("         Create New MoFA Agent")
    click.echo("=" * 50 + "\n")

    # Collect inputs
    agent_name = name or click.prompt("Agent name", type=str)
    agent_version = version or click.prompt("Version", default="0.0.1")
    agent_description = description or click.prompt(
        "Description", default=f"A {agent_name} agent"
    )
    agent_authors = authors or click.prompt("Authors", default="MoFA Team")
    agent_output = output or click.prompt("Output directory", default=agents_dir_path)

    # Confirm
    click.echo("\n" + "-" * 50)
    click.echo("Agent Configuration:")
    click.echo(f"  Name: {agent_name}")
    click.echo(f"  Version: {agent_version}")
    click.echo(f"  Description: {agent_description}")
    click.echo(f"  Authors: {agent_authors}")
    click.echo(f"  Output: {agent_output}")
    click.echo("-" * 50 + "\n")

    if not click.confirm("Create agent?", default=True):
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

    # Use Cookiecutter to generate the new agent from the template
    try:
        result_path = cookiecutter(
            template=template_dir,
            output_dir=agent_output,
            no_input=True,
            extra_context={
                "user_agent_dir": agent_name,
                "agent_name": agent_name,
                "version": agent_version,
                "description": agent_description,
                "authors": agent_authors,
            },
        )
        click.echo(f"\nSuccessfully created agent: {result_path}")
        click.echo(f"\nNext steps:")
        click.echo(f"  1. cd {result_path}")
        click.echo(f"  2. Edit {agent_name}/main.py to implement your agent logic")
        click.echo(f"  3. Test with: mofa debug-agent {result_path} tests/test_main.py")
    except Exception as e:
        click.echo(f"\nError: Failed to create agent: {e}", err=True)
        import traceback

        traceback.print_exc()


# ============ Legacy Commands (Deprecated) ============
@mofa_cli_group.command(hidden=True)
@click.argument("node_folder_path", type=click.Path(exists=True))
@click.argument("test_case_yml", type=click.Path(exists=True), required=False)
@click.option("--interactive", is_flag=True, help="Enable interactive input (no YAML file required)")
def debug(node_folder_path, test_case_yml, interactive):
    """[Deprecated] Use 'mofa debug-agent' instead"""
    click.echo("Warning: 'debug' is deprecated, use 'mofa debug-agent' instead")
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
    """[Deprecated] Use 'mofa create agent' instead"""
    click.echo("Warning: 'new-agent' is deprecated, use 'mofa create agent' instead")
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


if __name__ == "__main__":
    mofa_cli_group()
