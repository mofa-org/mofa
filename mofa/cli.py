import os
import shutil
import time
import uuid
import subprocess
from mofa import agent_dir_path, cli_dir_path

import click
import sys
from mofa.utils.files.dir import get_subdirectories
from mofa.utils.process.util import stop_process, stop_dora_dataflow

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
@click.option('--agent-name', default='reasoner', help='agent name')
def run(agent_name: str = 'reasoner'):
    agent_path = agent_dir_path + f'/{agent_name}'
    agent_dataflow_path = agent_path + f'/{agent_name}_dataflow.yml'

    dora_up_process = subprocess.Popen(
        # conda_cmd_list + ['dora', 'up'] if is_conda_run else ['dora', 'up'],
        ['dora', 'up'],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        cwd=agent_path,
    )
    time.sleep(1)

    dora_build_node = subprocess.Popen(
        ['dora', 'build', agent_dataflow_path],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        cwd=agent_path
    )

    time.sleep(3)
    stdout, stderr = dora_build_node.communicate()
    dataflow_name = str(uuid.uuid4()).replace('-','')
    if dora_build_node.returncode == 0:
        # 启动 dora_dataflow_process 进程并等待其启动
        dora_dataflow_process = subprocess.Popen(
            ['dora', 'start', agent_dataflow_path,'--name',dataflow_name],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd=agent_path
        )

        time.sleep(2)

        task_input_process = subprocess.Popen(
            ['terminal-input'],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=agent_path,
            bufsize=0,
            universal_newlines=True
        )

        try:
            while True:
                task_input = input(">>>  ")
                if task_input.lower() in ["exit", "quit"]:
                    stop_process([dora_dataflow_process,task_input_process])
                    stop_dora_dataflow(dataflow_name=dataflow_name)
                    break

                if task_input_process.poll() is None:
                    task_input_process.stdin.write(task_input + '\n')
                    task_input_process.stdin.flush()
                else:
                    print("Process has already terminated. Cannot send input.")
                    break
                time.sleep(2)
                while True:
                    # ready, _, _ = select.select([task_input_process.stdout], [], [], 0.2)
                    # if ready:
                    output = task_input_process.stdout.readline()
                    if output:
                        print(output.strip().replace('Send You Task :', '').replace('Answer:', '').replace(':dataflow_status',''), flush=True)
                        sys.stdout.flush()
                        if ":dataflow_status" in output:
                            break
                    else:
                        print('wait agent result')
        except KeyboardInterrupt:
            stop_process([dora_dataflow_process, task_input_process])
            stop_dora_dataflow(dataflow_name=dataflow_name)
        finally:
            stop_process([dora_dataflow_process,task_input_process])
            stop_dora_dataflow(dataflow_name=dataflow_name)
            click.echo("Main process terminated.")

@mofa_cli_group.command()
@click.argument('agent_name', required=True)
@click.option('--version', default='0.0.1', help='Version of the new agent')
@click.option('--output', default=os.getcwd()+"/", help='node output path')
@click.option('--authors', default='Mofa Bot', help='authors')
def new_agent(agent_name: str, version: str, output: str, authors: str):
    """Create a new agent from the template with configuration options using Cookiecutter."""

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