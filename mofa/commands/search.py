"""
MoFA search command - Search for nodes and flows
"""

import os
import click
from mofa import agents_dir_path, flows_dir_path
from mofa.utils.files.dir import get_subdirectories
from mofa.registry import HubClient


def register_search_commands(cli_group):
    """Register search command group to the main CLI"""

    @cli_group.group(invoke_without_command=True)
    @click.pass_context
    def search(ctx):
        """Search for nodes and flows"""
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
                matches = [
                    name for name in local_agents if keyword_lower in name.lower()
                ]
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
                        if click.confirm(
                            "\nDownload any of these agents?", default=False
                        ):
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
                matches = [
                    name for name in local_flows if keyword_lower in name.lower()
                ]
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
                        if click.confirm(
                            "\nDownload any of these flows?", default=False
                        ):
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
                click.echo(
                    f"Remote agents matching '{keyword}' ({len(remote_matches)}):"
                )
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
                click.echo(
                    f"Remote flows matching '{keyword}' ({len(remote_matches)}):"
                )
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
