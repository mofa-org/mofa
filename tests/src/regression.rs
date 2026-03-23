//! Small regression-oriented assertions used by fixture-driven contract tests.

/// Assert that two strings are exactly equal.
pub fn assert_exact_match(actual: impl AsRef<str>, expected: impl AsRef<str>) {
    assert_eq!(
        actual.as_ref(),
        expected.as_ref(),
        "exact regression mismatch"
    );
}

/// Assert that an error string contains the expected substring.
pub fn assert_error_contains(error: impl AsRef<str>, expected_substring: impl AsRef<str>) {
    let error = error.as_ref();
    let expected_substring = expected_substring.as_ref();
    assert!(
        error.contains(expected_substring),
        "expected error to contain '{}', got '{}'",
        expected_substring,
        error
    );
}

/// Assert that a string contains each expected substring.
pub fn assert_contains_all(actual: impl AsRef<str>, expected_substrings: &[String]) {
    let actual = actual.as_ref();
    for expected in expected_substrings {
        assert!(
            actual.contains(expected),
            "expected '{}' to contain '{}'",
            actual,
            expected
        );
    }
}
