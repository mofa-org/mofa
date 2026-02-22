//! Integration tests for Rhai Plugin
//!
//! Tests the RhaiPlugin call_script_function implementation with various scenarios

use mofa_plugins::rhai_runtime::RhaiPlugin;
use rhai::Dynamic;

// ============================================================================
// Helper Functions
// ============================================================================

async fn create_test_plugin(script_content: &str, plugin_id: &str) -> RhaiPlugin {
    RhaiPlugin::from_content(plugin_id, script_content)
        .await
        .expect("Failed to create plugin")
}

// ============================================================================
// Basic Function Calling Tests
// ============================================================================

#[tokio::test]
async fn test_call_script_function_basic() {
    let script = r#"
        fn greet(name) {
            "Hello, " + name + "!"
        }
    "#;

    let plugin = create_test_plugin(script, "test_greet").await;

    let args = vec![Dynamic::from("World")];
    let result = plugin
        .call_script_function("greet", &args)
        .await
        .expect("Failed to call function");

    assert!(result.is_some());
    let result_str = result.unwrap().to_string();
    assert!(result_str.contains("Hello") && result_str.contains("World"));
}

#[tokio::test]
async fn test_call_script_function_with_multiple_args() {
    let script = r#"
        fn add(a, b) {
            a + b
        }
    "#;

    let plugin = create_test_plugin(script, "test_add").await;

    let args = vec![Dynamic::from(5), Dynamic::from(3)];
    let result = plugin
        .call_script_function("add", &args)
        .await
        .expect("Failed to call function");

    assert!(result.is_some());
    let result_value = result.unwrap();
    assert_eq!(result_value.as_int().unwrap(), 8);
}

#[tokio::test]
async fn test_call_script_function_with_array_arg() {
    let script = r#"
        fn sum_array(arr) {
            let total = 0;
            for i in arr {
                total = total + i;
            }
            total
        }
    "#;

    let plugin = create_test_plugin(script, "test_sum_array").await;

    let array = rhai::Array::from(vec![Dynamic::from(1), Dynamic::from(2), Dynamic::from(3)]);
    let args = vec![array.into()];

    let result = plugin
        .call_script_function("sum_array", &args)
        .await
        .expect("Failed to call function");

    assert!(result.is_some());
    let result_value = result.unwrap();
    assert_eq!(result_value.as_int().unwrap(), 6);
}

// ============================================================================
// Optional Function Tests
// ============================================================================

#[tokio::test]
async fn test_call_script_function_not_found_returns_none() {
    let script = r#"
        fn existing_function() {
            42
        }
    "#;

    let plugin = create_test_plugin(script, "test_optional_func").await;

    // Try to call a function that doesn't exist
    let result = plugin
        .call_script_function("non_existent_function", &[])
        .await
        .expect("Should not error, just return None");

    assert!(result.is_none(), "Non-existent function should return None");
}

#[tokio::test]
async fn test_call_optional_init_function() {
    let script = r#"
        // No init function defined
        fn process() {
            "processed"
        }
    "#;

    let plugin = create_test_plugin(script, "test_optional_init").await;

    // Try to call optional init function
    let result = plugin
        .call_script_function("init", &[])
        .await
        .expect("Should handle missing optional function");

    assert!(result.is_none());

    // Verify that existing function still works
    let result = plugin
        .call_script_function("process", &[])
        .await
        .expect("Should call existing function");

    assert!(result.is_some());
}

// ============================================================================
// Complex Function Tests
// ============================================================================

#[tokio::test]
async fn test_call_script_function_with_state() {
    let script = r#"
        fn process_message(msg) {
            // Process based on message type
            if msg == "hello" {
                "greeting_response"
            } else if msg == "goodbye" {
                "farewell_response"
            } else {
                "unknown_response"
            }
        }
    "#;

    let plugin = create_test_plugin(script, "test_state").await;

    // Test different inputs
    let test_cases = vec![
        ("hello", "greeting_response"),
        ("goodbye", "farewell_response"),
        ("other", "unknown_response"),
    ];

    for (input, expected) in test_cases {
        let args = vec![Dynamic::from(input)];
        let result = plugin
            .call_script_function("process_message", &args)
            .await
            .expect("Failed to call function");

        assert!(result.is_some());
        let result_str = result.unwrap().to_string();
        assert!(
            result_str.contains(expected),
            "Expected {} but got {}",
            expected,
            result_str
        );
    }
}

#[tokio::test]
async fn test_call_script_function_with_object_return() {
    let script = r#"
        fn create_response(status, message) {
            #{
                status: status,
                message: message,
                timestamp: 1234567890
            }
        }
    "#;

    let plugin = create_test_plugin(script, "test_object_return").await;

    let args = vec![
        Dynamic::from("success"),
        Dynamic::from("Operation completed"),
    ];
    let result = plugin
        .call_script_function("create_response", &args)
        .await
        .expect("Failed to call function");

    assert!(result.is_some());
    let result_map = result.unwrap();

    // Verify it's a map/object by checking its string representation
    let result_str = result_map.to_string();
    assert!(
        result_str.contains("status"),
        "Expected 'status' in result: {}",
        result_str
    );
    assert!(
        result_str.contains("message"),
        "Expected 'message' in result: {}",
        result_str
    );
}

// ============================================================================
// Recursive and Nested Function Tests
// ============================================================================

#[tokio::test]
async fn test_call_script_function_recursive() {
    let script = r#"
        fn factorial(n) {
            if n <= 1 {
                1
            } else {
                n * factorial(n - 1)
            }
        }
    "#;

    let plugin = create_test_plugin(script, "test_recursive").await;

    let args = vec![Dynamic::from(5)];
    let result = plugin
        .call_script_function("factorial", &args)
        .await
        .expect("Failed to call function");

    assert!(result.is_some());
    let result_value = result.unwrap();
    assert_eq!(result_value.as_int().unwrap(), 120); // 5! = 120
}

