"""Debug runner that wraps mofa debug command"""

import subprocess
import re
from pathlib import Path
from .models import TestResult, SingleTestResult


class DebugRunner:
    """Runs mofa debug and parses results"""

    def run_tests(self, agent_path: str, test_yaml: str) -> TestResult:
        """
        Run mofa debug command and parse output

        Args:
            agent_path: Path to agent directory
            test_yaml: Path to test YAML file

        Returns:
            TestResult with parsed test outcomes
        """
        try:
            # Run mofa debug command
            result = subprocess.run(
                ['mofa', 'debug', agent_path, test_yaml],
                capture_output=True,
                text=True,
                timeout=60
            )

            # Parse the output
            return self._parse_output(result.stdout, result.stderr)

        except subprocess.TimeoutExpired:
            return TestResult(
                total=0,
                passed=0,
                failed=0,
                pass_rate=0.0,
                tests=[]
            )
        except Exception as e:
            print(f"Error running debug: {e}")
            return TestResult(
                total=0,
                passed=0,
                failed=0,
                pass_rate=0.0,
                tests=[]
            )

    def _parse_output(self, stdout: str, stderr: str) -> TestResult:
        """
        Parse mofa debug output to extract test results

        Expected output format:
        Test case 1/3: test_name
        Status: ✅ Passed
        ----------------------------------
        ...
        Total test cases: 3
        Passed: 2
        Failed: 1
        Pass rate: 66.67%
        """
        tests = []
        total = 0
        passed = 0
        failed = 0
        pass_rate = 0.0

        # Parse individual test results
        test_pattern = r'Test case \d+/\d+: (.+?)\nStatus: (✅|❌) (Passed|Failed)'
        for match in re.finditer(test_pattern, stdout):
            test_name = match.group(1).strip()
            status = match.group(2)
            is_passed = status == '✅'

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

        return TestResult(
            total=total,
            passed=passed,
            failed=failed,
            pass_rate=pass_rate,
            tests=tests
        )

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
