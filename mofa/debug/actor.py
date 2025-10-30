"""Module to execute unit tests for dynamically loaded node/agent modules."""
from .load_node import normalize_relative_imports, build_execution_globals, remove_main_guard
import textwrap

def clean_code(code: str) -> str:
    """"Remove common leading whitespace from all lines, preserving relative indentation"""
    # Use textwrap.dedent for proper deindentation
    # This removes the minimum common leading whitespace from all lines
    return textwrap.dedent(code)

def execute_unit_tests(node_module, test_cases, unit_test=True):
    """
    Execute unit tests for the given node module and test cases.
    
    Parameters:
        node_module: The dynamically loaded node/agent module
        test_cases: List of test case dictionaries

    Returns:
        List of test results
    """
    SEND_OUTPUT_FUNC_ARG = "agent_result"

    MULTI_RECV_PARAM_FUNC_ARG = "parameter_names"

    YAML_NAME= "name"
    YAML_OUTPUT= "expected_output"
    YAML_INPUT= "input"

    def get_adaptive_result(test_case):
        return test_case.get(YAML_OUTPUT)

    def validate_output(test_case, output_value):
        """
        Validate output against test case expectations.
        Supports both exact matching and validation rules.
        """
        # Check if test case has validation rules (for non-deterministic outputs)
        if 'validation' in test_case:
            validation = test_case['validation']

            # Type check
            expected_type = validation.get('type')
            if expected_type:
                type_map = {'str': str, 'int': int, 'float': float, 'list': list, 'dict': dict, 'bool': bool}
                if expected_type in type_map and not isinstance(output_value, type_map[expected_type]):
                    return False, f"Expected type {expected_type}, got {type(output_value).__name__}"

            # Not empty check
            if validation.get('not_empty') and not output_value:
                return False, "Output is empty"

            # Min length check
            min_length = validation.get('min_length')
            if min_length and hasattr(output_value, '__len__') and len(output_value) < min_length:
                return False, f"Output length {len(output_value)} < min_length {min_length}"

            # Max length check
            max_length = validation.get('max_length')
            if max_length and hasattr(output_value, '__len__') and len(output_value) > max_length:
                return False, f"Output length {len(output_value)} > max_length {max_length}"

            # Contains check
            contains = validation.get('contains', [])
            if contains and isinstance(output_value, str):
                for keyword in contains:
                    if keyword not in output_value:
                        return False, f"Output does not contain '{keyword}'"

            return True, "Validation passed"

        # Traditional exact match for deterministic outputs
        expected_output = get_adaptive_result(test_case)
        if expected_output is None:
            return False, "No expected_output or validation defined"

        if output_value == expected_output:
            return True, "Passed"
        else:
            return False, f"Expected {expected_output}, got {output_value}"
    
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
    
    source_code = node_module['source'] if isinstance(node_module, dict) else getattr(node_module, 'source', '')
    # print("Source code:", type(source_code))
    code = remove_main_guard(source_code)
    # print("!!Cleaned code for testing:\n", code)
    exec_globals = build_execution_globals(code, node_module['path'])
    # print("!!Execution globals prepared:", exec_globals.keys())
    temp_globals = {**exec_globals}

    IS_MULTI_PARAM = False
    if isinstance(receive_params, list) and len(receive_params) > 1:
        IS_MULTI_PARAM = True
 
    send_params_dict = {}
    for param in send_params:
      key, value = param.split('=', 1) 
      send_params_dict[key.strip()] = value.strip()

    if unit_test:
        results = []
        for case in test_cases:
            # Prepare the test environment
            if IS_MULTI_PARAM:
                input_query = case[YAML_INPUT]
            else:
                # Single parameter: extract the value from the input dict
                input_dict = case[YAML_INPUT]
                if isinstance(input_dict, dict) and len(input_dict) == 1:
                    # Extract the single value from the dict
                    input_query = list(input_dict.values())[0]
                else:
                    input_query = input_dict
            local_vars = {receive_target: input_query}
            # Execute the test case
            try:
                exec_code = normalize_relative_imports(format_code)
                exec(exec_code, temp_globals, local_vars)
                output_value = local_vars.get(send_params_dict.get(SEND_OUTPUT_FUNC_ARG, '').strip("'"))

                # Use new validation logic
                passed, message = validate_output(case, output_value)
                results.append((case[YAML_NAME], passed, message))

            except Exception as e:
                results.append((case[YAML_NAME], False, str(e)))
        return results
    else:
        for case in test_cases:
            # Prepare the test environment
            if IS_MULTI_PARAM:
                input_query = case[YAML_INPUT]
            else:
                # Single parameter: extract the value from the input dict
                input_dict = case[YAML_INPUT]
                if isinstance(input_dict, dict) and len(input_dict) == 1:
                    # Extract the single value from the dict
                    input_query = list(input_dict.values())[0]
                else:
                    input_query = input_dict
            local_vars = {receive_target: input_query}
            # Execute the test case
            try:
                exec_code = normalize_relative_imports(format_code)
                exec(exec_code, temp_globals, local_vars)
                output_value = local_vars.get(send_params_dict.get(SEND_OUTPUT_FUNC_ARG, '').strip("'"))
                print(f"节点运行结果, Output: {output_value}")
            except Exception as e:
                print(f"节点运行错误, Error: {str(e)}")
