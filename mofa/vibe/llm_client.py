"""LLM client for code and test generation"""

import os
from typing import Optional
import time


class LLMClient:
    """Client for interacting with LLMs (OpenAI, etc.)"""

    def __init__(self, model: str = "gpt-4", api_key: Optional[str] = None, temperature: float = 0.3):
        self.model = model
        self.api_key = api_key or os.getenv('OPENAI_API_KEY')
        self.temperature = temperature

        if not self.api_key:
            raise ValueError("OpenAI API key not found. Set OPENAI_API_KEY environment variable.")

    def generate(self, prompt: str, max_retries: int = 3) -> str:
        """
        Generate text from prompt with retry logic

        Args:
            prompt: The prompt to send to the LLM
            max_retries: Maximum number of retry attempts

        Returns:
            Generated text response
        """
        try:
            from openai import OpenAI
        except ImportError:
            raise ImportError("openai package not installed. Run: pip install openai")

        client = OpenAI(api_key=self.api_key)

        for attempt in range(max_retries):
            try:
                response = client.chat.completions.create(
                    model=self.model,
                    messages=[
                        {"role": "system", "content": "You are an expert Python developer specializing in MoFA agent development."},
                        {"role": "user", "content": prompt}
                    ],
                    temperature=self.temperature,
                    max_tokens=2000
                )

                return response.choices[0].message.content.strip()

            except Exception as e:
                if attempt < max_retries - 1:
                    wait_time = 2 ** attempt  # Exponential backoff
                    print(f"⚠️  LLM API error: {e}. Retrying in {wait_time}s...")
                    time.sleep(wait_time)
                else:
                    raise Exception(f"LLM API failed after {max_retries} attempts: {e}")

    def generate_test_cases(self, requirement: str) -> str:
        """Generate test cases YAML from requirement description"""
        prompt = f"""
Generate comprehensive test cases for a MoFA agent based on this requirement:

{requirement}

Output ONLY a valid YAML format with this structure:

```yaml
test_cases:
  - name: descriptive_test_name
    input:
      parameter_name: value
    expected_output:
      output_name: expected_value
```

Guidelines:
1. Include at least 3 test cases covering:
   - Normal/happy path cases
   - Edge cases (empty input, special characters, etc.)
   - Boundary conditions
2. Use clear, descriptive test names
3. Ensure input/output parameter names are consistent
4. Output ONLY the YAML, no explanations or markdown code blocks
"""
        return self.generate(prompt)

    def generate_code(self, requirement: str, test_cases_yaml: str, agent_name: str) -> str:
        """Generate MoFA agent code from requirement and test cases"""
        prompt = f"""
Generate a complete MoFA agent implementation that passes all the test cases.

## Requirement
{requirement}

## Test Cases (MUST ALL PASS)
```yaml
{test_cases_yaml}
```

## Agent Name
{agent_name}

## MoFA Agent Template
You MUST follow this exact structure:

```python
from mofa.agent_build.base.base_agent import MofaAgent, run_agent

@run_agent
def run(agent: MofaAgent):
    # Step 1: Receive input parameter(s)
    # Use agent.receive_parameter('param_name') for single parameter
    # Use agent.receive_parameters(['param1', 'param2']) for multiple parameters

    # Step 2: Implement the business logic
    # Your code here to process the input

    # Step 3: Send output
    # Use agent.send_output(agent_output_name='output_name', agent_result=result)

def main():
    agent = MofaAgent(agent_name='{agent_name}')
    run(agent=agent)

if __name__ == "__main__":
    main()
```

## Guidelines
1. Import necessary libraries at the top
2. Follow Python best practices
3. Add error handling only if truly necessary
4. Keep it simple and focused
5. The code MUST pass all test cases
6. Output ONLY the complete Python code, no explanations or markdown

Generate the complete main.py code now:
"""
        return self.generate(prompt)

    def regenerate_code(self, original_code: str, test_failures: str, requirement: str) -> str:
        """Regenerate code based on test failures"""
        prompt = f"""
The following MoFA agent code failed some tests. Fix the issues.

## Original Requirement
{requirement}

## Current Code
```python
{original_code}
```

## Test Failures
{test_failures}

## Task
Analyze the failures and generate FIXED code that passes all tests.
Keep the same structure but fix the logic errors.

Output ONLY the complete corrected Python code, no explanations:
"""
        return self.generate(prompt)
