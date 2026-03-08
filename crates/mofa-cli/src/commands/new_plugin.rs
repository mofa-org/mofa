use crate::CliError;
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

pub fn run(name: &str, plugin_type: &str, dry_run: bool) -> Result<(), CliError> {
    if plugin_type != "compile-time" && plugin_type != "runtime" {
        return Err(CliError::Other(format!("Unknown plugin type: {}. Must be 'compile-time' or 'runtime'.", plugin_type)));
    }

    if dry_run {
        println!("{} Dry run: scaffolding new MoFA {} plugin: {}", "→".yellow(), plugin_type.cyan(), name.cyan());
    } else {
        println!("{} Creating new MoFA {} plugin: {}", "→".green(), plugin_type.cyan(), name.cyan());
    }

    if plugin_type == "compile-time" {
        let project_dir = PathBuf::from(name);
        
        if !dry_run {
            println!("  Directory: {}", project_dir.display());
            fs::create_dir_all(&project_dir)?;
            fs::create_dir_all(project_dir.join("src"))?;
        }

        let struct_name = to_pascal_case(name);

        let cargo_toml = format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
mofa-sdk = "0.1"
anyhow = "1"
async-trait = "0.1"
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
"#
        );

        let lib_rs = format!(
            r#"use mofa_sdk::kernel::plugin::{{AgentPlugin, PluginContext, PluginError, PluginMetadata, PluginState, PluginType, PluginResult}};
use async_trait::async_trait;
use serde::{{Deserialize, Serialize}};
use std::any::Any;

/// See https://docs.rs/mofa-sdk for more information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {struct_name} {{
    metadata: PluginMetadata,
    state: PluginState,
}}

impl {struct_name} {{
    pub fn new() -> Self {{
        let mut metadata = PluginMetadata::new(
            "{name}",
            "{name} plugin",
            PluginType::Custom("custom".to_string()),
        );
        metadata.description = "A MoFA compile-time plugin".to_string();
        
        Self {{
            metadata,
            state: PluginState::Unloaded,
        }}
    }}
}}

impl Default for {struct_name} {{
    fn default() -> Self {{
        Self::new()
    }}
}}

#[async_trait]
impl AgentPlugin for {struct_name} {{
    fn metadata(&self) -> &PluginMetadata {{
        &self.metadata
    }}

    fn state(&self) -> PluginState {{
        self.state.clone()
    }}

    async fn load(&mut self, _ctx: &PluginContext) -> PluginResult<()> {{
        self.state = PluginState::Loaded;
        Ok(())
    }}

    async fn init_plugin(&mut self) -> PluginResult<()> {{
        Ok(())
    }}

    async fn start(&mut self) -> PluginResult<()> {{
        self.state = PluginState::Running;
        Ok(())
    }}

    async fn stop(&mut self) -> PluginResult<()> {{
        self.state = PluginState::Loaded;
        Ok(())
    }}

    async fn unload(&mut self) -> PluginResult<()> {{
        self.state = PluginState::Unloaded;
        Ok(())
    }}

    async fn execute(&mut self, input: String) -> PluginResult<String> {{
        Ok(format!("Executed with input: {{}}", input))
    }}

    fn as_any(&self) -> &dyn Any {{
        self
    }}

    fn as_any_mut(&mut self) -> &mut dyn Any {{
        self
    }}

    fn into_any(self: Box<Self>) -> Box<dyn Any> {{
        self
    }}
}}
"#
        );

        write_file(&project_dir.join("Cargo.toml"), &cargo_toml, dry_run)?;
        write_file(&project_dir.join("src").join("lib.rs"), &lib_rs, dry_run)?;

    } else if plugin_type == "runtime" {
        let file_path = PathBuf::from(format!("{name}.rhai"));
        if !dry_run {
            println!("  File: {}", file_path.display());
        }

        let rhai_script = r#"/// A runtime Rhai script plugin
/// See https://docs.rs/mofa-sdk for more information.

fn on_load() {
    print("Plugin loaded");
}

fn execute(input) {
    print("Executing with input: " + input);
    return input;
}
"#;
        write_file(&file_path, rhai_script, dry_run)?;
    }

    if !dry_run {
        println!("{} Plugin created successfully!", "✓".green());
    }

    Ok(())
}

fn to_pascal_case(s: &str) -> String {
    let mut pascal = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' || c == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            pascal.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            pascal.push(c);
        }
    }
    pascal
}

fn write_file(path: &PathBuf, content: &str, dry_run: bool) -> Result<(), CliError> {
    if dry_run {
        println!("\n--- {} ---", path.display());
        println!("{}", content.trim_end());
    } else {
        fs::write(path, content)?;
    }
    Ok(())
}