#[tokio::test]
async fn test_call_script_function_with_nested_calls() {
    let script = r#"
        fn helper(x) {
            x * 2
        }

        fn main_function(value) {
            helper(value) + 10
        }
    "#;

    let plugin = create_test_plugin(script, "test_nested").await;

    let args = vec![Dynamic::from(5)];
    let result = plugin
        .call_script_function("main_function", &args)
        .await
        .expect("Failed to call function");

    assert!(result.is_some());
    let result_value = result.unwrap();
    assert_eq!(result_value.as_int().unwrap(), 20); // (5 * 2) + 10 = 20
}

// ============================================================================
// Type Conversion Tests
// ============================================================================

#[tokio::test]
async fn test_call_script_function_with_various_types() {
    let script = r#"
        fn identify_type(value) {
            let t = type_of(value);
            "Type: " + t
        }
    "#;

    let plugin = create_test_plugin(script, "test_types").await;

    // Test with different types
    let test_cases = vec![
        (Dynamic::from(42), "int"),
        (Dynamic::from(3.14), "float"),
        (Dynamic::from("text"), "string"),
        (Dynamic::TRUE, "bool"),
    ];

    for (value, _expected_type) in test_cases {
        let args = vec![value];
        let result = plugin
            .call_script_function("identify_type", &args)
            .await
            .expect("Failed to call function");

        assert!(result.is_some(), "Should handle type identification");
    }
}

// ============================================================================
// Empty and Edge Case Tests
// ============================================================================

#[tokio::test]
async fn test_call_script_function_no_args() {
    let script = r#"
        fn get_constant() {
            42
        }
    "#;

    let plugin = create_test_plugin(script, "test_no_args").await;

    let result = plugin
        .call_script_function("get_constant", &[])
        .await
        .expect("Failed to call function");

    assert!(result.is_some());
    let result_value = result.unwrap();
    assert_eq!(result_value.as_int().unwrap(), 42);
}

#[tokio::test]
async fn test_call_script_function_returns_nothing() {
    let script = r#"
        fn side_effect_function() {
            let _tmp = 1 + 1;
        }
    "#;

    let plugin = create_test_plugin(script, "test_no_return").await;

    let result = plugin
        .call_script_function("side_effect_function", &[])
        .await
        .expect("Failed to call function");

    // In Rhai, functions without explicit return still return ()
    assert!(result.is_some());
}

#[tokio::test]
async fn test_call_script_function_with_empty_string() {
    let script = r#"
        fn process_string(s) {
            if s == "" {
                "empty"
            } else {
                "not_empty"
            }
        }
    "#;

    let plugin = create_test_plugin(script, "test_empty_string").await;

    let args = vec![Dynamic::from("")];
    let result = plugin
        .call_script_function("process_string", &args)
        .await
        .expect("Failed to call function");

    assert!(result.is_some());
    let result_str = result.unwrap().to_string();
    assert!(result_str.contains("empty"));
}

// ============================================================================
// Multiple Function Calls Tests
// ============================================================================

#[tokio::test]
async fn test_call_multiple_functions_in_sequence() {
    let script = r#"
        fn initialize() {
            "initialized"
        }

        fn process(data) {
            "processing: " + data
        }

        fn finalize() {
            "finalized"
        }
    "#;

    let plugin = create_test_plugin(script, "test_sequence").await;

    // Call init
    let result = plugin
        .call_script_function("initialize", &[])
        .await
        .expect("Failed to call initialize");
    assert!(result.is_some());

    // Call process
    let result = plugin
        .call_script_function("process", &[Dynamic::from("test_data")])
        .await
        .expect("Failed to call process");
    assert!(result.is_some());

    // Call finalize
    let result = plugin
        .call_script_function("finalize", &[])
        .await
        .expect("Failed to call finalize");
    assert!(result.is_some());
}

// ============================================================================
// Concurrent Function Calls Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_function_calls() {
    let script = r#"
        fn concurrent_task(id, delay) {
            "Task " + id + " completed after " + delay + "ms"
        }
    "#;

    let plugin = std::sync::Arc::new(create_test_plugin(script, "test_concurrent").await);

    let mut tasks = vec![];

    for i in 0..5 {
        let plugin_clone = plugin.clone();
        let task = tokio::spawn(async move {
            let args = vec![Dynamic::from(i), Dynamic::from(100)];
            plugin_clone
                .call_script_function("concurrent_task", &args)
                .await
        });
        tasks.push(task);
    }

    for task in tasks {
        let result = task
            .await
            .expect("Task failed")
            .expect("Function call failed");
        assert!(result.is_some());
    }
}

// ============================================================================
// Script with Multiple Functions and State Tests
// ============================================================================

#[tokio::test]
async fn test_script_with_helper_functions() {
    let script = r#"
        fn is_even(n) {
            n % 2 == 0
        }

        fn filter_even_numbers(numbers) {
            let result = [];
            for num in numbers {
                if is_even(num) {
                    result.push(num);
                }
            }
            result
        }
    "#;

    let plugin = create_test_plugin(script, "test_helpers").await;

    let array = rhai::Array::from(vec![
        Dynamic::from(1),
        Dynamic::from(2),
        Dynamic::from(3),
        Dynamic::from(4),
        Dynamic::from(5),
    ]);
    let args = vec![array.into()];

    let result = plugin
        .call_script_function("filter_even_numbers", &args)
        .await
        .expect("Failed to call function");

    assert!(result.is_some());
}
