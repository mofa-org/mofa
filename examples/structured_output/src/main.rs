use mofa_foundation::{AgentExecutor, SchemaValidator};
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
                "city":                 { "type": "string" },
                "temperature_celsius":  { "type": "number" },
                "condition":            { "type": "string" }
            },
            "required": ["city", "temperature_celsius", "condition"]
        }"#
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // create a validator from the schema
    let validator = SchemaValidator::new(WeatherReport::schema())?;

    // simulate a valid JSON response from an LLM
    let raw = r#"{"city": "Tokyo", "temperature_celsius": 22.5, "condition": "sunny"}"#;

    match validator.validate(raw) {
        Ok(value) => {
            let report: WeatherReport = serde_json::from_value(value)?;
            println!("city: {}", report.city);
            println!("temperature: {}°C", report.temperature_celsius);
            println!("condition: {}", report.condition);
        }
        Err(e) => eprintln!("validation failed: {e}"),
    }

    Ok(())
}
