# MoFA Vibe - AI Agent Generator Architecture

## Overview
Vibe is an intelligent, iterative agent generation system that creates MoFA agents through natural language descriptions, automatic testing, and multi-round optimization.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      User Interface Layer                       │
│  • Interactive CLI with rich output                             │
│  • Real-time progress display                                   │
│  • User feedback collection                                     │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│                    Orchestrator (VibeEngine)                    │
│  • Workflow coordination                                        │
│  • State management                                             │
│  • Round iteration control                                      │
└─────┬──────────┬──────────┬──────────┬──────────┬──────────────┘
      │          │          │          │          │
      │          │          │          │          │
┌─────▼────┐ ┌──▼────┐ ┌───▼────┐ ┌───▼────┐ ┌──▼──────┐
│Requirement│ │Test   │ │Code    │ │Debug   │ │Optimizer│
│Parser    │ │Gen    │ │Gen     │ │Runner  │ │         │
└──────────┘ └───────┘ └────────┘ └────────┘ └─────────┘
      │          │          │          │          │
      └──────────┴──────────┴──────────┴──────────┘
                             │
                    ┌────────▼────────┐
                    │  LLM Interface  │
                    │  (GPT-4/Claude) │
                    └─────────────────┘
```

## Core Components

### 1. VibeEngine (Main Orchestrator)
**Location**: `mofa/vibe/engine.py`

**Responsibilities**:
- Coordinate the entire generation workflow
- Manage iteration rounds
- Track state and history
- Handle user interactions

**Key Methods**:
```python
class VibeEngine:
    def run_interactive(self) -> GenerationResult
    def run_from_description(self, description: str) -> GenerationResult
    def iterate(self, user_feedback: str) -> GenerationResult
```

### 2. RequirementParser
**Location**: `mofa/vibe/parser.py`

**Responsibilities**:
- Parse natural language requirements
- Extract input/output expectations
- Identify complexity and dependencies

**Flow**:
```
User Description
      ↓
LLM Analysis
      ↓
Structured Requirements:
  - inputs: List[Parameter]
  - outputs: List[Parameter]
  - logic_description: str
  - dependencies: List[str]
```

### 3. TestCaseGenerator
**Location**: `mofa/vibe/test_generator.py`

**Responsibilities**:
- Generate comprehensive test cases from requirements
- Create edge cases automatically
- Output YAML format compatible with mofa debug

**Process**:
```python
def generate_test_cases(requirement: Requirement) -> TestSuite:
    # 1. Generate normal cases
    normal_cases = self._generate_normal_cases(requirement)

    # 2. Generate edge cases
    edge_cases = self._generate_edge_cases(requirement)

    # 3. Generate error cases
    error_cases = self._generate_error_cases(requirement)

    # 4. Combine and format
    return TestSuite(cases=[*normal_cases, *edge_cases, *error_cases])
```

### 4. CodeGenerator
**Location**: `mofa/vibe/code_generator.py`

**Responsibilities**:
- Generate MoFA-compliant agent code
- Support multiple generation strategies
- Maintain code quality and best practices

**Strategies**:
```python
class CodeGenerator:
    def generate(self, requirement: Requirement, test_cases: TestSuite) -> AgentCode:
        if self._is_simple_pattern(requirement):
            return self._generate_from_template(requirement)
        else:
            return self._generate_with_llm(requirement, test_cases)

    def regenerate(self, previous_code: str, errors: List[Error]) -> AgentCode:
        """Regenerate code based on test failures"""
        pass
```

**LLM Prompt Template**:
```python
GENERATION_PROMPT = """
You are an expert MoFA agent developer. Generate a complete MoFA agent implementation.

## Requirements
{requirement_description}

## Test Cases (Must Pass)
```yaml
{test_cases_yaml}
```

## MoFA Agent Template
```python
from mofa.agent_build.base.base_agent import MofaAgent, run_agent

@run_agent
def run(agent: MofaAgent):
    # Step 1: Receive parameters
    {input_pattern}

    # Step 2: Process logic
    {processing_logic}

    # Step 3: Send output
    {output_pattern}

def main():
    agent = MofaAgent(agent_name='{agent_name}')
    run(agent=agent)

if __name__ == "__main__":
    main()
```

## Guidelines
1. Use appropriate Python libraries (import at top)
2. Add error handling where needed
3. Follow Python best practices
4. Code must be production-ready
5. Add helpful comments

## Output Format
Provide ONLY the complete main.py code, no explanations.
"""
```

### 5. DebugRunner
**Location**: `mofa/vibe/debug_runner.py`

**Responsibilities**:
- Execute `mofa debug` command
- Capture and parse output
- Extract failure information

**Implementation**:
```python
class DebugRunner:
    def run_tests(self, agent_path: str, test_yaml: str) -> TestResult:
        # Execute mofa debug command
        process = subprocess.run(
            ['mofa', 'debug', agent_path, test_yaml],
            capture_output=True,
            text=True
        )

        # Parse output
        result = self._parse_debug_output(process.stdout)
        return result

    def _parse_debug_output(self, output: str) -> TestResult:
        """Parse mofa debug output to extract test results"""
        # Extract pass/fail status for each test
        # Extract error messages
        # Calculate statistics
        pass
