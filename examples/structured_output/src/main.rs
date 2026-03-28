//! Demonstrates JSON schema validation for structured LLM output.

use mofa_foundation::SchemaValidator;
use mofa_kernel::StructuredOutput;
use serde::{Deserialize, Serialize};

/// Example structured output type demonstrating JSON schema validation.
#[derive(Debug, Serialize, Deserialize)]
struct WeatherReport {
    city: String,
    temperature_celsius: f64,
    condition: String,
}

impl StructuredOutput for WeatherReport {
    fn schema() -> &'static str {
        r#"{
            "type": "object",
            "properties": {
                "city":                { "type": "string" },
                "temperature_celsius": { "type": "number" },
                "condition":           { "type": "string" }
            },
            "required": ["city", "temperature_celsius", "condition"]
        }"#
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let validator = SchemaValidator::new(WeatherReport::schema())?;

    // valid response — passes schema validation
    let valid = r#"{"city": "Tokyo", "temperature_celsius": 22.5, "condition": "sunny"}"#;
    match validator.validate(valid) {
        Ok(value) => {
            let report: WeatherReport = serde_json::from_value(value)?;
            println!("city: {}", report.city);
            println!("temperature: {}°C", report.temperature_celsius);
            println!("condition: {}", report.condition);
        }
        Err(e) => eprintln!("validation failed: {e}"),
    }

    // invalid response — missing required field "condition"
    let invalid = r#"{"city": "London", "temperature_celsius": 15.0}"#;
    match validator.validate(invalid) {
        Ok(_) => eprintln!("expected validation to fail"),
        Err(e) => println!("correctly rejected invalid response: {e}"),
    }

    // to use AgentExecutor with a real LLM, set OPENAI_API_KEY and:
    //
    //   use mofa_foundation::AgentExecutor;
    //   use mofa_foundation::llm::{LLMClient, OpenAIProvider, OpenAIConfig};
    //   use std::sync::Arc;
    //
    //   let provider = Arc::new(OpenAIProvider::new(OpenAIConfig::from_env()?));
    //   let client = LLMClient::new(provider);
    //   let executor = AgentExecutor::new(client, WeatherReport::schema())?;
    //   let report: WeatherReport = executor.execute("What is the weather in Tokyo?", 2).await?;

    Ok(())
}
