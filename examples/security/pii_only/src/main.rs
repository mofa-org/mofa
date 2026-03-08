//! PII Redaction Example
//!
//! This example demonstrates PII detection and redaction capabilities in MoFA.
//! It shows how to:
//! - Detect various types of PII (email, phone, credit card, SSN, etc.)
//! - Redact PII using different strategies (mask, hash, remove, replace)
//! - Configure category-specific redaction strategies
//! - Handle GDPR compliance requirements
//!
//! Run with: `cargo run --example pii_only`

use mofa_foundation::security::{RegexPiiDetector, RegexPiiRedactor};
use mofa_kernel::security::{
    PiiDetector, PiiRedactor, RedactionStrategy, SensitiveDataCategory,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("MoFA PII Redaction - Example\n");

    // ============================================================================
    // Step 1: Create PII Detector
    // ============================================================================
    println!("Step 1: Creating PII detector...");

    let detector = RegexPiiDetector::new();

    println!("  Detector ready");
    println!();

    // ============================================================================
    // Step 2: Detect PII in Sample Text
    // ============================================================================
    println!("Step 2: Detecting PII in sample text...\n");

    let sample_text = r#"
        Customer Information:
        Name: John Doe
        Email: john.doe@example.com
        Phone: (555) 123-4567
        Credit Card: 4111-1111-1111-1111
        SSN: 123-45-6789
        IP Address: 192.168.1.100
        API Key: sk-1234567890abcdef1234567890abcdef
    "#;

    let detections = detector.detect(sample_text).await?;

    println!("  Detected {} PII items:\n", detections.len());
    for detection in &detections {
        println!(
            "    - {}: {} (position: {}-{})",
            match detection.category {
                SensitiveDataCategory::Email => "email",
                SensitiveDataCategory::Phone => "phone",
                SensitiveDataCategory::CreditCard => "credit_card",
                SensitiveDataCategory::Ssn => "ssn",
                SensitiveDataCategory::IpAddress => "ip_address",
                SensitiveDataCategory::ApiKey => "api_key",
                SensitiveDataCategory::Custom(ref s) => s,
                _ => "unknown",
            },
            detection.original,
            detection.start,
            detection.end
        );
    }
    println!();

    // ============================================================================
    // Step 3: Redaction Strategies
    // ============================================================================
    println!("Step 3: Demonstrating different redaction strategies...\n");

    let strategies = vec![
        ("Mask", RedactionStrategy::Mask),
        ("Hash", RedactionStrategy::Hash),
        ("Remove", RedactionStrategy::Remove),
        ("Replace", RedactionStrategy::Replace("[REDACTED]".to_string())),
    ];

    for (name, strategy) in strategies {
        let redactor = RegexPiiRedactor::new()
            .with_default_strategy(strategy.clone());

        let result = redactor.redact(sample_text, &strategy).await?;

        println!("  Strategy: {}", name);
        println!("    Redacted {} items", result.matches.len());
        let categories: Vec<String> = result
            .matches
            .iter()
            .map(|m| match m.category {
                SensitiveDataCategory::Email => "email".to_string(),
                SensitiveDataCategory::Phone => "phone".to_string(),
                SensitiveDataCategory::CreditCard => "credit_card".to_string(),
                SensitiveDataCategory::Ssn => "ssn".to_string(),
                SensitiveDataCategory::IpAddress => "ip_address".to_string(),
                SensitiveDataCategory::ApiKey => "api_key".to_string(),
                SensitiveDataCategory::Custom(ref s) => format!("custom:{}", s),
                _ => "unknown".to_string(),
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        println!("    Categories: {}", categories.join(", "));
        println!("    Sample output:");
        println!("      {}", result.redacted_text.lines().skip(2).next().unwrap_or(""));
        println!();
    }

    // ============================================================================
    // Step 4: Category-Specific Strategies
    // ============================================================================
    println!("Step 4: Category-specific redaction strategies...\n");

    let gdpr_redactor = RegexPiiRedactor::new()
        .with_default_strategy(RedactionStrategy::Hash)
        .with_category_strategy(
            SensitiveDataCategory::Ssn,
            RedactionStrategy::Remove, // GDPR: Remove SSNs entirely
        )
        .with_category_strategy(
            SensitiveDataCategory::Email,
            RedactionStrategy::Hash, // Hash emails for audit trail
        )
        .with_category_strategy(
            SensitiveDataCategory::CreditCard,
            RedactionStrategy::Hash, // Hash credit cards
        );

    let gdpr_result = gdpr_redactor.redact(sample_text, &RedactionStrategy::Hash).await?;

    println!("  GDPR-compliant redaction:");
    println!("    Redacted {} items", gdpr_result.matches.len());
    let categories: Vec<String> = gdpr_result
        .matches
        .iter()
        .map(|m| match m.category {
            SensitiveDataCategory::Email => "email".to_string(),
            SensitiveDataCategory::Phone => "phone".to_string(),
            SensitiveDataCategory::CreditCard => "credit_card".to_string(),
            SensitiveDataCategory::Ssn => "ssn".to_string(),
            SensitiveDataCategory::IpAddress => "ip_address".to_string(),
            SensitiveDataCategory::ApiKey => "api_key".to_string(),
            SensitiveDataCategory::Custom(ref s) => format!("custom:{}", s),
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    println!("    Categories: {}", categories.join(", "));
    println!("    Redacted text:");
    for line in gdpr_result.redacted_text.lines().skip(2).take(8) {
        if !line.trim().is_empty() {
            println!("      {}", line);
        }
    }
    println!();

    // ============================================================================
    // Step 5: Real-World Scenarios
    // ============================================================================
    println!("Step 5: Real-world scenarios...\n");

    // Scenario 1: Customer support ticket
    let ticket = r#"
        Ticket #12345
        Customer: customer@example.com
        Phone: 555-987-6543
        Issue: My credit card 5555-5555-5555-4444 was charged incorrectly.
        Please help resolve this billing issue.
    "#;

    println!("  Scenario 1: Customer support ticket");
    let ticket_detections = detector.detect(ticket).await?;
    println!("    Detected {} PII items", ticket_detections.len());

    let ticket_redacted = RegexPiiRedactor::new()
        .with_default_strategy(RedactionStrategy::Mask)
        .redact(ticket, &RedactionStrategy::Mask)
        .await?;

    println!("    Redacted text (safe for LLM):");
    println!("      {}", ticket_redacted.redacted_text.lines().skip(1).next().unwrap_or(""));
    println!();

    // Scenario 2: Log file with PII
    let log_entry = r#"
        [2025-01-15 10:30:00] User login: user@company.com from IP 10.0.0.1
        [2025-01-15 10:31:00] API call: sk-abc123def456ghi789jkl012mno345pqr678
        [2025-01-15 10:32:00] Payment processed: Card ending in 4444
    "#;

    println!("  Scenario 2: Log file with PII");
    let log_detections = detector.detect(log_entry).await?;
    println!("    Detected {} PII items", log_detections.len());

    let log_redacted = RegexPiiRedactor::new()
        .with_default_strategy(RedactionStrategy::Hash)
        .redact(log_entry, &RedactionStrategy::Hash)
        .await?;

    println!("    Redacted log (safe for storage):");
    for line in log_redacted.redacted_text.lines().skip(1).take(3) {
        if !line.trim().is_empty() {
            println!("      {}", line);
        }
    }
    println!();

    // ============================================================================
    // Step 6: Performance Test
    // ============================================================================
    println!("Step 6: Performance test...\n");

    use std::time::Instant;

    let test_text = "Email: user@example.com, Phone: (555) 123-4567. ".repeat(100);
    let redactor = RegexPiiRedactor::new();

    let start = Instant::now();
    for _ in 0..100 {
        let _ = redactor.redact(&test_text, &RedactionStrategy::Mask).await?;
    }
    let elapsed = start.elapsed();

    println!("  Processed 100 redactions in {:?}", elapsed);
    println!("  Average: {:.2}ms per redaction", elapsed.as_millis() as f64 / 100.0);
    println!();

    println!("PII redaction example completed successfully!");
    println!();
    println!("Next steps:");
    println!("  1. Integrate PII redaction into LLM pipeline hooks");
    println!("  2. Configure redaction strategies per data type");
    println!("  3. Set up audit logging for redaction events");
    println!("  4. Add custom PII patterns if needed");

    Ok(())
}
