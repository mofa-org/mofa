//! CI/CD Integration Example
//!
//! Demonstrates how to use `mofa agent logs` and `mofa plugin install` in CI/CD pipelines:
//! - Automated plugin installation with checksum verification
//! - Log export for analysis and debugging
//! - Plugin verification before deployment
//! - Automated testing workflows

use anyhow::Result;
use std::process::Command;
use tempfile::TempDir;
use tokio::fs;

#[tokio::main]
async fn main() -> Result<()> {
    println!("CI/CD Integration Demo\n");
    println!("This example shows production-ready CLI usage in CI/CD pipelines.\n");

    let temp = TempDir::new()?;
    let workspace = temp.path();

    println!("1. Setting up CI/CD workspace: {}", workspace.display());

    // Simulate CI/CD environment
    std::env::set_var("MOFA_DATA_DIR", workspace.join("data"));
    std::env::set_var("MOFA_CONFIG_DIR", workspace.join("config"));

    println!("\n2. Automated Plugin Installation with Security");
    println!("   In CI/CD, you'd run:");
    println!("   mofa plugin install https://repo.example.com/plugin.tar.gz \\");
    println!("     --checksum abc123def456... \\");
    println!("     --verify-signature");
    println!("\n   This ensures:");
    println!("   - Plugin integrity (checksum verification)");
    println!("   - No tampering (signature verification)");
    println!("   - Reproducible builds");

    println!("\n3. Log Export for Analysis");
    println!("   Export agent logs for analysis:");
    println!("   mofa agent logs my-agent --json > logs.json");
    println!("   mofa agent logs my-agent --level ERROR --limit 100 > errors.txt");
    println!("\n   Benefits:");
    println!("   - Structured data for log aggregation (Splunk, ELK, etc.)");
    println!("   - Error-focused debugging");
    println!("   - Integration with monitoring systems");

    println!("\n4. Automated Testing Workflow");
    println!("   Example CI/CD script:");
    println!("\n   #!/bin/bash");
    println!("   set -e");
    println!("\n   # Install required plugins");
    println!("   mofa plugin install ./plugins/my-plugin \\");
    println!("     --checksum $(cat plugins/my-plugin.sha256)");
    println!("\n   # Start agent");
    println!("   mofa agent start my-agent");
    println!("\n   # Wait for agent to initialize");
    println!("   sleep 5");
    println!("\n   # Check logs for errors");
    println!("   ERROR_COUNT=$(mofa agent logs my-agent --level ERROR --json | jq '.count')");
    println!("   if [ \"$ERROR_COUNT\" -gt 0 ]; then");
    println!("     echo \"Agent has errors, failing build\"");
    println!("     exit 1");
    println!("   fi");
    println!("\n   # Export logs for artifact storage");
    println!("   mofa agent logs my-agent --json > build-logs.json");
    println!("\n   # Verify plugin is working");
    println!("   mofa plugin list | grep -q my-plugin || exit 1");

    println!("\n5. Production Deployment Checklist");
    println!("   Before deploying to production:");
    println!("   - Verify all plugins have checksums");
    println!("   - Test plugin installation in staging");
    println!("   - Export and review agent logs");
    println!("   - Verify no errors in logs");
    println!("   - Document plugin versions and checksums");

    println!("\n6. Monitoring Integration");
    println!("   Export logs for monitoring systems:");
    println!("   # Send to log aggregation");
    println!("   mofa agent logs my-agent --json | curl -X POST \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d @- https://logs.example.com/api/ingest");
    println!("\n   # Filter and alert on errors");
    println!("   mofa agent logs my-agent --level ERROR --tail | \\");
    println!("     while read line; do");
    println!("       send_alert \"Agent error: $line\"");
    println!("     done");

    println!("\nCI/CD integration patterns demonstrated!");
    println!("\nKey takeaways:");
    println!("   - Use --checksum for security in CI/CD");
    println!("   - Use --json for automation and parsing");
    println!("   - Use --level and --grep for focused debugging");
    println!("   - Export logs for analysis and compliance");
    println!("   - Integrate with monitoring systems");

    println!("\nPress Ctrl+C to exit");
    tokio::signal::ctrl_c().await?;

    Ok(())
}
