"""LLM client for code and test generation"""

import os
import re
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

        # Get custom base URL from environment if set
        base_url = os.getenv('OPENAI_API_BASE')
        client = OpenAI(api_key=self.api_key, base_url=base_url) if base_url else OpenAI(api_key=self.api_key)

        # Check if this is a GPT-5/reasoning model (uses responses API)
        is_reasoning_model = 'gpt-5' in self.model.lower() or 'o1' in self.model.lower() or 'o3' in self.model.lower()

        for attempt in range(max_retries):
            try:
                if is_reasoning_model:
                    # GPT-5 and reasoning models use the new responses API
                    response = client.responses.create(
                        model=self.model,
                        input=prompt
                    )
                    return response.output_text.strip()

                elif 'gpt-4o' in self.model.lower():
                    # GPT-4o uses max_completion_tokens
                    response = client.chat.completions.create(
                        model=self.model,
                        messages=[
                            {"role": "system", "content": "You are an expert Python developer specializing in MoFA agent development."},
                            {"role": "user", "content": prompt}
                        ],
                        temperature=self.temperature,
                        max_completion_tokens=2000
                    )
                    return response.choices[0].message.content.strip()

                else:
                    # Standard models (GPT-4, GPT-3.5, Kimi, etc.)
                    # Build parameters dynamically to support different providers
                    params = {
                        "model": self.model,
                        "messages": [
                            {"role": "system", "content": "You are an expert Python developer specializing in MoFA agent development."},
                            {"role": "user", "content": prompt}
                        ],
                        "temperature": self.temperature
                    }

                    # Only add max_tokens for OpenAI models
                    # Some providers like Kimi don't need it or handle it automatically
                    if 'gpt' in self.model.lower():
                        params["max_tokens"] = 2000

                    response = client.chat.completions.create(**params)
                    return response.choices[0].message.content.strip()

            except Exception as e:
                error_msg = str(e)

                # Provide friendly error messages
                if "temperature" in error_msg.lower() and "not support" in error_msg.lower():
                    print(f"\nError: Model '{self.model}' does not support custom temperature.")
                    print("Tip: Try using 'gpt-4o-mini' or 'gpt-4o' instead, or check the model documentation.")
                    raise RuntimeError(f"Model '{self.model}' does not support temperature parameter. Use a standard chat model instead.")

                elif "max_tokens" in error_msg.lower() or "max_completion_tokens" in error_msg.lower():
                    print(f"\nError: Parameter mismatch for model '{self.model}'.")
                    print("Tip: The model API may have changed. Try updating your openai package: pip install --upgrade openai")
                    raise RuntimeError(f"Token parameter error for model '{self.model}'. Try updating openai package.")

                elif attempt < max_retries - 1:
                    wait_time = 2 ** attempt
                    print(f"Warning: API error (attempt {attempt + 1}/{max_retries}): {error_msg[:100]}...")
                    print(f"Retrying in {wait_time}s...")
                    time.sleep(wait_time)
                else:
                    print(f"\nError: API request failed after {max_retries} attempts.")
                    print(f"Model: {self.model}")
                    print(f"Error: {error_msg}")
                    print("\nTroubleshooting:")
                    print("  1. Check your API key is valid")
                    print("  2. Verify the model name is correct")
                    print("  3. Check your API endpoint (if using custom endpoint)")
                    print("  4. Try a different model (e.g., 'gpt-4o-mini')")
                    raise RuntimeError(f"LLM API failed: {error_msg}")

    def generate_test_cases(self, requirement: str) -> str:
        """Generate test cases YAML from requirement description"""
        prompt = f"""
Generate comprehensive test cases for a MoFA agent based on this requirement:

{requirement}

IMPORTANT:
- Carefully analyze the requirement to understand inputs and outputs
- If the requirement is ambiguous, choose the SIMPLEST interpretation
- Focus on what the agent DOES, not what it processes

Output ONLY a valid YAML format with this structure:

For DETERMINISTIC outputs (calculations, transformations):
```yaml
test_cases:
  - name: descriptive_test_name
    input:
      parameter_name: value
    expected_output:
      output_name: expected_value
```

For NON-DETERMINISTIC outputs (LLM calls, random generation):
```yaml
test_cases:
  - name: descriptive_test_name
    input:
      parameter_name: value
    validation:
      type: str  # Expected output type
      not_empty: true  # Output should not be empty
      min_length: 10  # Optional: minimum length
      max_length: 1000  # Optional: maximum length
      contains: ["keyword"]  # Optional: must contain these keywords
```

Guidelines:
1. IMPORTANT: If the agent calls LLM APIs (OpenAI, Claude, etc.) or generates random content, use the validation format!
2. Include at least 3 test cases covering:
   - Normal/happy path cases
   - Edge cases (empty input, special characters, etc.)
   - Boundary conditions
3. Use clear, descriptive test names
4. Ensure input/output parameter names are consistent
5. CRITICAL: Use ONLY literal values in YAML. DO NOT use Python expressions like "a"*1000 or any code.
   - WRONG: dog_name: "a"*1000
   - WRONG: contains: ["a"*1000]
   - RIGHT: dog_name: "aaaaaaa..." (write out actual string)
   - RIGHT: Use reasonable test values (short strings are fine)
6. Output ONLY the YAML, no explanations or markdown code blocks
"""
        return self.generate(prompt)

    def generate_code(self, requirement: str, test_cases_yaml: str, agent_name: str) -> str:
        """Generate MoFA agent code from requirement and test cases"""

        # Read LLM config from .env
        import os
        from dotenv import load_dotenv
        load_dotenv()

        api_key = os.getenv('OPENAI_API_KEY', '')
        base_url = os.getenv('OPENAI_API_BASE', 'https://api.openai.com/v1')
        model = os.getenv('MOFA_VIBE_MODEL', 'gpt-4o-mini')

        # Build LLM integration example with actual values
        llm_example = ""
        if api_key:
            llm_example = f"""

## LLM Integration (if needed)
If the requirement involves LLM/AI calls, you can use this pre-configured OpenAI client:

```python
from openai import OpenAI

# Pre-configured with your .env settings
client = OpenAI(
    api_key="{api_key}",
    base_url="{base_url}"
)

response = client.chat.completions.create(
    model="{model}",
    messages=[
        {{"role": "system", "content": "You are a helpful assistant."}},
        {{"role": "user", "content": user_input}}
    ],
    temperature=0.7
)

result = response.choices[0].message.content
```

Note: The API key, base URL, and model are from your .env configuration.
"""

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
{llm_example}
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

    def generate_agent_name(self, requirement: str) -> str:
        """Generate a descriptive agent name from requirement"""
        prompt = f"""
Generate a concise, descriptive agent name for this requirement:

{requirement}

Guidelines:
1. Use lowercase letters, numbers, and hyphens only
2. 2-4 words maximum
3. Focus on the CORE FUNCTIONALITY (what it does)
4. Use simple, clear English words
5. Examples:
   - "将文本转换为ASCII艺术" → "text-to-ascii"
   - "调用OpenAI API生成回复" → "openai-chat"
   - "读取CSV文件并统计" → "csv-analyzer"
   - "把图片转成黑白" → "image-grayscale"

Output ONLY the agent name (e.g., "text-analyzer"), no explanations or quotes:
"""
        name = self.generate(prompt).strip().strip('"').strip("'")
        # Ensure it's valid (only lowercase, numbers, hyphens)
        name = re.sub(r'[^a-z0-9-]', '', name.lower())
        return name or "custom-agent"

    def modify_test_cases_conversational(self, current_yaml: str, user_instruction: str, requirement: str, conversation_history: list = None) -> str:
        """
        Modify test cases based on conversational instruction

        Args:
            current_yaml: Current test cases YAML
            user_instruction: User's instruction for modification
            requirement: Original requirement
            conversation_history: List of previous messages for context

        Returns:
            Modified test cases YAML
        """
        # Build conversation context
        messages = [
            {"role": "system", "content": "You are an expert in creating and modifying test cases for MoFA agents. You help users refine their test cases through conversation."}
        ]

        # Add conversation history if exists
        if conversation_history:
            messages.extend(conversation_history)

        # Add current request
        prompt = f"""Current test cases YAML:
```yaml
{current_yaml}
```

Original requirement:
{requirement}

User instruction:
{user_instruction}

Please modify the test cases according to the user's instruction. Follow these guidelines:
1. Keep the same YAML structure (test_cases list with name, input, expected_output/validation)
2. If the user mentions specific test cases by name, modify those
3. If the user wants to add tests, append them to the list
4. If the user wants to remove tests, remove the specified ones
5. Ensure all modifications maintain valid YAML syntax
6. For deterministic outputs, use expected_output; for non-deterministic (LLM calls), use validation

Output ONLY the complete modified YAML, no explanations or markdown code blocks.
"""

        messages.append({"role": "user", "content": prompt})

        # Call LLM with conversation
        try:
            from openai import OpenAI
        except ImportError:
            raise ImportError("openai package not installed. Run: pip install openai")

        base_url = os.getenv('OPENAI_API_BASE')
        client = OpenAI(api_key=self.api_key, base_url=base_url) if base_url else OpenAI(api_key=self.api_key)

        # Use chat completion for conversation
        is_reasoning_model = 'gpt-5' in self.model.lower() or 'o1' in self.model.lower() or 'o3' in self.model.lower()

        if is_reasoning_model:
            # Reasoning models don't support multi-turn conversations yet, use simple prompt
            response = client.responses.create(
                model=self.model,
                input=prompt
            )
            return response.output_text.strip()
        else:
            # Standard chat models
            params = {
                "model": self.model,
                "messages": messages,
                "temperature": self.temperature
            }

            if 'gpt-4o' in self.model.lower():
                params["max_completion_tokens"] = 2000
            elif 'gpt' in self.model.lower():
                params["max_tokens"] = 2000

            response = client.chat.completions.create(**params)
            return response.choices[0].message.content.strip()

    def generate_flow_yaml(self, requirement: str, flow_name: str, available_agents: list = None, agent_details: dict = None) -> str:
        """Generate MoFA flow dataflow YAML from requirement"""

        agents_info = ""
        if available_agents:
            if agent_details:
                # Detailed agent info with inputs/outputs
                agents_list = []
                for agent in available_agents:
                    detail = agent_details.get(agent, {})
                    inputs = detail.get('inputs', [])
                    outputs = detail.get('outputs', [])
                    desc = detail.get('description', '')

                    agent_info = f"- {agent}"
                    if inputs:
                        agent_info += f"\n  Inputs: {', '.join(inputs)}"
                    if outputs:
                        agent_info += f"\n  Outputs: {', '.join(outputs)}"
                    if desc:
                        agent_info += f"\n  Description: {desc}"
                    agents_list.append(agent_info)

                agents_info = f"""

## Available Local Agents
You MUST use agents from this list only. Each agent has specific inputs and outputs that you MUST respect:

{chr(10).join(agents_list)}

IMPORTANT:
1. Only use agents that exist in the list above
2. Connect outputs to inputs correctly based on the parameter names
3. Do not invent new agent names or input/output parameters
"""
            else:
                # Simple list
                agents_info = f"""

## Available Agents
You MUST use agents from this list only:
{chr(10).join([f"- {agent}" for agent in available_agents])}

IMPORTANT: Only use agents that exist in the list above. Do not invent new agent names.
"""

        prompt = f"""
Generate a complete MoFA flow dataflow YAML based on this requirement:

{requirement}

## Flow Name
{flow_name}
{agents_info}

## MoFA Flow Structure
A flow is a YAML file that defines nodes (agents) and their connections.

Example structure:
```yaml
nodes:
  - id: terminal-input
    build: pip install -e ../../agents/terminal-input
    path: dynamic
    outputs:
      - data
    inputs:
      result: my-agent/output

  - id: my-agent
    build: pip install -e ../../agents/my-agent
    path: my-agent
    outputs:
      - output
    inputs:
      query: terminal-input/data
    env:
      IS_DATAFLOW_END: true
      WRITE_LOG: true
```

## Guidelines
1. Use `terminal-input` as the first node (for user input)
2. ONLY use agents from the available agents list
3. Define clear input/output connections between nodes
4. Use descriptive node IDs (lowercase-with-hyphens)
5. The last node should have `IS_DATAFLOW_END: true` in env
6. Input/output format: `node-id/output-name`
7. Keep it simple - 2-4 nodes maximum
8. Output ONLY valid YAML, no explanations or markdown

Generate the flow dataflow YAML now:
"""
        return self.generate(prompt)
