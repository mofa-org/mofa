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

    results = []
    for case in test_cases:
        # Prepare the test environment
        input_query = case['input'][receive_params[0]] if receive_params else None
        local_vars = {receive_target: input_query}
        # Execute the test case
        try:
            exec(format_code, {}, local_vars)
            output_value = local_vars.get(send_params['agent_result']) if len(send_params) > 1 else None
            expected_output = case['expected_output']
            print(f"Test case '{case['name']}': input={input_query}, expected_output={expected_output}, actual_output={output_value}")
            if output_value == expected_output: # Compare actual output with expected output
                results.append((case['name'], True, "Passed"))
            else:
                results.append((case['name'], False, f"Failed: expected {expected_output}, got {output_value}"))

        except Exception as e:
            results.append((case['name'], False, str(e)))
    return results
