//! PyO3 Python bindings for MoFA
//!
//! This module provides native Python extension bindings.

use pyo3::exceptions::PyNotImplementedError;
use pyo3::prelude::*;

// Note: Python bindings are being refactored to use MoFAAgent directly.
// The PyAgentWrapper will be reimplemented to wrap MoFAAgent instead of RuntimeAgent.

/// Python module initialization
#[pymodule]
pub fn mofa(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_agents_py, m)?)?;
    Ok(())
}

/// Run a Python agent (placeholder)
#[pyfunction]
fn run_agents_py(_py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    Err(PyNotImplementedError::new_err(
        "mofa.run_agents_py is not implemented; use UniFFI LLMAgent APIs for now",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN_AGENTS_UNSUPPORTED_MESSAGE: &str =
        "mofa.run_agents_py is not implemented; use UniFFI LLMAgent APIs for now";

    #[test]
    fn run_agents_py_returns_explicit_unsupported_error() {
        Python::attach(|py| {
            let err = run_agents_py(py).expect_err("placeholder success must be removed");
            assert!(err.is_instance_of::<PyNotImplementedError>(py));
            assert!(err.to_string().contains(RUN_AGENTS_UNSUPPORTED_MESSAGE));
        });
    }
}
