//! Prompt Injection Guard Implementation
//!
//! Detects and prevents prompt injection attacks.

pub mod regex;

pub use regex::RegexPromptGuard;
