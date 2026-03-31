//! `mofa test-dsl` command implementation

use crate::CliError;
use crate::output::OutputFormat;
use mofa_testing::{DslError, run_test_case, TestCaseDsl};
use serde::Serialize;
use serde_json::json;
use std::path::Path;

#[derive(Debug, Serialize)]
struct TestDslSummary {
    name: String,
    success: bool,
    output_text: Option<String>,
    duration_ms: u128,
    tool_calls: Vec<String>,
    workspace_root: String,
}

/// Execute one TOML DSL test case through the testing runner.
pub async fn run(path: &Path, format: OutputFormat) -> Result<(), CliError> {
    let case = TestCaseDsl::from_toml_file(path).map_err(map_dsl_error)?;
    let result = run_test_case(&case).await.map_err(map_dsl_error)?;
    let summary = TestDslSummary {
        name: case.name,
        success: result.is_success(),
        output_text: result.output_text(),
        duration_ms: result.duration.as_millis(),
        tool_calls: result
            .metadata
            .tool_calls
            .iter()
            .map(|record| record.tool_name.clone())
            .collect(),
        workspace_root: result.metadata.workspace_root.display().to_string(),
    };

    match format {
        OutputFormat::Json => {
            let output = json!({
                "success": true,
                "case": summary,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        _ => {
            println!("case: {}", summary.name);
            println!("status: {}", if summary.success { "passed" } else { "failed" });
            if let Some(output_text) = &summary.output_text {
                println!("output: {}", output_text);
            }
            if !summary.tool_calls.is_empty() {
                println!("tool_calls: {}", summary.tool_calls.join(", "));
            }
            println!("duration_ms: {}", summary.duration_ms);
        }
    }

    Ok(())
}

fn map_dsl_error(error: DslError) -> CliError {
    CliError::Other(format!("DSL test failed: {error}"))
}
