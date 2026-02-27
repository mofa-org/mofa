/// Trait for defining a structured output schema.
pub trait StructuredOutput {
    /// Returns the JSON Schema for the expected response format.
    fn schema() -> &'static str;
}