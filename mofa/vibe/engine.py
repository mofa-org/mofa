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
        self.console.print(Panel(
            "[bold cyan]🤖 MoFA Vibe - AI Agent Generator[/bold cyan]\n"
            "Automatically generate agents from requirements",
            border_style="cyan"
        ))
        self.console.print()

    def _ask_requirement(self):
        """Ask user for agent requirement"""
        self.console.print("📝 [bold]请描述你想要的Agent功能:[/bold]")
        self.requirement = Prompt.ask(">", console=self.console)

        # Generate agent name from requirement
        self.agent_name = self._generate_agent_name(self.requirement)

        # Ask if user wants to customize name
        self.console.print(f"\n💡 建议的Agent名称: [cyan]{self.agent_name}[/cyan]")
        if not Confirm.ask("使用这个名称?", default=True, console=self.console):
            self.agent_name = Prompt.ask("请输入Agent名称", console=self.console)

    def _generate_agent_name(self, requirement: str) -> str:
        """Generate agent name from requirement"""
        # Simple heuristic: take first few words and slugify
        words = requirement.lower().split()[:3]
        name = '-'.join(words)
        # Remove special characters
        name = re.sub(r'[^a-z0-9-]', '', name)
        return name or "custom-agent"

    def _generate_and_confirm_tests(self):
        """Generate test cases and get user confirmation"""
        self.console.print("\n✨ [bold]正在分析并生成测试用例...[/bold]")

        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=self.console,
            transient=True
        ) as progress:
            task = progress.add_task("生成测试用例", total=None)

            # Generate test cases using LLM
            test_yaml = self.llm.generate_test_cases(self.requirement)

            # Clean up any markdown code blocks
            test_yaml = self._clean_yaml_response(test_yaml)

            progress.update(task, completed=True)

        # Display generated tests
        self.console.print("\n" + "━" * 60)
        self.console.print("📋 [bold]生成的测试用例:[/bold]")
        self.console.print("━" * 60)
        self.console.print()

        syntax = Syntax(test_yaml, "yaml", theme="monokai", line_numbers=False)
        self.console.print(syntax)
        self.console.print()

        # Ask for confirmation
        while True:
            response = Prompt.ask(
                "这些测试用例可以吗?",
                choices=["y", "n", "edit"],
                default="y",
                console=self.console
            )

            if response == "y":
                self.test_suite = TestSuite.from_yaml(test_yaml)
                break
            elif response == "edit":
                self.console.print("\n请编辑测试用例（完成后输入空行）:")
                lines = []
                while True:
                    line = input()
                    if line == "" and lines:
                        break
                    lines.append(line)
                test_yaml = "\n".join(lines)
                self.test_suite = TestSuite.from_yaml(test_yaml)
                break
            else:  # n
                self.console.print("⚠️  请手动描述你想要的测试用例:")
                manual_tests = Prompt.ask(">", console=self.console)
                self.requirement += f"\n\n测试要求: {manual_tests}"
                # Regenerate
                return self._generate_and_confirm_tests()

    def _clean_yaml_response(self, yaml_str: str) -> str:
        """Remove markdown code blocks from LLM response"""
        # Remove ```yaml and ``` markers
        yaml_str = re.sub(r'```yaml\n', '', yaml_str)
        yaml_str = re.sub(r'```\n?', '', yaml_str)
        return yaml_str.strip()

    def _auto_optimize_loop(self) -> GenerationResult:
        """Automatic generation and optimization loop"""
        self.console.print("\n" + "━" * 60)
        self.console.print("🚀 [bold]开始自动生成和优化[/bold]")
        self.console.print("━" * 60)
        self.console.print("[dim]提示: 按 Ctrl+C 暂停，可选择保存当前版本[/dim]\n")

        current_code = None
        test_result = None

        try:
            for round_num in range(1, self.config.max_optimization_rounds + 1):
                self.console.print(f"\n[bold cyan]Round {round_num}[/bold cyan] {'━' * 40}")

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
                    self.console.print(f"\n[bold green]✅ 所有测试通过！优化完成[/bold green]")
                    break

        except KeyboardInterrupt:
            self.console.print("\n\n⏸️  [yellow]用户暂停[/yellow]")
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
            task = progress.add_task("📝 生成代码...", total=None)

            code_str = self.llm.generate_code(
                requirement=self.requirement,
                test_cases_yaml=self.test_suite.to_yaml(),
                agent_name=self.agent_name
            )

            # Clean up code (remove markdown)
            code_str = self._clean_code_response(code_str)

            progress.update(task, description="📝 生成代码... ✓")

        return AgentCode(main_py=code_str, agent_name=self.agent_name)

    def _regenerate_code(self, previous_code: AgentCode, test_result: TestResult) -> AgentCode:
        """Regenerate code based on test failures"""
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=self.console,
            transient=True
        ) as progress:
            task1 = progress.add_task("💡 分析错误...", total=None)

            failure_info = self.debug_runner.format_failures(test_result)

            progress.update(task1, description="💡 分析错误... ✓")
            task2 = progress.add_task("🔧 优化代码...", total=None)

            code_str = self.llm.regenerate_code(
                original_code=previous_code.main_py,
                test_failures=failure_info,
                requirement=self.requirement
            )

            code_str = self._clean_code_response(code_str)

            progress.update(task2, description="🔧 优化代码... ✓")

        return AgentCode(main_py=code_str, agent_name=self.agent_name)

    def _clean_code_response(self, code_str: str) -> str:
        """Remove markdown code blocks from LLM response"""
        # Remove ```python and ``` markers
        code_str = re.sub(r'```python\n', '', code_str)
        code_str = re.sub(r'```\n?', '', code_str)
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
            task = progress.add_task("🔍 运行测试...", total=None)

            result = self.debug_runner.run_tests(self.project_path, test_yaml_path)

            progress.update(task, description="🔍 运行测试... ✓")

        return result

    def _display_round_result(self, result: TestResult):
        """Display test results for a round"""
        status_emoji = "✓" if result.all_passed else "✗"
        status_color = "green" if result.all_passed else "red"

        self.console.print(
            f"  📊 Pass: {result.passed}/{result.total} "
            f"({result.pass_rate:.1f}%) [{status_color}]{status_emoji}[/{status_color}]"
        )

        # Show failed tests
        if not result.all_passed:
            for test in result.get_failed_tests():
                self.console.print(f"  [red]❌ {test.name}[/red]")

    def _handle_pause(self) -> GenerationResult:
        """Handle user pause - allow selecting version"""
        if not self.rounds:
            self.console.print("❌ 还没有生成任何版本")
            return None

        # Show version history
        self.console.print("\n[bold]版本历史:[/bold]")
        table = Table()
        table.add_column("版本", style="cyan")
        table.add_column("通过率", style="white")
        table.add_column("状态", style="white")

        for round_data in self.rounds:
            status = "✓" if round_data.test_result.all_passed else "✗"
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
            self.console.print("❌ 无效的版本号")
            return self._handle_pause()

    def _show_final_summary(self, result: GenerationResult):
        """Display final summary"""
        if not result:
            return

        self.console.print("\n" + "━" * 60)
        if result.success:
            self.console.print("[bold green]🎉 生成成功！[/bold green]")
        else:
            self.console.print("[bold yellow]⚠️  生成完成（部分测试未通过）[/bold yellow]")
        self.console.print("━" * 60)
        self.console.print()

        # Summary info
        info_table = Table.grid(padding=(0, 2))
        info_table.add_column(style="cyan bold")
        info_table.add_column(style="white")

        info_table.add_row("✅ Agent:", result.agent_name)
        info_table.add_row("📂 位置:", result.agent_path)
        info_table.add_row(
            "📊 通过率:",
            f"{result.final_test_result.pass_rate:.1f}% "
            f"({result.final_test_result.passed}/{result.final_test_result.total})"
        )
        info_table.add_row("🔄 优化轮次:", str(result.total_rounds))

        self.console.print(info_table)
        self.console.print()

        # Version history
        if len(result.rounds) > 1:
            self.console.print("[bold]版本历史:[/bold]")
            for round_data in result.rounds:
                status = "⭐" if round_data == result.rounds[-1] else "  "
                emoji = "✓" if round_data.test_result.all_passed else "✗"
                self.console.print(
                    f"  {status} [{round_data.round_number}] Round {round_data.round_number} - "
                    f"{round_data.test_result.pass_rate:.1f}% passed {emoji}"
                )
            self.console.print()

        self.console.print(f"[dim]使用版本: Round {result.rounds[-1].round_number}[/dim]\n")
