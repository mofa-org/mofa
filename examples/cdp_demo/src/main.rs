//! This example demonstrates the Capability Discovery Protocol (CDP) introduced 
//! in the `mofa-kernel`'s `llm::cdp` module. It shows how:
//! - Providers register their capabilities (HardwareClass, Modalities, Tool Schemas, Context Windows).
//! - The `CapabilityRegistry` stores these provider manifests.
//! - The `CapabilityFilter` can dynamically discover suitable models based on strict capability queries.
//!
//! Run with: `cargo run --example cdp_demo`

use mofa_kernel::llm::cdp::{
    CapabilityFilter, CapabilityManifest, CapabilityRegistry, HardwareClass, Modality, ModelEntry,
    ToolSchemaFormat,
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting Capability Discovery Protocol (CDP) demonstration...\n");

    // ==========================================
    // 1. Defining Provider Manifests
    // ==========================================
    info!("Step 1: Constructing Capability Manifests for different providers...");

    // Provider 1: A cloud-based provider with a large vision model and a standard text model.
    let cloud_manifest = CapabilityManifest::builder("cloud-ai", "1.0.0", HardwareClass::Cloud)
        .add_model(
            ModelEntry::builder("vision-max-128k")
                .input_modalities([Modality::Text, Modality::Image])
                .output_modalities([Modality::Text])
                .max_context_tokens(128_000)
                .supports_tool_calling(true)
                .supports_streaming(true)
                .add_tool_schema_format(ToolSchemaFormat::OpenAi)
                .build(),
        )
        .add_model(
            ModelEntry::builder("text-fast-8k")
                .input_modalities([Modality::Text])
                .output_modalities([Modality::Text])
                .max_context_tokens(8_192)
                .supports_tool_calling(false)
                .supports_streaming(true)
                .build(),
        )
        .build();

    // Provider 2: A local GPU-based provider running a smaller open-weights text model.
    let local_gpu_manifest = CapabilityManifest::builder("local-ollama", "0.3.0", HardwareClass::Gpu)
        .add_model(
            ModelEntry::builder("mistral-7b-instruct")
                .input_modalities([Modality::Text])
                .output_modalities([Modality::Text])
                .max_context_tokens(8_192)
                .supports_tool_calling(true)
                .supports_streaming(true)
                .add_tool_schema_format(ToolSchemaFormat::Custom("OpenHermes".into()))
                .build(),
        )
        .build();

    // Provider 3: An Anthropic-style cloud provider with massive context and specific tool schema requirements.
    let anthropic_manifest = CapabilityManifest::builder("anthropic", "2024.1", HardwareClass::Cloud)
        .add_model(
            ModelEntry::builder("claude-3-5-sonnet")
                .input_modalities([Modality::Text, Modality::Image])
                .output_modalities([Modality::Text])
                .max_context_tokens(200_000)
                .supports_tool_calling(true)
                .supports_streaming(true)
                .add_tool_schema_format(ToolSchemaFormat::Anthropic)
                .build(),
        )
        .build();

    // ==========================================
    // 2. Populating the Registry
    // ==========================================
    info!("Step 2: Registering providers into the CapabilityRegistry...");
    let mut registry = CapabilityRegistry::new();
    
    registry.register(cloud_manifest)?;
    registry.register(local_gpu_manifest)?;
    registry.register(anthropic_manifest)?;
    
    assert_eq!(registry.len(), 3);
    info!("Successfully registered {} providers.\n", registry.len());

    // ==========================================
    // 3. Capability Filtering
    // ==========================================
    info!("Step 3: Querying the registry dynamically via CapabilityFilters:");

    // Scenario A: An agent needs to analyze an image, and expects the document to be around 100k tokens long.
    let vision_heavy_filter = CapabilityFilter::new()
        .require_input_modality(Modality::Image)
        .min_context_tokens(100_000);
    
    let vision_matches = registry.query(&vision_heavy_filter);
    info!("  Scenario A: Vision + 100k Context Token Limit:");
    for (provider, model) in &vision_matches {
        info!("    => Matched: {} / {}", provider, model);
    }
    assert_eq!(vision_matches.len(), 2); // vision-max-128k and claude-3-5-sonnet

    // Scenario B: Privacy-preserving task that MUST run on local GPU hardware.
    let local_filter = CapabilityFilter::new()
        .hardware_class(HardwareClass::Gpu);
    
    let local_matches = registry.query(&local_filter);
    info!("  Scenario B: Strict Local Hardware (GPU) execution:");
    for (provider, model) in &local_matches {
        info!("    => Matched: {} / {}", provider, model);
    }
    assert_eq!(local_matches.len(), 1); // mistral-7b-instruct
    
    // Scenario C: We have pre-formatted tools in Anthropic's specific JSON schema format and need a compatible model.
    let specific_schema_filter = CapabilityFilter::new()
        .require_tool_calling()
        .tool_schema_format(ToolSchemaFormat::Anthropic);
        
    let schema_matches = registry.query(&specific_schema_filter);
    info!("  Scenario C: Anthropic Tool Schema Format requirement:");
    for (provider, model) in &schema_matches {
        info!("    => Matched: {} / {}", provider, model);
    }
    assert_eq!(schema_matches.len(), 1); // claude-3-5-sonnet

    info!("\nCapability Discovery Protocol demonstration complete.");
    Ok(())
}
