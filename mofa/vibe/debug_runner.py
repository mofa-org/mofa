"""Debug runner that wraps mofa unit-test command"""

import subprocess
import re
from pathlib import Path
from .models import TestResult, SingleTestResult


class DebugRunner:
    """Runs mofa unit-test and parses results"""

    def run_tests(self, agent_path: str, test_yaml: str) -> TestResult:
        """
        Run mofa unit-test command and parse output

        Args:
            agent_path: Path to agent directory
            test_yaml: Path to test YAML file

        Returns:
            TestResult with parsed test outcomes
        """
        try:
            # Run mofa unit-test command
            result = subprocess.run(
                ['mofa', 'unit-test', agent_path, test_yaml],
                capture_output=True,
                text=True,
                timeout=60
            )

            # Parse the output
            return self._parse_output(result.stdout, result.stderr, result.returncode)

        except subprocess.TimeoutExpired:
            print("[ERROR] Test execution timeout (>60s)")
            return TestResult(
                total=0,
                passed=0,
                failed=0,
                pass_rate=0.0,
                tests=[]
            )
        except Exception as e:
            print(f"[ERROR] Failed to run unit-test: {e}")
            return TestResult(
                total=0,
                passed=0,
                failed=0,
                pass_rate=0.0,
                tests=[]
            )

    def _parse_output(self, stdout: str, stderr: str, returncode: int) -> TestResult:
        """
        Parse mofa unit-test output to extract test results

        Expected output format:
        Test case 1/3: test_name
        Status: [PASS] Passed
        ----------------------------------
        ...
        ========================================
        Test Summary:
        Total test cases: 3
        Passed: 2
        Failed: 1
        Pass rate: 66.67%
        ========================================
        """
        tests = []
        total = 0
        passed = 0
        failed = 0
        pass_rate = 0.0

        # Parse individual test results
        test_pattern = r'Test case \d+/\d+: (.+?)\nStatus: \[(PASS|FAIL)\] (Passed|Failed)'
        for match in re.finditer(test_pattern, stdout):
            test_name = match.group(1).strip()
            status = match.group(2)
            is_passed = status == 'PASS'

            tests.append(SingleTestResult(
                name=test_name,
                passed=is_passed
            ))

        # Parse summary statistics
        total_match = re.search(r'Total test cases: (\d+)', stdout)
        if total_match:
            total = int(total_match.group(1))

        passed_match = re.search(r'Passed: (\d+)', stdout)
        if passed_match:
            passed = int(passed_match.group(1))

        failed_match = re.search(r'Failed: (\d+)', stdout)
        if failed_match:
            failed = int(failed_match.group(1))

        pass_rate_match = re.search(r'Pass rate: ([\d.]+)%', stdout)
        if pass_rate_match:
            pass_rate = float(pass_rate_match.group(1))

        # If parsing failed, try to extract from test count
        if total == 0 and tests:
            total = len(tests)
            passed = sum(1 for t in tests if t.passed)
            failed = total - passed
            pass_rate = (passed / total * 100) if total > 0 else 0.0

        # Debug output when no results parsed
        if total == 0:
            print("\n[DEBUG] Failed to parse test results")
            print(f"[DEBUG] Return code: {returncode}")
            if stderr:
                print(f"[DEBUG] Stderr:\n{stderr}")
            print(f"[DEBUG] Stdout (first 500 chars):\n{stdout[:500]}")

        return TestResult(
            total=total,
            passed=passed,
            failed=failed,
            pass_rate=pass_rate,
            tests=tests
        )

    def run_interactive(self, agent_path: str) -> bool:
        """
        Run mofa run-node command for interactive manual testing

        Args:
            agent_path: Path to agent directory

        Returns:
            True if user wants to continue, False to skip
        """
        try:
            print("\n" + "=" * 60)
            print("Interactive Manual Testing (run-node)")
            print("=" * 60)
            print("You can now manually test the agent with your own inputs.")
            print("The agent will execute and show you the output.")
            print("=" * 60 + "\n")

            # Run mofa run-node in interactive mode
            # Note: This will inherit stdin/stdout for user interaction
            result = subprocess.run(
                ['mofa', 'run-node', agent_path],
                timeout=120  # 2 minutes timeout
            )

            return True  # Continue after manual testing

        except subprocess.TimeoutExpired:
            print("\n[ERROR] Interactive testing timeout")
            return True
        except KeyboardInterrupt:
            print("\n[INFO] Interactive testing interrupted by user")
            return True
        except Exception as e:
            print(f"\n[ERROR] Failed to run interactive test: {e}")
            return True

    def format_failures(self, test_result: TestResult) -> str:
        """Format failed tests for LLM consumption"""
        failed_tests = test_result.get_failed_tests()

        if not failed_tests:
            return "All tests passed!"

        failure_text = f"Failed {len(failed_tests)} out of {test_result.total} tests:\n\n"

        for test in failed_tests:
            failure_text += f"Test: {test.name}\n"
            if test.error_message:
                failure_text += f"Error: {test.error_message}\n"
            if test.expected and test.actual:
                failure_text += f"Expected: {test.expected}\n"
                failure_text += f"Got: {test.actual}\n"
            failure_text += "\n"

        return failure_text
