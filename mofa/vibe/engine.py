"""Core Vibe engine for agent generation"""

import os
import re
import shutil
from pathlib import Path
from typing import Optional
from datetime import datetime

from rich.console import Console
from rich.panel import Panel
from rich.syntax import Syntax
from rich.progress import Progress, SpinnerColumn, TextColumn, BarColumn
from rich.table import Table
from rich.prompt import Prompt, Confirm

from .models import (
    VibeConfig,
    TestSuite,
    TestCase,
    AgentCode,
    GenerationRound,
    GenerationResult,
    TestResult
)
from .llm_client import LLMClient
from .scaffolder import ProjectScaffolder
from .debug_runner import DebugRunner


class VibeEngine:
    """Main orchestrator for agent generation"""

    def __init__(self, config: Optional[VibeConfig] = None):
        self.config = config or VibeConfig()
        self.console = Console()
        self.llm = LLMClient(
            model=self.config.llm_model,
            api_key=self.config.llm_api_key,
            temperature=self.config.temperature
        )
        self.scaffolder = ProjectScaffolder(output_dir=self.config.output_dir)
        self.debug_runner = DebugRunner()
        self.output_dir = Path(self.config.output_dir)

        # State
        self.requirement = ""
        self.agent_name = ""
        self.test_suite: Optional[TestSuite] = None
        self.rounds: list[GenerationRound] = []
        self.project_path = ""
        self.skip_testing = False  # Flag to skip testing for open-ended outputs

    def run_interactive(self) -> GenerationResult:
        """Run in interactive mode"""
        self._show_header()

        # Step 1: Get requirement from user
        self._ask_requirement()

        # Step 2: Generate and confirm test cases
        self._generate_and_confirm_tests()

        # Step 3: Auto-generate and optimize
        result = self._auto_optimize_loop()

        # Step 4: Show final summary
        self._show_final_summary(result)

        return result

    def _show_header(self):
        """Display welcome header"""
        self.console.print()
        self.console.print("[bold cyan]MoFA Vibe - AI Agent Generator[/bold cyan]")
        self.console.print("[dim]Automatically generate agents from requirements[/dim]")
        self.console.print()

    def _ask_requirement(self):
        """Ask user for agent requirement"""
        self.requirement = Prompt.ask("[bold]Describe what the agent should do[/bold]", console=self.console)

        # Generate agent name from requirement using LLM
        self.console.print("[dim]Generating agent name...[/dim]")
        self.agent_name = self.llm.generate_agent_name(self.requirement)

        # Ask if user wants to customize name
        suggested_name = Prompt.ask(
            f"[bold]Agent name[/bold] [dim](suggested: {self.agent_name})[/dim]",
            default=self.agent_name,
            console=self.console
        )
        self.agent_name = suggested_name or self.agent_name

    def _generate_and_confirm_tests(self):
        """Generate test cases and get user confirmation"""
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=self.console,
            transient=True
        ) as progress:
            task = progress.add_task("Generating test cases...", total=None)

            # Generate test cases using LLM
            test_yaml = self.llm.generate_test_cases(self.requirement)

            # Clean up any markdown code blocks
            test_yaml = self._clean_yaml_response(test_yaml)

            progress.update(task, completed=True)

        # Display generated tests
        self.console.print()
        self.console.print("[bold]Generated Test Cases[/bold]")
        self.console.print()

        syntax = Syntax(test_yaml, "yaml", theme="monokai", line_numbers=False)
        self.console.print(syntax)
        self.console.print()

        # Ask for confirmation
        while True:
            self.console.print("[dim]Options: y=go unit testing / n=regenerate test file / skip=go interactive testing[/dim]")
            response = Prompt.ask(
                "Confirm",
                choices=["y", "n", "skip"],
                default="skip",
                console=self.console,
                show_choices=False
            )

            if response == "y":
                self.test_suite = TestSuite.from_yaml(test_yaml)
                break
            elif response == "skip":
                self.console.print("[dim]Skipping tests, generating code only[/dim]")
                # Create a minimal dummy test suite for code generation
                dummy_yaml = """test_cases:
  - name: basic_functionality
    input:
      user_input: "test"
    validation:
      type: str
      not_empty: true
"""
                self.test_suite = TestSuite.from_yaml(dummy_yaml)
                self.skip_testing = True
                break
            else:  # n
                additional_req = Prompt.ask(
                    "[yellow]Additional test requirements[/yellow]",
                    default="",
                    console=self.console
                )
                if additional_req:
                    self.requirement += f"\n\nTest requirements: {additional_req}"
                # Regenerate
                return self._generate_and_confirm_tests()

    def _clean_yaml_response(self, yaml_str: str) -> str:
        """Remove markdown code blocks and fix common issues in LLM response"""
        import yaml

        # Remove ```yaml and ``` markers
        yaml_str = re.sub(r'```yaml\n', '', yaml_str)
        yaml_str = re.sub(r'```\n?', '', yaml_str)
        yaml_str = yaml_str.strip()

        # Fix common Python expression issues in YAML
        # Pattern: "string"*number or 'string'*number
        def replace_repeated_string(match):
            quote = match.group(1)
            char = match.group(2)
            count = int(match.group(3))
            # Limit to reasonable length
            actual_count = min(count, 100)
            return f'{quote}{char * actual_count}{quote}'

        # Replace patterns like "a"*1000 with actual repeated strings
        yaml_str = re.sub(r'(["\'])(\w)\1\s*\*\s*(\d+)', replace_repeated_string, yaml_str)

        # Try to parse and validate
        try:
            yaml.safe_load(yaml_str)
        except yaml.YAMLError as e:
            self.console.print(f"[yellow]Warning: YAML parsing error, attempting to fix...[/yellow]")
            # Additional cleanup attempts
            # Remove any remaining * expressions in lists
            yaml_str = re.sub(r'\[\s*["\'](\w+)["\']\s*\*\s*\d+\s*\]', r'["\1"]', yaml_str)

            # Try parsing again
            try:
                yaml.safe_load(yaml_str)
            except yaml.YAMLError:
                # If still fails, we'll let it fail naturally and user can regenerate
                self.console.print("[yellow]Auto-fix failed, may need to regenerate[/yellow]")

        return yaml_str

    def _auto_optimize_loop(self) -> GenerationResult:
        """Automatic generation and optimization loop"""
        self.console.print()
        self.console.print("[bold]Starting generation[/bold]")

        if self.skip_testing:
            self.console.print("[dim]Testing skipped[/dim]")
        else:
            self.console.print("[dim]Press Ctrl+C to interrupt[/dim]")

        self.console.print()

        current_code = None
        test_result = None

        try:
            # If skip_testing, use flow-based testing loop
            if self.skip_testing:
                round_num = 0
                max_rounds = self.config.max_optimization_rounds

                while True:
                    round_num += 1

                    # Check if we've exceeded max rounds
                    if max_rounds > 0 and round_num > max_rounds:
                        self.console.print(f"[yellow]Reached maximum rounds ({max_rounds})[/yellow]")
                        break

                    self.console.print(f"[cyan]Round {round_num}[/cyan]")

                    # Generate or regenerate code
                    if round_num == 1:
                        current_code = self._generate_initial_code()
                    else:
                        # For regeneration, ask user what to fix
                        self.console.print("\n[yellow]What needs to be fixed?[/yellow]")
                        fix_description = Prompt.ask(
                            "Describe the issues",
                            default="",
                            console=self.console
                        )
                        if fix_description:
                            current_code = self._regenerate_code_with_feedback(current_code, fix_description)
                        else:
                            self.console.print("[yellow]No changes requested, keeping current code[/yellow]")

                    self.project_path = self._create_project(current_code)

                    # Generate test flow
                    flow_yaml_path = self._generate_test_flow(current_code)
                    self.console.print(f"\n[cyan]Generated test flow:[/cyan] {flow_yaml_path}")

                    # Ask if user wants to run interactive flow test
                    if Confirm.ask(
                        "\n[bold cyan]Run interactive flow test? (run-flow)[/bold cyan]",
                        default=True,
                        console=self.console
                    ):
                        self._run_flow_test(flow_yaml_path)

                    # Create a dummy passing test result
                    from .models import SingleTestResult
                    test_result = TestResult(
                        total=1,
                        passed=1,
                        failed=0,
                        pass_rate=100.0,
                        tests=[SingleTestResult(name="flow_tested", passed=True)]
                    )

                    round_data = GenerationRound(
                        round_number=round_num,
                        code=current_code,
                        test_result=test_result,
                        optimization_note="Flow-based testing" if round_num == 1 else "User feedback"
                    )
                    self.rounds.append(round_data)

                    # Ask user if satisfied
                    self.console.print("\n[yellow]Are you satisfied with the agent?[/yellow]")
                    self.console.print("  1. Yes, complete")
                    self.console.print("  2. No, need to fix issues")

                    choice = Prompt.ask(
                        "Choice",
                        choices=["1", "2"],
                        default="1",
                        console=self.console
                    )

                    if choice == "1":
                        self.console.print("[green]Complete[/green]")
                        break
                    # If choice == "2", continue to next round

                # After skip_testing loop ends, allow version selection if multiple rounds
                if len(self.rounds) > 1:
                    select_version = Confirm.ask(
                        "\nWould you like to select a specific version?",
                        default=True,
                        console=self.console
                    )
                    if select_version:
                        return self._handle_pause()
            else:
                # Normal testing loop
                round_num = 0
                max_rounds = self.config.max_optimization_rounds

                # If max_rounds is 0, run indefinitely until tests pass or user interrupts
                while True:
                    round_num += 1

                    # Check if we've exceeded max rounds (only if max_rounds > 0)
                    if max_rounds > 0 and round_num > max_rounds:
                        self.console.print(f"[yellow]Reached maximum rounds ({max_rounds})[/yellow]")
                        break

                    self.console.print(f"[cyan]Round {round_num}[/cyan]")

                    # Generate or regenerate code
                    if round_num == 1:
                        current_code = self._generate_initial_code()
                    else:
                        current_code = self._regenerate_code(current_code, test_result)

                    # Create/update project
                    self.project_path = self._create_project(current_code)

                    # Generate test flow
                    flow_yaml_path = self._generate_test_flow(current_code)
                    self.console.print(f"\n[cyan]Generated test flow:[/cyan] {flow_yaml_path}")

                    # Run interactive flow test
                    if Confirm.ask(
                        "\n[bold cyan]Run interactive flow test? (run-flow)[/bold cyan]",
                        default=False,
                        console=self.console
                    ):
                        self._run_flow_test(flow_yaml_path)

                    # Ask if user wants to run unit tests
                    if Confirm.ask(
                        "\n[bold cyan]Run unit tests?[/bold cyan]",
                        default=False,
                        console=self.console
                    ):
                        # Run unit tests
                        test_result = self._run_tests()
                    else:
                        # Skip unit tests - create a passing result
                        self.console.print("[yellow]Skipping unit tests[/yellow]")
                        test_result = TestResult(
                            total=0,
                            passed=0,
                            failed=0,
                            pass_rate=100.0,
                            tests=[]
                        )

                    # Save round
                    round_data = GenerationRound(
                        round_number=round_num,
                        code=current_code,
                        test_result=test_result,
                        optimization_note="" if round_num == 1 else "Auto-optimization"
                    )
                    self.rounds.append(round_data)

                    # Display result
                    self._display_round_result(test_result)

                    # Check if all tests passed
                    if test_result.all_passed:
                        self.console.print("[green]Tests passed[/green]")
                        break

                    # If tests failed, ask user if they want to modify tests
                    if not test_result.all_passed:
                        self.console.print("\n[yellow]Tests failed. What would you like to do?[/yellow]")
                        self.console.print("  1. Continue optimization")
                        self.console.print("  2. Modify tests")
                        self.console.print("  3. Stop and select version")

                        modify_choice = Prompt.ask(
                            "Choice",
                            choices=["1", "2", "3"],
                            default="1",
                            console=self.console
                        )

                        if modify_choice == "2":
                            if self._modify_tests_interactive():
                                # Tests were modified, regenerate code with new tests
                                self.console.print("[cyan]Regenerating code with updated tests...[/cyan]")
                                continue
                        elif modify_choice == "3":
                            self.console.print("[yellow]Stopping optimization[/yellow]")
                            break

        except KeyboardInterrupt:
            self.console.print("\n[yellow]Interrupted[/yellow]")
            return self._handle_pause()

        # If user stopped early or tests didn't pass, let them select a version
        if not test_result or not test_result.all_passed:
            if len(self.rounds) > 1:
                # Multiple rounds exist, let user choose
                select_version = Confirm.ask(
                    "\nWould you like to select a specific version?",
                    default=True,
                    console=self.console
                )
                if select_version:
                    return self._handle_pause()

        # Create final result
        return GenerationResult(
            success=test_result.all_passed if test_result else False,
            agent_name=self.agent_name,
            agent_path=self.project_path,
            rounds=self.rounds,
            test_suite=self.test_suite,
            final_code=current_code,
            final_test_result=test_result
        )

    def _generate_initial_code(self) -> AgentCode:
        """Generate initial agent code"""
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=self.console,
            transient=True
        ) as progress:
            task = progress.add_task("Generating code...", total=None)

            code_str = self.llm.generate_code(
                requirement=self.requirement,
                test_cases_yaml=self.test_suite.to_yaml(),
                agent_name=self.agent_name
            )

            # Clean up code (remove markdown)
            code_str = self._clean_code_response(code_str)

        return AgentCode(main_py=code_str, agent_name=self.agent_name)

    def _regenerate_code(self, previous_code: AgentCode, test_result: TestResult) -> AgentCode:
        """Regenerate code based on test failures"""
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=self.console,
            transient=True
        ) as progress:
            task = progress.add_task("Optimizing code...", total=None)

            failure_info = self.debug_runner.format_failures(test_result)

            code_str = self.llm.regenerate_code(
                original_code=previous_code.main_py,
                test_failures=failure_info,
                requirement=self.requirement
            )

            code_str = self._clean_code_response(code_str)

        return AgentCode(main_py=code_str, agent_name=self.agent_name)

    def _regenerate_code_with_feedback(self, previous_code: AgentCode, feedback: str) -> AgentCode:
        """Regenerate code based on user feedback"""
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=self.console,
            transient=True
        ) as progress:
            task = progress.add_task("Fixing based on feedback...", total=None)

            code_str = self.llm.regenerate_code(
                original_code=previous_code.main_py,
                test_failures=f"User feedback: {feedback}",
                requirement=self.requirement
            )

            code_str = self._clean_code_response(code_str)

        return AgentCode(main_py=code_str, agent_name=self.agent_name)

    def _clean_code_response(self, code_str: str) -> str:
        """Remove markdown code blocks and explanatory text from LLM response"""
        # If there's a markdown code block, extract only the code inside
        code_block_pattern = r'```(?:python)?\n(.*?)```'
        match = re.search(code_block_pattern, code_str, re.DOTALL)
        if match:
            code_str = match.group(1)
        else:
            # No markdown blocks, remove them if they exist without proper closing
            code_str = re.sub(r'```python\n', '', code_str)
            code_str = re.sub(r'```\n?', '', code_str)

        # Remove any text after the final Python code block
        # Look for the end of the main block: 'if __name__ == "__main__":\n    main()'
        # and remove everything after it
        main_block_pattern = r'(if __name__ == ["\']__main__["\']:\s+main\(\s*\))\s*\n\s*(.+)'
        code_str = re.sub(main_block_pattern, r'\1', code_str, flags=re.DOTALL)

        return code_str.strip()

    def _create_project(self, code: AgentCode) -> str:
        """Create or update project files"""
        project_path = self.scaffolder.create_project(
            agent_name=self.agent_name,
            code=code.main_py,
            test_yaml=self.test_suite.to_yaml(),
            dependencies=code.dependencies
        )
        return project_path

    def _generate_test_flow(self, agent_code: AgentCode) -> str:
        """Generate a test flow YAML for the agent with terminal-input"""
        import yaml
        from mofa.debug.load_node import extract_agent_info

        # Extract agent info to get input/output parameter names
        agent_info = extract_agent_info(agent_code.main_py)
        receive_params = agent_info.get('receive_params', [])
        send_params = agent_info.get('send_params', [])

        # Extract parameter names
        input_param = None
        if receive_params:
            # Parse 'parameter_name' or parameter_names=['param1', 'param2']
            param_str = receive_params[0].strip("'\"")
            input_param = param_str

        output_param = None
        if send_params:
            # Parse agent_output_name='output_name'
            for param in send_params:
                if 'agent_output_name' in param:
                    output_param = param.split('=')[1].strip().strip("'\"")
                    break

        if not input_param or not output_param:
            raise ValueError(f"Could not extract input/output parameters from agent code")

        # Generate flow YAML
        flow_data = {
            'nodes': [
                {
                    'id': 'terminal-input',
                    'build': f'pip install -e ../../agents/terminal-input',
                    'path': 'dynamic',
                    'outputs': ['data'],
                    'inputs': {
                        'agent_response': f'{self.agent_name}/{output_param}'
                    }
                },
                {
                    'id': self.agent_name,
                    'build': f'pip install -e ../../agents/{self.agent_name}',
                    'path': self.agent_name,
                    'outputs': [output_param],
                    'inputs': {
                        input_param: 'terminal-input/data'
                    },
                    'env': {
                        'IS_DATAFLOW_END': True,
                        'WRITE_LOG': True
                    }
                }
            ]
        }

        # Create flows directory if it doesn't exist
        flows_dir = Path(self.config.output_dir).parent / 'flows'
        flows_dir.mkdir(exist_ok=True)

        # Create flow directory
        flow_dir = flows_dir / f'{self.agent_name}-flow'
        flow_dir.mkdir(exist_ok=True)

        # Write flow YAML
        flow_yaml_path = flow_dir / f'{self.agent_name}_dataflow.yml'
        with open(flow_yaml_path, 'w') as f:
            yaml.dump(flow_data, f, default_flow_style=False, sort_keys=False)

        return str(flow_yaml_path)

    def _run_flow_test(self, flow_yaml_path: str):
        """Run interactive flow test using run-flow command"""
        import subprocess

        self.console.print("\n" + "=" * 60)
        self.console.print("[bold cyan]Interactive Flow Testing[/bold cyan]")
        self.console.print("=" * 60)
        self.console.print("Testing the agent in a real flow with terminal-input...")
        self.console.print("=" * 60 + "\n")

        try:
            # Run the flow interactively (not vibe-test mode, just normal run-flow)
            result = subprocess.run(
                ['mofa', 'run-flow', flow_yaml_path],
                cwd=os.getcwd(),
                text=True
            )

            if result.returncode != 0:
                self.console.print("\n[yellow]Flow test encountered issues[/yellow]")
            else:
                self.console.print("\n[green]Flow test completed[/green]")

        except KeyboardInterrupt:
            self.console.print("\n[yellow]Flow test interrupted by user[/yellow]")
        except Exception as e:
            self.console.print(f"\n[red]Error running flow test: {e}[/red]")

    def _run_tests(self) -> TestResult:
        """Run mofa unit-test tests"""
        test_yaml_path = os.path.join(
            self.project_path,
            "tests",
            f"test_{self.agent_name.replace('-', '_')}.yml"
        )

        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=self.console,
            transient=True
        ) as progress:
            task = progress.add_task("Testing...", total=None)

            result = self.debug_runner.run_tests(self.project_path, test_yaml_path)

        return result

    def _display_round_result(self, result: TestResult):
        """Display test results for a round"""
        status_mark = "PASS" if result.all_passed else "FAIL"
        status_color = "green" if result.all_passed else "red"

        self.console.print(
            f"  [{status_color}]{status_mark}[/{status_color}] "
            f"{result.passed}/{result.total} ({result.pass_rate:.0f}%)"
        )

        # Show failed tests
        if not result.all_passed:
            for test in result.get_failed_tests():
                self.console.print(f"  [dim]- {test.name}[/dim]")

    def _handle_pause(self) -> GenerationResult:
        """Handle user pause - allow selecting version"""
        if not self.rounds:
            self.console.print("[red]No versions generated yet[/red]")
            return None

        # Show version history
        self.console.print("\n[bold]Version History:[/bold]")
        table = Table()
        table.add_column("Version", style="cyan")
        table.add_column("Pass Rate", style="white")
        table.add_column("Status", style="white")

        for round_data in self.rounds:
            status = "PASS" if round_data.test_result.all_passed else "FAIL"
            table.add_row(
                f"Round {round_data.round_number}",
                f"{round_data.test_result.pass_rate:.1f}%",
                status
            )

        self.console.print(table)

        # Ask which version to use
        version_num = Prompt.ask(
            "\nSelect version to use (enter round number)",
            default=str(len(self.rounds)),
            console=self.console
        )

        try:
            selected_round = self.rounds[int(version_num) - 1]

            # Update project with selected version
            self.project_path = self._create_project(selected_round.code)

            # Clean up backup files after selection
            self._cleanup_backups()

            return GenerationResult(
                success=selected_round.test_result.all_passed,
                agent_name=self.agent_name,
                agent_path=self.project_path,
                rounds=self.rounds,
                test_suite=self.test_suite,
                final_code=selected_round.code,
                final_test_result=selected_round.test_result
            )

        except (ValueError, IndexError):
            self.console.print("[red]Invalid version number[/red]")
            return self._handle_pause()

    def _modify_tests_interactive(self) -> bool:
        """
        Allow user to interactively modify test cases
        Returns True if tests were modified, False otherwise
        """
        self.console.print("\n[bold]Current Test Cases:[/bold]")

        # Display current tests
        current_yaml = self.test_suite.to_yaml()
        syntax = Syntax(current_yaml, "yaml", theme="monokai", line_numbers=True)
        self.console.print(syntax)
        self.console.print()

        # Ask what to do
        self.console.print("What would you like to do?")
        self.console.print("  1. Chat with AI to modify tests")
        self.console.print("  2. Manually edit YAML")
        self.console.print("  3. Regenerate with additional requirements")
        self.console.print("  4. Cancel")

        action = Prompt.ask(
            "Choice",
            choices=["1", "2", "3", "4"],
            default="1",
            console=self.console
        )

        if action == "4":
            return False

        if action == "1":
            # Conversational editing with LLM
            return self._modify_tests_conversational()

        if action == "3":
            # Regenerate tests with additional requirements
            additional_req = Prompt.ask(
                "[yellow]Additional test requirements (or press Enter to skip)[/yellow]",
                default="",
                console=self.console
            )

            if additional_req:
                self.requirement += f"\n\nAdditional test requirements: {additional_req}"

            # Regenerate test cases
            self.console.print("[dim]Regenerating test cases...[/dim]")
            test_yaml = self.llm.generate_test_cases(self.requirement)
            test_yaml = self._clean_yaml_response(test_yaml)

            # Show new tests
            self.console.print("\n[bold]New Test Cases:[/bold]")
            syntax = Syntax(test_yaml, "yaml", theme="monokai", line_numbers=False)
            self.console.print(syntax)
            self.console.print()

            if Confirm.ask("Use these new tests?", default=True, console=self.console):
                self.test_suite = TestSuite.from_yaml(test_yaml)
                return True
            else:
                return False

        elif action == "2":
            # Manual editing
            self.console.print("\n[yellow]Enter new YAML content (type 'END' on a new line to finish):[/yellow]")
            self.console.print("[dim]Tip: Copy current YAML above and modify it[/dim]\n")

            lines = []
            while True:
                try:
                    line = input()
                    if line.strip() == "END":
                        break
                    lines.append(line)
                except EOFError:
                    break

            if not lines:
                self.console.print("[yellow]No changes made[/yellow]")
                return False

            new_yaml = "\n".join(lines)

            # Try to parse new YAML
            try:
                new_test_suite = TestSuite.from_yaml(new_yaml)
                self.test_suite = new_test_suite
                self.console.print("[green]Tests updated successfully[/green]")
                return True
            except Exception as e:
                self.console.print(f"[red]Error parsing YAML: {e}[/red]")
                return False

        return False

    def _modify_tests_conversational(self) -> bool:
        """
        Modify tests through conversational interaction with LLM
        Returns True if tests were modified, False otherwise
        """
        self.console.print("\n[bold cyan]Conversational Test Editor[/bold cyan]")
        self.console.print("[dim]Chat with AI to modify your tests. Type 'done' when finished, 'cancel' to abort.[/dim]\n")

        conversation_history = []
        current_yaml = self.test_suite.to_yaml()

        while True:
            # Get user instruction
            user_input = Prompt.ask("[bold]You[/bold]", console=self.console)

            if user_input.lower() in ['done', 'finish', 'ok']:
                self.console.print("[green]Conversation complete[/green]")
                break
            elif user_input.lower() in ['cancel', 'abort', 'quit']:
                self.console.print("[yellow]Cancelled, keeping original tests[/yellow]")
                return False

            # Add user message to history
            conversation_history.append({"role": "user", "content": user_input})

            # Call LLM to modify tests
            with Progress(
                SpinnerColumn(),
                TextColumn("[progress.description]{task.description}"),
                console=self.console,
                transient=True
            ) as progress:
                task = progress.add_task("AI is thinking...", total=None)

                try:
                    modified_yaml = self.llm.modify_test_cases_conversational(
                        current_yaml=current_yaml,
                        user_instruction=user_input,
                        requirement=self.requirement,
                        conversation_history=conversation_history[:-1]  # Exclude the current message
                    )

                    # Clean up response
                    modified_yaml = self._clean_yaml_response(modified_yaml)

                except Exception as e:
                    self.console.print(f"[red]Error: {e}[/red]")
                    conversation_history.pop()  # Remove failed message
                    continue

            # Add assistant response to history
            conversation_history.append({"role": "assistant", "content": modified_yaml})

            # Show modified tests
            self.console.print("\n[bold]Modified Test Cases:[/bold]")
            syntax = Syntax(modified_yaml, "yaml", theme="monokai", line_numbers=False)
            self.console.print(syntax)
            self.console.print()

            # Validate YAML
            try:
                test_suite = TestSuite.from_yaml(modified_yaml)
                current_yaml = modified_yaml  # Update for next iteration
                self.console.print("[dim green]Valid YAML ✓[/dim green]\n")
            except Exception as e:
                self.console.print(f"[yellow]Warning: YAML validation failed: {e}[/yellow]")
                self.console.print("[dim]You can continue chatting to fix this[/dim]\n")
                conversation_history.pop()  # Remove invalid response
                continue

        # Ask for final confirmation
        self.console.print("\n[bold]Final Test Cases:[/bold]")
        syntax = Syntax(current_yaml, "yaml", theme="monokai", line_numbers=False)
        self.console.print(syntax)
        self.console.print()

        if Confirm.ask("Apply these changes?", default=True, console=self.console):
            try:
                self.test_suite = TestSuite.from_yaml(current_yaml)
                self.console.print("[green]Tests updated successfully[/green]")
                return True
            except Exception as e:
                self.console.print(f"[red]Error applying changes: {e}[/red]")
                return False
        else:
            self.console.print("[yellow]Changes discarded[/yellow]")
            return False

    def _cleanup_backups(self):
        """Clean up backup directories for current agent"""
        if not self.agent_name:
            return

        # Find all backup directories for this agent
        agent_dir = self.output_dir / self.agent_name
        backup_pattern = f"{self.agent_name}_backup_*"

        backups_removed = 0
        for backup_dir in self.output_dir.glob(backup_pattern):
            if backup_dir.is_dir():
                shutil.rmtree(backup_dir, ignore_errors=True)
                backups_removed += 1

        if backups_removed > 0:
            self.console.print(f"[dim]Cleaned up {backups_removed} backup(s)[/dim]")

    def _show_final_summary(self, result: GenerationResult):
        """Display final summary"""
        if not result:
            return

        # Clean up backup files
        self._cleanup_backups()

        self.console.print()
        if result.success:
            self.console.print("[bold green]Generation successful[/bold green]")
        else:
            self.console.print("[yellow]Generation complete (some tests failed)[/yellow]")

        self.console.print()

        # Summary info - simpler format
        self.console.print(f"[cyan]Agent:[/cyan] {result.agent_name}")
        self.console.print(f"[cyan]Path:[/cyan] {result.agent_path}")

        if not self.skip_testing:
            self.console.print(
                f"[cyan]Tests:[/cyan] {result.final_test_result.passed}/{result.final_test_result.total} passed"
            )
            if result.total_rounds > 1:
                self.console.print(f"[cyan]Iterations:[/cyan] {result.total_rounds} rounds")

        self.console.print()
