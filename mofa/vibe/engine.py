"""Core Vibe engine for agent generation"""

import os
import re
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
        self.requirement = Prompt.ask("[bold]Agent功能描述[/bold]", console=self.console)

        # Generate agent name from requirement using LLM
        self.console.print("[dim]生成agent名称中...[/dim]")
        self.agent_name = self.llm.generate_agent_name(self.requirement)

        # Ask if user wants to customize name
        suggested_name = Prompt.ask(
            f"[bold]Agent名称[/bold] [dim](建议: {self.agent_name})[/dim]",
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
            task = progress.add_task("生成测试用例中...", total=None)

            # Generate test cases using LLM
            test_yaml = self.llm.generate_test_cases(self.requirement)

            # Clean up any markdown code blocks
            test_yaml = self._clean_yaml_response(test_yaml)

            progress.update(task, completed=True)

        # Display generated tests
        self.console.print()
        self.console.print("[bold]生成的测试用例[/bold]")
        self.console.print()

        syntax = Syntax(test_yaml, "yaml", theme="monokai", line_numbers=False)
        self.console.print(syntax)
        self.console.print()

        # Ask for confirmation
        while True:
            self.console.print("[dim]选项: y=使用 / n=重新生成 / skip=跳过测试[/dim]")
            response = Prompt.ask(
                "确认",
                choices=["y", "n", "skip"],
                default="y",
                console=self.console,
                show_choices=False
            )

            if response == "y":
                self.test_suite = TestSuite.from_yaml(test_yaml)
                break
            elif response == "skip":
                self.console.print("[dim]跳过测试，仅生成代码[/dim]")
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
                    "[yellow]补充测试要求[/yellow]",
                    default="",
                    console=self.console
                )
                if additional_req:
                    self.requirement += f"\n\n测试要求: {additional_req}"
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
            self.console.print(f"[yellow]警告: YAML解析错误，尝试修复...[/yellow]")
            # Additional cleanup attempts
            # Remove any remaining * expressions in lists
            yaml_str = re.sub(r'\[\s*["\'](\w+)["\']\s*\*\s*\d+\s*\]', r'["\1"]', yaml_str)

            # Try parsing again
            try:
                yaml.safe_load(yaml_str)
            except yaml.YAMLError:
                # If still fails, we'll let it fail naturally and user can regenerate
                self.console.print("[yellow]自动修复失败，可能需要重新生成[/yellow]")

        return yaml_str

    def _auto_optimize_loop(self) -> GenerationResult:
        """Automatic generation and optimization loop"""
        self.console.print()
        self.console.print("[bold]开始生成[/bold]")

        if self.skip_testing:
            self.console.print("[dim]测试已跳过[/dim]")
        else:
            self.console.print("[dim]Ctrl+C 可中断[/dim]")

        self.console.print()

        current_code = None
        test_result = None

        try:
            # If skip_testing, just generate code once without optimization
            if self.skip_testing:
                current_code = self._generate_initial_code()
                self.project_path = self._create_project(current_code)

                # Create a dummy passing test result
                from .models import SingleTestResult
                test_result = TestResult(
                    total=1,
                    passed=1,
                    failed=0,
                    pass_rate=100.0,
                    tests=[SingleTestResult(name="skipped", passed=True)]
                )

                round_data = GenerationRound(
                    round_number=1,
                    code=current_code,
                    test_result=test_result,
                    optimization_note="跳过测试"
                )
                self.rounds.append(round_data)

                self.console.print("[green]完成[/green]")
            else:
                # Normal testing loop
                round_num = 0
                max_rounds = self.config.max_optimization_rounds

                # If max_rounds is 0, run indefinitely until tests pass or user interrupts
                while True:
                    round_num += 1

                    # Check if we've exceeded max rounds (only if max_rounds > 0)
                    if max_rounds > 0 and round_num > max_rounds:
                        self.console.print(f"[yellow]已达到最大轮次 ({max_rounds})[/yellow]")
                        break

                    self.console.print(f"[cyan]Round {round_num}[/cyan]")

                    # Generate or regenerate code
                    if round_num == 1:
                        current_code = self._generate_initial_code()
                    else:
                        current_code = self._regenerate_code(current_code, test_result)

                    # Create/update project
                    self.project_path = self._create_project(current_code)

                    # Run tests
                    test_result = self._run_tests()

                    # Save round
                    round_data = GenerationRound(
                        round_number=round_num,
                        code=current_code,
                        test_result=test_result,
                        optimization_note="" if round_num == 1 else "自动优化"
                    )
                    self.rounds.append(round_data)

                    # Display result
                    self._display_round_result(test_result)

                    # Check if all tests passed
                    if test_result.all_passed:
                        self.console.print("[green]测试通过[/green]")
                        break

        except KeyboardInterrupt:
            self.console.print("\n[yellow]已中断[/yellow]")
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
            task = progress.add_task("生成代码中", total=None)

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
            task = progress.add_task("优化代码中", total=None)

            failure_info = self.debug_runner.format_failures(test_result)

            code_str = self.llm.regenerate_code(
                original_code=previous_code.main_py,
                test_failures=failure_info,
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

    def _run_tests(self) -> TestResult:
        """Run mofa debug tests"""
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
            task = progress.add_task("测试中", total=None)

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
            self.console.print("[red]还没有生成任何版本[/red]")
            return None

        # Show version history
        self.console.print("\n[bold]版本历史:[/bold]")
        table = Table()
        table.add_column("版本", style="cyan")
        table.add_column("通过率", style="white")
        table.add_column("状态", style="white")

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
            "\n选择要使用的版本 (输入round number)",
            default=str(len(self.rounds)),
            console=self.console
        )

        try:
            selected_round = self.rounds[int(version_num) - 1]

            # Update project with selected version
            self.project_path = self._create_project(selected_round.code)

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
            self.console.print("[red]无效的版本号[/red]")
            return self._handle_pause()

    def _show_final_summary(self, result: GenerationResult):
        """Display final summary"""
        if not result:
            return

        self.console.print()
        if result.success:
            self.console.print("[bold green]生成成功[/bold green]")
        else:
            self.console.print("[yellow]生成完成（部分测试未通过）[/yellow]")

        self.console.print()

        # Summary info - simpler format
        self.console.print(f"[cyan]Agent:[/cyan] {result.agent_name}")
        self.console.print(f"[cyan]路径:[/cyan] {result.agent_path}")

        if not self.skip_testing:
            self.console.print(
                f"[cyan]测试:[/cyan] {result.final_test_result.passed}/{result.final_test_result.total} 通过"
            )
            if result.total_rounds > 1:
                self.console.print(f"[cyan]迭代:[/cyan] {result.total_rounds} 轮")

        self.console.print()