```

### 6. Optimizer
**Location**: `mofa/vibe/optimizer.py`

**Responsibilities**:
- Analyze test failures
- Generate improvement suggestions
- Coordinate code regeneration

**Optimization Loop**:
```python
class Optimizer:
    def optimize(self, code: str, test_result: TestResult, max_rounds: int = 5) -> OptimizationResult:
        for round_num in range(max_rounds):
            if test_result.pass_rate == 100:
                return OptimizationResult(success=True, final_code=code)

            # Analyze failures
            error_analysis = self._analyze_errors(test_result)

            # Generate fix suggestions
            suggestions = self._generate_suggestions(error_analysis)

            # Regenerate code
            code = self.code_generator.regenerate(code, suggestions)

            # Re-test
            test_result = self.debug_runner.run_tests(agent_path, test_yaml)

            # Display progress
            self._display_round_result(round_num + 1, test_result)

        return OptimizationResult(success=False, final_code=code)
```

**Error Analysis**:
```python
def _analyze_errors(self, test_result: TestResult) -> ErrorAnalysis:
    """
    Analyze test failures to identify root causes:
    - Logic errors (wrong algorithm)
    - Type errors (wrong data type handling)
    - Edge case errors (missing boundary checks)
    - Dependency errors (missing imports)
    """
    failed_tests = [t for t in test_result.tests if not t.passed]

    error_patterns = {
        'logic_error': [],
        'type_error': [],
        'edge_case_error': [],
        'dependency_error': []
    }

    for test in failed_tests:
        pattern = self._classify_error(test)
        error_patterns[pattern].append(test)

    return ErrorAnalysis(patterns=error_patterns)
```

### 7. ProjectScaffolder
**Location**: `mofa/vibe/scaffolder.py`

**Responsibilities**:
- Create agent project structure
- Use cookiecutter template
- Manage file generation

**Structure**:
```python
class ProjectScaffolder:
    def create_project(self, agent_name: str, code: str, test_cases: str) -> str:
        """
        Create complete agent project structure:

        agent-hub/{agent_name}/
        ├── agent/
        │   ├── __init__.py
        │   ├── main.py              <- Generated code
        │   └── configs/
        │       └── agent.yml
        ├── tests/
        │   └── test_{agent_name}.yml  <- Generated test cases
        ├── pyproject.toml
        └── README.md
        """
        project_path = self._create_directory_structure(agent_name)
        self._write_main_code(project_path, code)
        self._write_test_cases(project_path, test_cases)
        self._write_pyproject_toml(project_path, agent_name)
        self._write_readme(project_path, agent_name)

        return project_path
```

### 8. StateManager
**Location**: `mofa/vibe/state.py`

**Responsibilities**:
- Track generation history
- Enable rollback
- Support incremental updates

**State Structure**:
```python
@dataclass
class GenerationState:
    agent_name: str
    requirement: Requirement
    test_cases: TestSuite
    history: List[GenerationRound]
    current_code: str
    current_test_result: TestResult

@dataclass
class GenerationRound:
    round_number: int
    code: str
    test_result: TestResult
    optimization_applied: str
    timestamp: datetime
```

## Data Models

```python
# mofa/vibe/models.py

@dataclass
class Requirement:
    description: str
    inputs: List[Parameter]
    outputs: List[Parameter]
    dependencies: List[str]
    complexity: str  # 'simple' | 'medium' | 'complex'

@dataclass
class Parameter:
    name: str
    type: str
    description: str
    example_value: Any

@dataclass
class TestCase:
    name: str
    input: Dict[str, Any]
    expected_output: Dict[str, Any]

