//! Cached regex patterns for prompt variable substitution.
//!
//! Per project standards: regex objects with high compilation costs MUST be
//! cached using LazyLock or OnceLock.

use std::sync::LazyLock;

/// Cached regex for template variable placeholders: `{var_name}`
pub(crate) static VARIABLE_PLACEHOLDER_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\{(\w+)\}").unwrap());
