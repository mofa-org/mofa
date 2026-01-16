//! Build script for mofa-sdk
//!
//! Generates UniFFI scaffolding when the `uniffi` feature is enabled.

fn main() {
    // Always rerun if UDL file changes
    println!("cargo:rerun-if-changed=src/mofa.udl");

    // Generate UniFFI scaffolding
    uniffi::generate_scaffolding("src/mofa.udl").expect("Failed to generate UniFFI scaffolding");
}
