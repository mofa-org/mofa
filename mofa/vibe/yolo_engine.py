"""
YOLO Engine - Fast multi-node generation with minimal questions

Generates multiple agents and their flow in one go based on a single requirement.
"""
import os
from pathlib import Path
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn

from mofa.vibe.llm_client import LLMClient
from mofa.vibe.engine import VibeEngine
from mofa.vibe.models import VibeConfig, AgentCode
from mofa.vibe.scaffolder import ProjectScaffolder


class YoloEngine:
    """YOLO mode engine for fast multi-node generation"""

    def __init__(self, requirement: str, llm_model: str, api_key: str,
                 agents_output: str, flows_output: str):
        self.requirement = requirement
        self.llm_model = llm_model
        self.api_key = api_key
        self.agents_output = agents_output
        self.flows_output = flows_output
        self.console = Console()
        self.llm = LLMClient(model=llm_model, api_key=api_key)

    def run(self):
        """Run YOLO generation"""
        try:
            # Step 1: Analyze requirement and generate plan
            self.console.print("\n[cyan]Analyzing requirement...[/cyan]")
            plan = self._generate_plan()

            # Step 2: Generate new agents and collect reused agents
            total_agents = len(plan.get('reuse_agents', [])) + len(plan.get('new_agents', []))
            self.console.print(f"\n[cyan]Processing {total_agents} agents...[/cyan]")
            agent_paths = self._generate_agents(plan)

            # Step 3: Generate flow
            self.console.print("\n[cyan]Generating flow...[/cyan]")
            flow_path = self._generate_flow(plan, agent_paths)

            # Step 4: Auto-run flow test
            self.console.print("\n[cyan]Running flow test...[/cyan]")
            self._run_flow_test(flow_path)

            # Collect all agent names (reused + new)
            all_agents = []
            if 'reuse_agents' in plan:
                all_agents.extend([a['name'] for a in plan['reuse_agents']])
            if 'new_agents' in plan:
                all_agents.extend([a['name'] for a in plan['new_agents']])

            return {
                'flow_path': flow_path,
                'agents': all_agents
            }

        except Exception as e:
            self.console.print(f"[red]Error: {e}[/red]")
            raise

    def _scan_existing_agents(self):
        """Scan existing agents directory and extract agent information"""
        agents_info = []
        agents_dir = Path(self.agents_output)

        if not agents_dir.exists():
            return agents_info

        # Iterate through agent directories
        for agent_dir in agents_dir.iterdir():
            # Skip hidden files and non-directories
            if agent_dir.name.startswith('.') or not agent_dir.is_dir():
                continue

            # Skip terminal-input (special agent)
            if agent_dir.name == 'terminal-input':
                continue

            try:
                agent_info = self._extract_agent_info(agent_dir)
                if agent_info:
                    agents_info.append(agent_info)
            except Exception as e:
                # Skip agents that can't be parsed
                continue

        return agents_info

    def _extract_agent_info(self, agent_dir):
        """Extract information from a single agent"""
        agent_name = agent_dir.name
        main_py = agent_dir / 'agent' / 'main.py'
        readme = agent_dir / 'README.md'

        if not main_py.exists():
            return None

        # Read main.py
        with open(main_py, 'r', encoding='utf-8') as f:
            code = f.read()

        # Extract input/output parameters
        import re
        inputs = re.findall(r"receive_parameter\(['\"](\w+)['\"]\)", code)
        outputs = re.findall(r"agent_output_name=['\"](\w+)['\"]", code)

        # Read description from README if exists
        description = ""
        if readme.exists():
            with open(readme, 'r', encoding='utf-8') as f:
                readme_content = f.read()
                # Try to extract first meaningful line
                lines = [l.strip() for l in readme_content.split('\n') if l.strip() and not l.startswith('#')]
                if lines and not lines[0].startswith('Auto-generated'):
                    description = lines[0]

        # If no description, try to infer from code comments
        if not description:
            comment_match = re.search(r'#\s*(.+)', code)
            if comment_match:
                description = comment_match.group(1).strip()

        # Fallback: generate description from agent name
        if not description:
            description = f"Agent: {agent_name.replace('-', ' ')}"

        return {
            'name': agent_name,
            'description': description[:100],  # Limit length
            'inputs': inputs,
            'outputs': outputs,
            'code_snippet': code[:300]  # First 300 chars for context
        }

    def _generate_plan(self):
        """Use LLM to analyze requirement and create generation plan"""
        # Scan existing agents
        existing_agents = self._scan_existing_agents()

        # Build context about existing agents
        existing_context = ""
        if existing_agents:
            existing_context = "\n\nExisting agents you can reuse:\n"
            for agent in existing_agents:
                existing_context += f"- {agent['name']}: {agent['description']}\n"
                existing_context += f"  Input: {', '.join(agent['inputs']) if agent['inputs'] else 'none'}\n"
                existing_context += f"  Output: {', '.join(agent['outputs']) if agent['outputs'] else 'none'}\n"

        prompt = f"""Analyze this requirement and create a plan for generating MoFA agents and their dataflow:
{existing_context}

Requirement: {self.requirement}

You need to:
1. Check if existing agents can be reused for the required functionality
2. Only create NEW agents for functionality that doesn't exist
3. Break down into 2-6 agents maximum
4. Design the dataflow connections - can be linear, parallel, or any topology

Return a JSON object with this structure:
{{
    "reuse_agents": [
        {{
            "name": "existing-agent-name",
            "reason": "Why reusing this agent"
        }}
    ],
    "new_agents": [
        {{
            "name": "new-agent-name",
            "description": "What this agent does",
            "input_param": "parameter_name",
            "output_param": "result_name"
        }}
    ],
    "connections": [
        {{
            "from": "source-agent-name (or 'terminal-input')",
            "from_param": "output_parameter_name (or 'data' for terminal-input)",
            "to": "target-agent-name",
            "to_param": "input_parameter_name"
        }}
    ]
}}

IMPORTANT RULES:
1. Prefer reusing existing agents when their functionality matches
2. Define ALL connections explicitly in the "connections" array
3. The dataflow must start from "terminal-input" with output "data"
4. The dataflow must end by sending back to "terminal-input" with input "agent_response"
5. You can have parallel branches, multiple inputs, or any topology as needed
6. Make sure input/output parameter names match the actual agent definitions

Example connection patterns:
- Linear: terminal-input -> agent1 -> agent2 -> terminal-input
- Parallel: terminal-input -> [agent1, agent2] -> merger -> terminal-input
- Multi-input: terminal-input -> agent1 -> agent3
                            └-> agent2 -> agent3
"""

        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=self.console,
            transient=True
        ) as progress:
            task = progress.add_task("Planning...", total=None)

            # Use LLMClient's generate method
            response_text = self.llm.generate(prompt)

            import json
            plan_text = response_text.strip()

            # Extract JSON from markdown code blocks if present
            if "```json" in plan_text:
                plan_text = plan_text.split("```json")[1].split("```")[0].strip()
            elif "```" in plan_text:
                plan_text = plan_text.split("```")[1].split("```")[0].strip()

            plan = json.loads(plan_text)

        # Display plan
        self.console.print(f"\n[bold]Plan:[/bold]")

        if 'reuse_agents' in plan and plan['reuse_agents']:
            self.console.print(f"\n[cyan]Reusing existing agents:[/cyan]")
            for agent in plan['reuse_agents']:
                self.console.print(f"  ✓ {agent['name']}: {agent.get('reason', '')}")

        if 'new_agents' in plan and plan['new_agents']:
            self.console.print(f"\n[yellow]Creating new agents:[/yellow]")
            for i, agent in enumerate(plan['new_agents'], 1):
                self.console.print(f"  {i}. {agent['name']}: {agent['description']}")

        return plan

    def _generate_agent_code_yolo(self, agent_name: str, description: str, input_param: str, output_param: str) -> str:
        """Generate agent code in YOLO mode (without test cases)"""
        prompt = f"""
Generate a complete MoFA agent implementation for the following requirement.

Agent Name: {agent_name}
Description: {description}
Input Parameter: {input_param}
Output Parameter: {output_param}

Requirements:
1. Use Dora-rs event system
2. Input: Receive from previous node via event.get("{input_param}")
3. Output: Send to next node via event.send_output("{output_param}", value)
4. Include proper error handling
5. Use Python 3.11+ syntax
6. Keep it simple and focused

Template structure:
```python
from dora import Node

def main():
    node = Node()

    for event in node:
        if event["type"] == "INPUT":
            # Your implementation here
            {input_param} = event.get("{input_param}")

            # Process the data
            {output_param} = ...  # Your logic

            # Send output
            event.send_output("{output_param}", {output_param})

if __name__ == "__main__":
    main()
```

Return ONLY the Python code, no explanations.
"""

        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{{task.description}}"),
            console=self.console,
            transient=True
        ) as progress:
            task = progress.add_task(f"Generating {agent_name}...", total=None)

            code_str = self.llm.generate(prompt)

            # Clean up code (remove markdown blocks)
            if "```python" in code_str:
                code_str = code_str.split("```python")[1].split("```")[0].strip()
            elif "```" in code_str:
                code_str = code_str.split("```")[1].split("```")[0].strip()

        return code_str

    def _generate_agents(self, plan):
        """Generate new agents and collect paths of reused agents"""
        agent_paths = {}

        # First, add paths for reused agents
        if 'reuse_agents' in plan:
            for agent_spec in plan['reuse_agents']:
                agent_name = agent_spec['name']
                agent_path = Path(self.agents_output) / agent_name
                if agent_path.exists():
                    agent_paths[agent_name] = str(agent_path)
                    self.console.print(f"  [green]✓[/green] {agent_name} (reused)")

        # Then generate new agents
        if 'new_agents' in plan:
            for agent_spec in plan['new_agents']:
                agent_name = agent_spec['name']
                agent_desc = agent_spec['description']
                input_param = agent_spec.get('input_param', 'data')
                output_param = agent_spec.get('output_param', 'result')

                self.console.print(f"\n  Generating {agent_name}...")

                # Generate code directly using LLM (no tests needed in YOLO mode)
                code_str = self._generate_agent_code_yolo(agent_name, agent_desc, input_param, output_param)

                # Create project using scaffolder
                from mofa.vibe.scaffolder import ProjectScaffolder

                scaffolder = ProjectScaffolder(output_dir=self.agents_output)
                # In YOLO mode, we don't need tests, pass empty test_yaml
                agent_path = scaffolder.create_project(
                    agent_name=agent_name,
                    code=code_str,
                    test_yaml="",  # No tests in YOLO mode
                )
                agent_paths[agent_name] = agent_path

                self.console.print(f"  [green]✓[/green] {agent_name}")

        return agent_paths

    def _generate_flow(self, plan, agent_paths):
        """Generate flow YAML connecting all agents based on explicit connections"""
        import yaml

        # Collect all agents info
        all_agents = {}

        # Add reused agents with their existing info
        if 'reuse_agents' in plan:
            for agent_spec in plan['reuse_agents']:
                agent_info = self._get_agent_io_from_existing(agent_spec['name'])
                if agent_info:
                    all_agents[agent_spec['name']] = {
                        'input_param': agent_info['input_param'],
                        'output_param': agent_info['output_param']
                    }

        # Add new agents
        if 'new_agents' in plan:
            for agent_spec in plan['new_agents']:
                all_agents[agent_spec['name']] = {
                    'input_param': agent_spec['input_param'],
                    'output_param': agent_spec['output_param']
                }

        # Build connection map: {agent_name: {input_param: source}}
        connections = plan.get('connections', [])
        agent_inputs = {}  # {agent_name: {param_name: source}}
        terminal_input_source = None  # What feeds back to terminal-input

        for conn in connections:
            from_agent = conn['from']
            from_param = conn['from_param']
            to_agent = conn['to']
            to_param = conn['to_param']

            source = f"{from_agent}/{from_param}"

            if to_agent == 'terminal-input':
                # This is the feedback to terminal-input
                terminal_input_source = source
            else:
                # Regular agent connection
                if to_agent not in agent_inputs:
                    agent_inputs[to_agent] = {}
                agent_inputs[to_agent][to_param] = source

        # If no explicit terminal-input feedback, use last agent
        if not terminal_input_source and all_agents:
            last_agent_name = list(all_agents.keys())[-1]
            last_agent_output = all_agents[last_agent_name]['output_param']
            terminal_input_source = f"{last_agent_name}/{last_agent_output}"

        # Build flow structure
        total_agents = len(all_agents)
        flow_name = f"yolo-flow-{total_agents + 1}-agents"
        flow_dir = Path(self.flows_output) / flow_name
        flow_dir.mkdir(parents=True, exist_ok=True)

        # Create nodes list
        nodes = []

        # Add terminal-input as first node
        nodes.append({
            'id': 'terminal-input',
            'build': f'pip install -e ../../agents/terminal-input',
            'path': 'dynamic',
            'outputs': ['data'],
            'inputs': {
                'agent_response': terminal_input_source
            }
        })

        # Add all other agents with their connections
        for agent_name, agent_info in all_agents.items():
            output_param = agent_info['output_param']

            # Get inputs for this agent
            inputs = agent_inputs.get(agent_name, {})

            # If no explicit inputs, default to terminal-input/data
            if not inputs:
                input_param = agent_info['input_param']
                inputs = {input_param: 'terminal-input/data'}

            node = {
                'id': agent_name,
                'build': f'pip install -e ../../agents/{agent_name}',
                'path': agent_name,
                'outputs': [output_param],
                'inputs': inputs
            }

            # Check if this is the final agent (feeds back to terminal-input)
            if terminal_input_source and terminal_input_source.startswith(f"{agent_name}/"):
                node['env'] = {
                    'IS_DATAFLOW_END': True,
                    'WRITE_LOG': True
                }

            nodes.append(node)

        flow_data = {'nodes': nodes}

        # Write flow YAML
        flow_yaml_path = flow_dir / f'{flow_name}_dataflow.yml'
        with open(flow_yaml_path, 'w') as f:
            yaml.dump(flow_data, f, default_flow_style=False, sort_keys=False)

        return str(flow_yaml_path)

    def _get_agent_io_from_existing(self, agent_name):
        """Extract input/output params from existing agent"""
        agent_dir = Path(self.agents_output) / agent_name
        main_py = agent_dir / 'agent' / 'main.py'

        if not main_py.exists():
            return None

        try:
            with open(main_py, 'r', encoding='utf-8') as f:
                code = f.read()

            import re
            inputs = re.findall(r"receive_parameter\(['\"](\w+)['\"]\)", code)
            outputs = re.findall(r"agent_output_name=['\"](\w+)['\"]", code)

            if inputs and outputs:
                return {
                    'input_param': inputs[0],
                    'output_param': outputs[0]
                }
        except Exception:
            pass

        return None

    def _run_flow_test(self, flow_yaml_path):
        """Auto-run flow test once with better interrupt handling"""
        import subprocess
        import signal

        self.console.print("\n" + "=" * 60)
        self.console.print("[bold cyan]Auto Flow Testing[/bold cyan]")
        self.console.print("=" * 60)
        self.console.print("Testing the flow with terminal-input...")
        self.console.print("[dim]Tip: Type 'exit' in terminal-input to quit gracefully[/dim]")
        self.console.print("[dim]Or press Ctrl+C to force stop[/dim]")
        self.console.print("=" * 60 + "\n")

        process = None
        try:
            # Run the flow test
            process = subprocess.Popen(
                ['mofa', 'run-flow', flow_yaml_path],
                cwd=str(Path(flow_yaml_path).parent),
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                bufsize=1
            )

            # Stream output in real-time
            for line in process.stdout:
                print(line, end='')

            # Wait for process to complete
            process.wait()

        except KeyboardInterrupt:
            self.console.print("\n\n[yellow]Interrupted by user (Ctrl+C)[/yellow]")

            # Gracefully terminate the process
            if process and process.poll() is None:
                try:
                    # Try graceful termination first
                    process.terminate()
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    # Force kill if it doesn't terminate
                    process.kill()
                    process.wait()

                self.console.print("[dim]Flow process terminated[/dim]")

        except Exception as e:
            self.console.print(f"\n[red]Error during flow test: {e}[/red]")
            if process and process.poll() is None:
                process.kill()
                process.wait()
