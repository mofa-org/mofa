"""Standalone flow generator - reads agents and generates flows"""

import os
from pathlib import Path
from typing import Dict, List, Optional
import re


class FlowGenerator:
    """Generate MoFA flows from available agents"""

    def __init__(self, agents_dir: str, flows_dir: str, llm_model: str = "gpt-4o-mini", api_key: Optional[str] = None):
        """
        Initialize flow generator

        Args:
            agents_dir: Path to agents directory
            flows_dir: Path to flows output directory
            llm_model: LLM model to use
            api_key: API key (if None, reads from env)
        """
        self.agents_dir = Path(agents_dir)
        self.flows_dir = Path(flows_dir)
        self.llm_model = llm_model
        self.api_key = api_key or os.getenv('OPENAI_API_KEY')

        if not self.api_key:
            raise ValueError("API key not found. Set OPENAI_API_KEY or pass api_key parameter.")

    def scan_agents(self) -> Dict[str, Dict]:
        """
        Scan agents directory and extract information about each agent

        Returns:
            Dict mapping agent_name -> {
                'path': str,
                'code': str,
                'inputs': List[str],
                'outputs': List[str],
                'description': str
            }
        """
        agents_info = {}

        if not self.agents_dir.exists():
            return agents_info

        for agent_path in self.agents_dir.iterdir():
            if not agent_path.is_dir() or agent_path.name.startswith('.'):
                continue

            # Find main.py (recursively, max depth 2)
            main_py_path = None
            try:
                for f in agent_path.rglob("main.py"):
                    if len(f.relative_to(agent_path).parts) <= 2:
                        main_py_path = f
                        break
            except:
                continue

            if not main_py_path:
                continue

            # Read agent code
            try:
                with open(main_py_path, 'r', encoding='utf-8') as f:
                    code = f.read()

                # Extract metadata
                info = self._extract_agent_info(code, str(agent_path))
                agents_info[agent_path.name] = info

            except Exception:
                # Skip agents that can't be read
                continue

        return agents_info

    def _extract_agent_info(self, code: str, agent_path: str) -> Dict:
        """Extract information from agent code"""

        # Extract input parameters
        inputs = re.findall(r'receive_parameter\([\'"](\w+)[\'"]\)', code)
        params_match = re.findall(r'receive_parameters\(\[(.*?)\]\)', code, re.DOTALL)
        for match in params_match:
            param_names = re.findall(r'[\'"](\w+)[\'"]', match)
            inputs.extend(param_names)
        inputs = list(set(inputs))

        # Extract output parameters
        outputs = re.findall(r'send_output\(agent_output_name=[\'"](\w+)[\'"]', code)
        outputs = list(set(outputs))

        # Extract docstring
        docstring_match = re.search(r'"""(.*?)"""', code, re.DOTALL)
        description = ""
        if docstring_match:
            desc = docstring_match.group(1).strip()
            # Take first line or first 200 chars
            first_line = desc.split('\n')[0].strip()
            description = first_line[:200] if len(first_line) > 0 else desc[:200]

        return {
            'path': agent_path,
            'code': code,
            'inputs': inputs,
            'outputs': outputs,
            'description': description
        }

    def generate_flow(self, requirement: str, flow_name: Optional[str] = None, interactive: bool = True) -> str:
        """
        Generate a flow from requirement

        Args:
            requirement: What the flow should do
            flow_name: Optional flow name (generated if not provided)
            interactive: Enable interactive refinement (default: True)

        Returns:
            Path to generated flow directory
        """
        from .llm_client import LLMClient
        from .flow_scaffolder import FlowScaffolder

        # Scan agents
        agents_info = self.scan_agents()

        if not agents_info:
            raise RuntimeError(f"No agents found in {self.agents_dir}. Create agents first.")

        # Initialize LLM client
        client = LLMClient(model=self.llm_model, api_key=self.api_key)

        # Generate flow name if not provided
        if not flow_name:
            flow_name = client.generate_agent_name(requirement)

        # Build agents context for LLM
        agents_context = self._build_agents_context(agents_info)

        # Generate flow YAML
        flow_yaml = self._generate_flow_yaml(client, requirement, flow_name, agents_context)

        # Interactive refinement if enabled
        if interactive:
            flow_yaml = self._interactive_refinement(client, flow_yaml, requirement, agents_context)

        # Extract agent names from YAML
        agent_names = self._extract_agent_names(flow_yaml)

        # Create flow project
        scaffolder = FlowScaffolder(output_dir=str(self.flows_dir))
        flow_path = scaffolder.create_flow(flow_name, flow_yaml, agent_names)

        return flow_path

    def _interactive_refinement(self, client, flow_yaml: str, requirement: str, agents_context: str) -> str:
        """
        Allow user to interactively refine the flow through conversation

        Args:
            client: LLM client
            flow_yaml: Initial flow YAML
            requirement: Original requirement
            agents_context: Context about available agents

        Returns:
            Final refined flow YAML
        """
        import yaml

        while True:
            # Show current flow
            print("\n" + "=" * 60)
            print("Generated Flow YAML:")
            print("=" * 60)
            print(flow_yaml)
            print("=" * 60)

            # Validate YAML
            try:
                yaml.safe_load(flow_yaml)
                print("\n✓ Valid YAML")
            except Exception as e:
                print(f"\n⚠ YAML validation failed: {e}")

            # Ask what to do
            print("\nOptions:")
            print("  1. Accept and create flow")
            print("  2. Modify with AI (describe changes)")
            print("  3. Regenerate from scratch")
            print("  q. Cancel")

            choice = input("\nChoice [1]: ").strip() or "1"

            if choice == "1":
                # Accept
                return flow_yaml
            elif choice.lower() == "q":
                raise KeyboardInterrupt("User cancelled")
            elif choice == "3":
                # Regenerate
                print("\nRegenerating flow...")
                flow_yaml = self._generate_flow_yaml(client, requirement, "", agents_context)
            elif choice == "2":
                # Conversational modification
                print("\n" + "=" * 60)
                print("Conversational Flow Editor")
                print("=" * 60)
                print("Describe the changes you want. Type 'done' when finished.\n")

                conversation_history = []

                while True:
                    try:
                        user_input = input("You: ").strip()
                    except EOFError:
                        break

                    if not user_input:
                        continue

                    if user_input.lower() in ['done', 'finish', 'ok']:
                        break

                    # Modify flow
                    print("AI is thinking...")
                    try:
                        modified_yaml = self._modify_flow_conversational(
                            client, flow_yaml, user_input, agents_context, conversation_history
                        )

                        # Validate
                        try:
                            yaml.safe_load(modified_yaml)
                            flow_yaml = modified_yaml
                            print("\n✓ Flow updated")
                        except Exception as e:
                            print(f"\n⚠ Modified YAML is invalid: {e}")
                            print("Keeping previous version.")

                        # Show updated flow
                        print("\nUpdated flow:")
                        print("-" * 60)
                        print(flow_yaml)
                        print("-" * 60)

                        # Update conversation history
                        conversation_history.append({"user": user_input, "yaml": flow_yaml})

                    except Exception as e:
                        print(f"\n✗ Error: {e}")

                # After conversation, show final version and loop back
                continue
            else:
                print("Invalid choice. Please try again.")

        return flow_yaml

    def _modify_flow_conversational(self, client, current_yaml: str, user_instruction: str,
                                    agents_context: str, conversation_history: list) -> str:
        """Modify flow YAML based on user instruction"""

        history_str = ""
        if conversation_history:
            history_str = "\nPrevious modifications:\n"
            for i, item in enumerate(conversation_history[-3:], 1):  # Last 3 changes
                history_str += f"{i}. User: {item['user']}\n"

        prompt = f"""Modify the following MoFA flow YAML according to the user's instruction.

Current YAML:
```yaml
{current_yaml}
```
{history_str}

{agents_context}

User instruction: {user_instruction}

IMPORTANT Rules:
1. Only use agents from the "Available Agents" section above
2. Maintain valid YAML structure
3. Keep the same flow format (nodes with id, build, path, inputs, outputs, env)
4. Ensure proper input/output connections
5. Last node must have IS_DATAFLOW_END: true in env

Output ONLY the complete modified YAML, no explanations or markdown:
"""

        yaml_str = client.generate(prompt)

        # Clean up markdown
        yaml_str = re.sub(r'```yaml\n', '', yaml_str)
        yaml_str = re.sub(r'```\n?', '', yaml_str)

        return yaml_str.strip()

    def _build_agents_context(self, agents_info: Dict) -> str:
        """Build context string with all agents info for LLM"""

        context_parts = ["Available agents:\n"]

        for agent_name, info in sorted(agents_info.items()):
            context_parts.append(f"\n## Agent: {agent_name}")
            context_parts.append(f"Path: {info['path']}")

            if info['description']:
                context_parts.append(f"Description: {info['description']}")

            if info['inputs']:
                context_parts.append(f"Inputs: {', '.join(info['inputs'])}")
            else:
                context_parts.append("Inputs: (none)")

            if info['outputs']:
                context_parts.append(f"Outputs: {', '.join(info['outputs'])}")
            else:
                context_parts.append("Outputs: (none)")

            # Include code snippet (first 50 lines or 2000 chars)
            code_lines = info['code'].split('\n')[:50]
            code_snippet = '\n'.join(code_lines)[:2000]
            context_parts.append(f"\nCode:\n```python\n{code_snippet}\n```")

        return '\n'.join(context_parts)

    def _generate_flow_yaml(self, client, requirement: str, flow_name: str, agents_context: str) -> str:
        """Generate flow YAML using LLM"""

        prompt = f"""Generate a MoFA flow dataflow YAML based on this requirement:

## Requirement
{requirement}

## Flow Name
{flow_name}

## Available Agents
{agents_context}

## Flow YAML Structure
A flow is a YAML file that defines nodes (agents) and their connections.

Example:
```yaml
nodes:
  - id: terminal-input
    build: pip install -e ../../agents/terminal-input
    path: dynamic
    outputs:
      - data
    inputs:
      result: agent2/final_output

  - id: agent1
    build: pip install -e ../../agents/agent1
    path: agent1
    outputs:
      - processed_data
    inputs:
      raw_input: terminal-input/data

  - id: agent2
    build: pip install -e ../../agents/agent2
    path: agent2
    outputs:
      - final_output
    inputs:
      data: agent1/processed_data
    env:
      IS_DATAFLOW_END: true
      WRITE_LOG: true
```

## Important Rules
1. ONLY use agents from the "Available Agents" list above
2. Use the exact input/output parameter names shown for each agent
3. Start with terminal-input node for user input
4. Connect outputs to inputs: format is `node-id/output-name`
5. Last node MUST have `IS_DATAFLOW_END: true` and `WRITE_LOG: true` in env
6. Build command format: `pip install -e ../../agents/AGENT_NAME`
7. Path should match the agent name
8. Keep it simple (2-4 agents maximum)

Output ONLY the YAML content, no explanations or markdown:
"""

        yaml_str = client.generate(prompt)

        # Clean up markdown
        yaml_str = re.sub(r'```yaml\n', '', yaml_str)
        yaml_str = re.sub(r'```\n?', '', yaml_str)

        return yaml_str.strip()

    def _extract_agent_names(self, flow_yaml: str) -> List[str]:
        """Extract agent names from flow YAML"""
        import yaml

        try:
            flow_data = yaml.safe_load(flow_yaml)
            if 'nodes' in flow_data:
                return [
                    node.get('id', '')
                    for node in flow_data['nodes']
                    if node.get('id') != 'terminal-input' and node.get('id')
                ]
        except:
            pass

        return []
