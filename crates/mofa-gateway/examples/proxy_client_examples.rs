//! Example: Client usage examples for the gateway proxy
//!
//! This example demonstrates various ways to interact with the gateway proxy:
//! - Listing models
//! - Getting model information
//! - Making chat completion requests
//! - Handling errors
//! - Using different client libraries
//!
//! # Prerequisites
//!
//! 1. Start mofa-local-llm server:
//!    ```bash
//!    cd mofa-local-llm && cargo run --release
//!    ```
//!
//! 2. Start gateway:
//!    ```bash
//!    cargo run --example gateway_local_llm_proxy
//!    ```
//!
//! 3. Run this example:
//!    ```bash
//!    cargo run --example proxy_client_examples
//!    ```

use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 MoFA Gateway Proxy - Client Examples");
    println!("{}", "=".repeat(60));

    let gateway_url = "http://localhost:8080";
    let client = Client::new();

    // Example 1: List all available models
    println!("\n📋 Example 1: List All Models");
    println!("{}", "-".repeat(60));

    match list_models(&client, gateway_url).await {
        Ok(models) => {
            println!(
                "✅ Found {} models:",
                models["data"].as_array().unwrap().len()
            );
            for model in models["data"].as_array().unwrap() {
                println!("  - {}", model["id"].as_str().unwrap());
            }
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    // Example 2: Get specific model information
    println!("\n📋 Example 2: Get Model Information");
    println!("{}", "-".repeat(60));

    let model_id = "qwen2.5-0.5b-instruct";
    match get_model_info(&client, gateway_url, model_id).await {
        Ok(model) => {
            println!("✅ Model Information:");
            println!("  ID: {}", model["id"].as_str().unwrap());
            println!("  Object: {}", model["object"].as_str().unwrap());
            println!(
                "  Owner: {}",
                model
                    .get("owned_by")
                    .and_then(|v| v.as_str())
                    .unwrap_or("N/A")
            );
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    // Example 3: Simple chat completion
    println!("\n📋 Example 3: Simple Chat Completion");
    println!("{}", "-".repeat(60));

    let messages = vec![json!({"role": "user", "content": "What is Rust programming language?"})];

    match chat_completion(&client, gateway_url, model_id, messages, 100).await {
        Ok(response) => {
            println!("✅ Chat Response:");
            let content = response["choices"][0]["message"]["content"]
                .as_str()
                .unwrap();
            println!("  {}", content);

            if let Some(usage) = response.get("usage") {
                println!("\n  Usage:");
                println!("    Prompt tokens: {}", usage["prompt_tokens"]);
                println!("    Completion tokens: {}", usage["completion_tokens"]);
                println!("    Total tokens: {}", usage["total_tokens"]);
            }
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    // Example 4: Chat with system message
    println!("\n📋 Example 4: Chat with System Message");
    println!("{}", "-".repeat(60));

    let messages = vec![
        json!({"role": "system", "content": "You are a helpful coding assistant."}),
        json!({"role": "user", "content": "Write a hello world in Rust"}),
    ];

    match chat_completion(&client, gateway_url, model_id, messages, 150).await {
        Ok(response) => {
            println!("✅ Chat Response:");
            let content = response["choices"][0]["message"]["content"]
                .as_str()
                .unwrap();
            println!("  {}", content);
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    // Example 5: Multi-turn conversation
    println!("\n📋 Example 5: Multi-turn Conversation");
    println!("{}", "-".repeat(60));

    let messages = vec![
        json!({"role": "user", "content": "What is 2+2?"}),
        json!({"role": "assistant", "content": "2+2 equals 4."}),
        json!({"role": "user", "content": "What about 3+3?"}),
    ];

    match chat_completion(&client, gateway_url, model_id, messages, 50).await {
        Ok(response) => {
            println!("✅ Chat Response:");
            let content = response["choices"][0]["message"]["content"]
                .as_str()
                .unwrap();
            println!("  {}", content);
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    // Example 6: Error handling - invalid model
    println!("\n📋 Example 6: Error Handling - Invalid Model");
    println!("{}", "-".repeat(60));

    match get_model_info(&client, gateway_url, "non-existent-model").await {
        Ok(_) => println!("✅ Model found (unexpected)"),
        Err(e) => println!("✅ Expected error: {}", e),
    }

    // Example 7: Check gateway health
    println!("\n📋 Example 7: Gateway Health Check");
    println!("{}", "-".repeat(60));

    match check_health(&client, gateway_url).await {
        Ok(health) => {
            println!("✅ Gateway Health:");
            println!("  Status: {}", health["status"].as_str().unwrap());
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    println!("\n{}", "=".repeat(60));
    println!("✨ All examples completed!");
    println!("\n💡 Tips:");
    println!("  - Use RUST_LOG=debug for detailed logs");
    println!("  - Check gateway metrics at http://localhost:8080/metrics");
    println!("  - See PROXY.md for more examples in other languages");

    Ok(())
}

/// List all available models
async fn list_models(
    client: &Client,
    gateway_url: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let response = client
        .get(format!("{}/v1/models", gateway_url))
        .send()
        .await?;

    let status = response.status();
    let body = response.json::<serde_json::Value>().await?;

    if !status.is_success() {
        return Err(format!("Request failed: {}", status).into());
    }

    Ok(body)
}

/// Get information about a specific model
async fn get_model_info(
    client: &Client,
    gateway_url: &str,
    model_id: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let response = client
        .get(format!("{}/v1/models/{}", gateway_url, model_id))
        .send()
        .await?;

    let status = response.status();
    let body = response.json::<serde_json::Value>().await?;

    if !status.is_success() {
        return Err(format!("Request failed: {}", status).into());
    }

    Ok(body)
}

/// Make a chat completion request
async fn chat_completion(
    client: &Client,
    gateway_url: &str,
    model: &str,
    messages: Vec<serde_json::Value>,
    max_tokens: u32,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let request_body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": 0.7,
    });

    let response = client
        .post(format!("{}/v1/chat/completions", gateway_url))
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    let body = response.json::<serde_json::Value>().await?;

    if !status.is_success() {
        return Err(format!("Request failed: {}", status).into());
    }

    Ok(body)
}

/// Check gateway health
async fn check_health(
    client: &Client,
    gateway_url: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let response = client.get(format!("{}/health", gateway_url)).send().await?;

    let status = response.status();
    let body = response.json::<serde_json::Value>().await?;

    if !status.is_success() {
        return Err(format!("Request failed: {}", status).into());
    }

    Ok(body)
}
