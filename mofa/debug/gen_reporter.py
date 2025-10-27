import sys
from typing import List, Tuple

def generate_test_report(results: List[Tuple[str, bool, str]]) -> None:
    """
    Generate and print test report
    
    Args:
        results: List of test results, each containing:
            (test case name, passed flag, message)
    """
    # Calculate summary statistics
    total = len(results)
    passed = sum(1 for r in results if r[1])
    failed = total - passed
    pass_rate = (passed / total) * 100 if total > 0 else 0

    # Print detailed results
    for i, (name, passed_flag, message) in enumerate(results, 1):
        status = "[PASS] Passed" if passed_flag else "[FAIL] Failed"
        print(f"Test case {i}/{total}: {name}")
        print(f"Status: {status}")
        
        if not passed_flag:
            print(f"Message: {message}")
        
        print("----------------------------------")

    # Print summary report
    print("\n" + "="*40)
    print(f"Test Summary:")
    print(f"Total test cases: {total}")
    print(f"Passed: {passed}")
    print(f"Failed: {failed}")
    print(f"Pass rate: {pass_rate:.2f}%")
    print("="*40 + "\n")

    # Exit with code 1 if there are failed tests
    if failed > 0:
        sys.exit(1)
