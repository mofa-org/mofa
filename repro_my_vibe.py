import click
from click.testing import CliRunner
from mofa.commands.vibe import register_vibe_commands

@click.group()
def cli():
    pass

register_vibe_commands(cli)
runner = CliRunner()
result = runner.invoke(cli, ['vibe', 'agent'])
print('Exit code:', result.exit_code)
print(result.output)