@dataclass
class TestSuite:
    cases: List[TestCase]

    def to_yaml(self) -> str:
        return yaml.dump({'test_cases': [asdict(c) for c in self.cases]})

@dataclass
class TestResult:
    total: int
    passed: int
    failed: int
    pass_rate: float
    tests: List[SingleTestResult]

@dataclass
class SingleTestResult:
    name: str
    passed: bool
    expected: Any
    actual: Any
    error_message: Optional[str]

@dataclass
class AgentCode:
    main_py: str
    dependencies: List[str]
    helper_files: Dict[str, str]  # filename -> content
```

## Workflow Implementation

### Complete Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                     1. User Input Phase                          │
└────────────────────────────┬────────────────────────────────────┘
                             │
                    ┌────────▼────────┐
                    │ User provides:  │
                    │ - Description   │
                    │ - Agent name    │
                    │ - (Optional)    │
                    │   Test cases    │
                    └────────┬────────┘
                             │
┌─────────────────────────────────────────────────────────────────┐
│                   2. Requirement Analysis                        │
└────────────────────────────┬────────────────────────────────────┘
                             │
                    ┌────────▼────────┐
                    │ LLM parses req  │
                    │ Extracts I/O    │
                    │ Identifies deps │
                    └────────┬────────┘
                             │
┌─────────────────────────────────────────────────────────────────┐
│                  3. Test Case Generation                         │
└────────────────────────────┬────────────────────────────────────┘
                             │
                    ┌────────▼────────┐
                    │ Generate tests  │
                    │ Show to user    │
                    │ Get approval    │
                    └────────┬────────┘
                             │
┌─────────────────────────────────────────────────────────────────┐
│              4. Initial Code Generation (Round 1)                │
└────────────────────────────┬────────────────────────────────────┘
                             │
                    ┌────────▼────────┐
                    │ LLM generates   │
                    │ agent code      │
                    │ Create project  │
                    └────────┬────────┘
                             │
┌─────────────────────────────────────────────────────────────────┐
│                    5. Automatic Testing                          │
└────────────────────────────┬────────────────────────────────────┘
                             │
                    ┌────────▼────────┐
                    │ Run mofa debug  │
                    │ Parse results   │
                    └────────┬────────┘
                             │
                        ┌────┴────┐
                        │         │
                   Pass │         │ Fail
                        │         │
        ┌───────────────▼─┐   ┌──▼──────────────┐
        │  6a. Success    │   │ 6b. Optimization│
        │  Show summary   │   │ Analyze errors  │
        │  Ask for more   │   │ Generate fix    │
        │  requirements   │   │ Regenerate code │
        └───────┬─────────┘   └──┬──────────────┘
                │                │
                │                │ Loop back to step 5
                │                │ (max 5 rounds)
                │                │
                └────────┬───────┘
                         │
┌─────────────────────────────────────────────────────────────────┐
│                    7. Iterative Enhancement                      │
└────────────────────────────┬────────────────────────────────────┘
                             │
                    ┌────────▼────────┐
                    │ User provides   │
                    │ new requirement │
                    │ or done         │
                    └────────┬────────┘
                             │
                        ┌────┴────┐
                        │         │
                    More│         │ Done
                        │         │
    ┌───────────────────▼─┐   ┌──▼──────────────┐
    │ Update existing     │   │ Finalize        │
    │ agent code          │   │ Show summary    │
    │ Re-test             │   │ Exit            │
    │ Loop to step 5      │   └─────────────────┘
    └─────────────────────┘
```

## CLI Command Design

### Basic Usage
```bash
# Interactive mode (recommended)
mofa vibe --interactive

# From description
mofa vibe --description "Extract email addresses from text" --name email-extractor

# From test cases
mofa vibe --tests test_cases.yml --name my-agent

# With specific LLM
mofa vibe --interactive --llm gpt-4

# With max optimization rounds
mofa vibe --interactive --max-rounds 3
```

### Command Options
```python
@mofa_cli_group.command()
@click.option('--interactive', '-i', is_flag=True, help='Interactive mode')
@click.option('--description', '-d', type=str, help='Agent description')
@click.option('--name', '-n', type=str, help='Agent name')
@click.option('--tests', '-t', type=click.Path(exists=True), help='Test case YAML file')
@click.option('--llm', type=str, default='gpt-4', help='LLM model to use')
@click.option('--max-rounds', type=int, default=5, help='Max optimization rounds')
@click.option('--output', '-o', type=click.Path(), help='Output directory')
def vibe(interactive, description, name, tests, llm, max_rounds, output):
    """AI-powered agent generator with automatic testing and optimization"""
    pass
```

