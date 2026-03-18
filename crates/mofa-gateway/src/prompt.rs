//! Shared prompt-building helpers for OpenAI-compatible chat requests.

/// Build a deterministic prompt string from role/content chat messages.
///
/// Format: one message per line as `"<role>: <content>"`.
pub fn build_chat_prompt<'a, I>(messages: I) -> String
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    messages
        .into_iter()
        .map(|(role, content)| format!("{role}: {content}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_builder_preserves_role_and_order() {
        let prompt = build_chat_prompt([
            ("system", "You are helpful"),
            ("user", "What is Rust?"),
            ("assistant", "A language"),
        ]);

        assert_eq!(
            prompt,
            "system: You are helpful\nuser: What is Rust?\nassistant: A language"
        );
    }
}
