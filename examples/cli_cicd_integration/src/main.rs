//! CI/CD Integration Example
//!
//! Demonstrates how to use `mofa agent logs` and `mofa plugin install` in CI/CD pipelines:
//! - Automated plugin installation with checksum verification
//! - Log export for analysis and debugging
//! - Plugin verification before deployment
//! - Automated testing workflows

use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("CI/CD Integration Demo\n");
    println!("This example shows production-ready CLI usage in CI/CD pipelines.\n");

    let temp = TempDir::new()?;
    let workspace = temp.path();

    println!("1. Setting up CI/CD workspace: {}", workspace.display());

    // simulate ci/cd environment (in real ci you'd export these)
    let data_dir = workspace.join("data");
    let config_dir = workspace.join("config");
    println!("MOFA_DATA_DIR={}", data_dir.display());
    println!("MOFA_CONFIG_DIR={}", config_dir.display());

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