## Display & Progress Tracking

### Rich Console Output

Use `rich` library for beautiful CLI output:

```python
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn
from rich.panel import Panel
from rich.syntax import Syntax
from rich.table import Table

class VibeDisplay:
    def __init__(self):
        self.console = Console()

    def show_header(self):
        self.console.print(Panel(
            "[bold cyan]🤖 MoFA Vibe - AI Agent Generator[/bold cyan]",
            border_style="cyan"
        ))

    def show_requirement_analysis(self, requirement: Requirement):
        table = Table(title="📋 Requirement Analysis")
        table.add_column("Aspect", style="cyan")
        table.add_column("Details", style="white")

        table.add_row("Description", requirement.description)
        table.add_row("Inputs", str(requirement.inputs))
        table.add_row("Outputs", str(requirement.outputs))
        table.add_row("Complexity", requirement.complexity)

        self.console.print(table)

    def show_code(self, code: str, title: str = "Generated Code"):
        syntax = Syntax(code, "python", theme="monokai", line_numbers=True)
        self.console.print(Panel(syntax, title=title, border_style="green"))

    def show_test_result(self, round_num: int, result: TestResult):
        status = "✅ PASSED" if result.pass_rate == 100 else "❌ FAILED"
        color = "green" if result.pass_rate == 100 else "red"

        self.console.print(f"\n[bold {color}]Round {round_num}: {status}[/bold {color}]")
        self.console.print(f"Pass Rate: {result.pass_rate}% ({result.passed}/{result.total})")

        # Show failed tests
        for test in result.tests:
            if not test.passed:
                self.console.print(f"  ❌ {test.name}")
                self.console.print(f"     Expected: {test.expected}")
                self.console.print(f"     Got: {test.actual}")
```

## Error Handling & Edge Cases

### 1. LLM API Failures
```python
def _call_llm_with_retry(self, prompt: str, max_retries: int = 3) -> str:
    for attempt in range(max_retries):
        try:
            return self.llm_client.generate(prompt)
        except APIError as e:
            if attempt < max_retries - 1:
                time.sleep(2 ** attempt)  # Exponential backoff
            else:
                raise GenerationError(f"LLM API failed after {max_retries} attempts")
```

### 2. Invalid Generated Code
```python
def _validate_code(self, code: str) -> bool:
    """Validate generated code before saving"""
    try:
        # Check syntax
        ast.parse(code)

        # Check for required patterns
        if '@run_agent' not in code:
            return False
        if 'def run(agent: MofaAgent)' not in code:
            return False

        return True
    except SyntaxError:
        return False
```

### 3. Infinite Optimization Loops
```python
def optimize(self, code: str, test_result: TestResult, max_rounds: int = 5) -> OptimizationResult:
    previous_codes = set()

    for round_num in range(max_rounds):
        # Detect code cycles
        code_hash = hashlib.md5(code.encode()).hexdigest()
        if code_hash in previous_codes:
            return OptimizationResult(
                success=False,
                reason="Stuck in optimization loop"
            )
        previous_codes.add(code_hash)

        # ... rest of optimization logic
```

## Configuration

```yaml
# mofa/vibe/config.yml

llm:
  default_model: "gpt-4"
  api_key_env: "OPENAI_API_KEY"
  temperature: 0.3
  max_tokens: 2000

generation:
  max_optimization_rounds: 5
  template_dir: "mofa/agent-template"
  output_dir: "agent-hub"

testing:
  auto_generate_edge_cases: true
  min_test_coverage: 80

display:
  show_intermediate_code: true
  verbose: true
```

## Future Enhancements

1. **Multi-file Generation**: Support generating complex agents with multiple helper modules
2. **Visual Editor**: Web-based UI for editing generated code
3. **Agent Composition**: Generate agents that compose existing agents
4. **Performance Optimization**: Analyze and optimize generated code for performance
5. **Documentation Generation**: Auto-generate comprehensive README and API docs
6. **Version Control Integration**: Auto-commit each iteration with meaningful messages
7. **Cloud Integration**: Deploy generated agents to cloud platforms
8. **Collaborative Mode**: Multiple users can iterate on the same agent
