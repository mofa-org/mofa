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
        send_params = agent_info.get('send_params', [])
    else:
        # assume module-like
        agent_info = getattr(node_module, 'agent_info', {}) or {}
        between = agent_info.get('between_code', '')
        receive_params = agent_info.get('receive_params', [])
        send_params = agent_info.get('send_params', [])

    format_code = clean_code(between or '')

    results = []
    for case in test_cases:
        # Prepare the test environment
        user_query = "test query"
        local_vars = {"user_query": user_query}
        # Execute the test case
        try:
            exec(format_code, {}, local_vars)
        except Exception as e:
            results.append((case['name'], False, str(e)))
    return results
