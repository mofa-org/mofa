import yaml

def parse_test_cases(yaml_file_path):
    """
    Parse test case YAML file
    
    Parameters:
        yaml_file_path: Path to the YAML file
        
    Returns:
        Parsed list of test cases
    """
    try:
        with open(yaml_file_path, 'r', encoding='utf-8') as f:
            # Load YAML file content
            data = yaml.safe_load(f)
            
            # Check if test_cases field exists
            if 'test_cases' not in data:
                raise ValueError("test_cases field not found in YAML file")
                
            return data['test_cases']
            
    except FileNotFoundError:
        print(f"Error: File {yaml_file_path} not found")
        return None
    except yaml.YAMLError as e:
        print(f"YAML parsing error: {e}")
        return None

def main():
    # Assume the YAML file is named test_hello_world.yml
    test_cases = parse_test_cases('./tests/test_hello_world.yml')
    
    if test_cases:
        print(f"Successfully parsed {len(test_cases)} test cases:")
        for i, case in enumerate(test_cases, 1):
            print(f"\nTest case {i}: {case['name']}")
            print(f"Input: {case['input']}")
            print(f"Expected output: {case['expected_output']}")

if __name__ == "__main__":
    main()
    