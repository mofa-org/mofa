//! Convenience assertion macros for mock verification.
//!
//! Reduces boilerplate when checking that mocks were called as expected.

/// Assert the named tool was called at least once.
///
/// # Example
/// ```ignore
/// assert_tool_called!(tool, "search");
/// ```
#[macro_export]
macro_rules! assert_tool_called {
    ($tool:expr, $name:expr) => {{
        use mofa_foundation::agent::components::tool::SimpleTool as _;
        assert_eq!(
            $tool.name(),
            $name,
            "Tool name mismatch: expected '{}', got '{}'",
            $name,
            $tool.name()
        );
        let count = $tool.call_count().await;
        assert!(
            count > 0,
            "Expected tool '{}' to be called at least once, but it was never called",
            $name
        );
    }};
}

/// Assert the tool received a call with the given JSON arguments.
///
/// # Example
/// ```ignore
/// assert_tool_called_with!(tool, json!({"query": "rust"}));
/// ```
#[macro_export]
macro_rules! assert_tool_called_with {
    ($tool:expr, $args:expr) => {{
        let history = $tool.history().await;
        let expected = $args;
        assert!(
            history.iter().any(|h| h.arguments == expected),
            "Expected tool to be called with arguments {:?}, but no matching call found in history: {:?}",
            expected,
            history.iter().map(|h| &h.arguments).collect::<Vec<_>>()
        );
    }};
}

/// Assert the LLM backend's `infer()` was called at least once.
///
/// # Example
/// ```ignore
/// assert_infer_called!(backend);
/// ```
#[macro_export]
macro_rules! assert_infer_called {
    ($backend:expr) => {{
        let count = $backend.call_count();
        assert!(
            count > 0,
            "Expected LLM backend infer() to be called at least once, but call_count is 0"
        );
    }};
}

/// Assert a bus message was sent by the given sender.
///
/// # Example
/// ```ignore
/// assert_bus_message_sent!(bus, "agent-1");
/// ```
#[macro_export]
macro_rules! assert_bus_message_sent {
    ($bus:expr, $sender_id:expr) => {{
        let messages = $bus.captured_messages.read().await;
        let found = messages.iter().any(|(sid, _, _)| sid == $sender_id);
        assert!(
            found,
            "Expected a message from sender '{}', but none found among {} captured message(s)",
            $sender_id,
            messages.len()
        );
    }};
}

/// Assert a session's messages match the expected (role, content) pairs.
pub fn assert_session_messages(
    session: &mofa_foundation::agent::session::Session,
    expected: &[(&str, &str)],
) {
    assert_eq!(
        session.messages.len(),
        expected.len(),
        "Expected {} session messages, got {}",
        expected.len(),
        session.messages.len()
    );

    for (idx, (role, content)) in expected.iter().enumerate() {
        let msg = &session.messages[idx];
        assert_eq!(
            msg.role, *role,
            "Expected role '{}' at index {}, got '{}'",
            role, idx, msg.role
        );
        assert_eq!(
            msg.content, *content,
            "Expected content '{}' at index {}, got '{}'",
            content, idx, msg.content
        );
    }
}
