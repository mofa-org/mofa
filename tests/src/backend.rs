use anyhow::Result;
use mofa_foundation::orchestrator::{ModelOrchestrator, TokenStream};
use futures::stream;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// A mock backend that implements `ModelOrchestrator`.
/// It allows developers to specify predefined responses for specific prompts,
/// enabling deterministic testing of Agent workflows without hitting real APIs.
#[derive(Clone, Default)]
pub struct MockLLMBackend {
    loaded_models: Arc<RwLock<HashSet<String>>>,
    /// Maps a prompt (or substring of a prompt) to a predefined response string
    predefined_responses: Arc<RwLock<HashMap<String, String>>>,
    /// Fallback response if no predefined prompt matches
    fallback_response: String,
}

impl MockLLMBackend {
    pub fn new() -> Self {
        Self {
            loaded_models: Arc::new(RwLock::new(HashSet::new())),
            predefined_responses: Arc::new(RwLock::new(HashMap::new())),
            fallback_response: "This is a fallback mock response.".to_string(),
        }
    }

    /// Add a predefined response for a given prompt substring.
    /// If the `prompt` contains `key`, it will return `response`.
    pub fn add_mock_response(&self, prompt_key: &str, response: &str) {
        if let Ok(mut resps) = self.predefined_responses.write() {
            resps.insert(prompt_key.to_string(), response.to_string());
        }
    }

    /// Set the fallback response for when no predefined response matches the prompt.
    pub fn set_fallback_response(&mut self, response: &str) {
        self.fallback_response = response.to_string();
    }
}

impl ModelOrchestrator for MockLLMBackend {
    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn load_model(&mut self, model_id: &str) -> Result<()> {
        if let Ok(mut models) = self.loaded_models.write() {
            models.insert(model_id.to_string());
        }
        Ok(())
    }

    fn unload_model(&mut self, model_id: &str) -> Result<()> {
        if let Ok(mut models) = self.loaded_models.write() {
            models.remove(model_id);
        }
        Ok(())
    }

    fn is_model_loaded(&self, model_id: &str) -> bool {
        if let Ok(models) = self.loaded_models.read() {
            models.contains(model_id)
        } else {
            false
        }
    }

    fn generate(&self, _model_id: &str, prompt: &str) -> Result<TokenStream> {
        let mut final_response = self.fallback_response.clone();

        if let Ok(resps) = self.predefined_responses.read() {
            for (k, v) in resps.iter() {
                if prompt.contains(k) {
                    final_response = v.clone();
                    break;
                }
            }
        }

        // Return a mock output stream
        // Tokenizer splits by space for realism
        let tokens: Vec<Result<String>> = final_response
            .split_whitespace()
            .map(|s| Ok(format!("{} ", s)))
            .collect();

        // Pin the Boxed stream to comply with TokenStream alias
        // `Send` is automatically derived because memory structures are simple
        Ok(Box::pin(stream::iter(tokens)))
    }
}
