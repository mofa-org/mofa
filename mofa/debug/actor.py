"""Module to execute unit tests for dynamically loaded node/agent modules."""

def clean_code(code: str) -> str:
    """"make sure the code is properly indented and formatted"""
    lines = code.split('\n')
    processed_lines = []
    for line in lines:
        # Check if the line starts with 4 spaces (a common indentation level)
        if line.startswith('    '):
            processed_lines.append(line[4:])
        else:
            processed_lines.append(line)
    return '\n'.join(processed_lines)

def execute_unit_tests(node_module, test_cases):
    """
    Execute unit tests for the given node module and test cases.
    
    Parameters:
        node_module: The dynamically loaded node/agent module
        test_cases: List of test case dictionaries

    Returns:
        List of test results
    """
    SEND_OUTPUT_FUNC_ARG = "agent_result"

    YAML_NAME= "name"
    YAML_OUTPUT= "expected_output"
    YAML_INPUT= "input"

    def get_adaptive_result(test_case):
        """Get the expected output value from the test case"""
        expected_output = test_case[YAML_OUTPUT]
        # assume there's only one key-value pair
        return next(iter(expected_output.values()))
    
    # node_module may be a module object with attribute agent_info, or a descriptor dict
    if isinstance(node_module, dict):
        agent_info = node_module.get('agent_info') or {}
        between = agent_info.get('between_code', '')
        receive_params = agent_info.get('receive_params', [])
        receive_target = agent_info.get('receive_target', None)
        send_params = agent_info.get('send_params', [])
    else:
        # assume module-like
        agent_info = getattr(node_module, 'agent_info', {}) or {}
        between = agent_info.get('between_code', '')
        receive_params = agent_info.get('receive_params', [])
        receive_target = agent_info.get('receive_target', None)
        send_params = agent_info.get('send_params', [])

    format_code = clean_code(between or '')
 
    send_params_dict = {}
    for param in send_params:
      key, value = param.split('=', 1) 
      send_params_dict[key.strip()] = value.strip()

    results = []
    for case in test_cases:
        # Prepare the test environment
        input_query = case[YAML_INPUT][receive_params[0].strip("'")] if receive_params else None
        local_vars = {receive_target: input_query}
        # Execute the test case
        try:
            exec(format_code, {}, local_vars)
            output_value = local_vars.get(send_params_dict.get(SEND_OUTPUT_FUNC_ARG, '').strip("'"))
            expected_output = get_adaptive_result(case)
            # print(f"Test case '{case[YAML_NAME]}': input={input_query}, expected_output={expected_output}, actual_output={output_value}")
            if output_value == expected_output: # Compare actual output with expected output
                results.append((case[YAML_NAME], True, "Passed"))
            else:
                results.append((case[YAML_NAME], False, f"Failed: expected {expected_output}, got {output_value}"))

        except Exception as e:
            results.append((case[YAML_NAME], False, str(e)))
    return results
