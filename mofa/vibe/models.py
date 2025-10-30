"""Data models for MoFA Vibe"""

from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional
from datetime import datetime
import yaml


@dataclass
class TestCase:
    """Single test case"""
    name: str
    input: Dict[str, Any]
    expected_output: Optional[Dict[str, Any]] = None
    validation: Optional[Dict[str, Any]] = None


@dataclass
class TestSuite:
    """Collection of test cases"""
    cases: List[TestCase]

    def to_yaml(self) -> str:
        """Convert to YAML format for mofa unit-test"""
        test_cases = []
        for case in self.cases:
            case_dict = {
                'name': case.name,
                'input': case.input,
            }
            if case.expected_output:
                case_dict['expected_output'] = case.expected_output
            if case.validation:
                case_dict['validation'] = case.validation
            test_cases.append(case_dict)

        data = {'test_cases': test_cases}
        return yaml.dump(data, allow_unicode=True, default_flow_style=False)

    @classmethod
    def from_yaml(cls, yaml_str: str) -> 'TestSuite':
        """Load from YAML string"""
        data = yaml.safe_load(yaml_str)
        cases = [
            TestCase(
                name=case['name'],
                input=case['input'],
                expected_output=case.get('expected_output'),
                validation=case.get('validation')
            )
            for case in data['test_cases']
        ]
        return cls(cases=cases)


@dataclass
class SingleTestResult:
    """Result of a single test case"""
    name: str
    passed: bool
    expected: Any = None
    actual: Any = None
    error_message: Optional[str] = None


@dataclass
class TestResult:
    """Overall test results"""
    total: int
    passed: int
    failed: int
    pass_rate: float
    tests: List[SingleTestResult] = field(default_factory=list)

    @property
    def all_passed(self) -> bool:
        return self.passed == self.total

    def get_failed_tests(self) -> List[SingleTestResult]:
        return [t for t in self.tests if not t.passed]


@dataclass
class AgentCode:
    """Generated agent code"""
    main_py: str
    dependencies: List[str] = field(default_factory=list)
    agent_name: str = ""

    def get_imports(self) -> List[str]:
        """Extract import statements from code"""
        imports = []
        for line in self.main_py.split('\n'):
            line = line.strip()
            if line.startswith('import ') or line.startswith('from '):
                imports.append(line)
        return imports


@dataclass
class GenerationRound:
    """Single generation/optimization round"""
    round_number: int
    code: AgentCode
    test_result: TestResult
    optimization_note: str = ""
    timestamp: datetime = field(default_factory=datetime.now)

    @property
    def summary(self) -> str:
        status = "PASS" if self.test_result.all_passed else "FAIL"
        return f"Round {self.round_number} - {self.test_result.pass_rate:.1f}% passed [{status}]"


@dataclass
class GenerationResult:
    """Final result of agent generation"""
    success: bool
    agent_name: str
    agent_path: str
    rounds: List[GenerationRound]
    test_suite: TestSuite
    final_code: AgentCode
    final_test_result: TestResult

    @property
    def total_rounds(self) -> int:
        return len(self.rounds)

    @property
    def best_round(self) -> GenerationRound:
        """Get the round with highest pass rate"""
        return max(self.rounds, key=lambda r: r.test_result.pass_rate)

    def get_round(self, round_number: int) -> Optional[GenerationRound]:
        """Get specific round"""
        for round_data in self.rounds:
            if round_data.round_number == round_number:
                return round_data
        return None


@dataclass
class VibeConfig:
    """Configuration for Vibe engine"""
    llm_model: str = "gpt-4"
    llm_api_key: Optional[str] = None
    max_optimization_rounds: int = 5
    output_dir: str = "./agents"
    temperature: float = 0.3
    verbose: bool = True
    auto_save_rounds: bool = True
