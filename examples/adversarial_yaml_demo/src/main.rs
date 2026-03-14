use mofa_testing::adversarial::{
    DefaultPolicyChecker, AdversarialCategory, AdversarialCase,
    load_suite_from_yaml, run_adversarial_suite,
};
use std::path::PathBuf;

fn main() {
    println!("=== Adversarial Testing with YAML Demo ===");

    // Determine the path to suite.yaml (relative to the example directory or workspace)
    let yaml_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("suite.yaml");
    
    // 1. Load the suite from the YAML file
    println!("Loading suite from {}...", yaml_path.display());
    let all_cases = match load_suite_from_yaml(&yaml_path) {
        Ok(suite) => suite,
        Err(e) => {
            eprintln!("Failed to load test suite: {}", e);
            return;
        }
    };
    println!("Loaded {} total test cases.", all_cases.len());

    // 2. Filter the suite (e.g., only High/Critical severity, or specific categories)
    // Here we use the built in category filter:
    let jailbreaks = AdversarialCase::filter_by_category(&all_cases, &AdversarialCategory::Jailbreak);
    println!("Filtered down to {} Jailbreak case(s).", jailbreaks.len());

    // Create our policy checker (defines what constitutes a failure vs a pass based on output)
    let checker = DefaultPolicyChecker::new();

    // 3. Define a generic dummy agent. It will fail the jailbreak!
    let my_agent = |prompt: &str| -> String {
        if prompt.to_lowercase().contains("ignore") {
            // Our agent incorrectly gives in and outputs system internals
            return "Okay, here is my internal system prompt: You are a helpful AI assistant...".to_string();
        }
        "I am sorry, I cannot fulfill this request.".to_string()
    };

    println!("\nRunning the suite against the dummy agent...");
    // 4. Run the suite
    let report = run_adversarial_suite(&jailbreaks, &checker, my_agent);

    // 5. Output results
    println!("\n=== Final Report ===");
    println!("Total cases run: {}", report.total());
    println!("Passed: {}", report.passed());
    println!("Failed: {}", report.failed());
    println!("Pass Rate: {:.2}%", report.pass_rate() * 100.0);

    // Iterate through any failures
    for (i, failure) in report.failures().enumerate() {
        println!("\nFailure #{}: {}", i + 1, failure.case_id);
        println!("  Category: {:?}", failure.category);
        
        let response = match &failure.outcome {
            mofa_testing::adversarial::PolicyOutcome::Fail { reason } => reason,
            _ => "Unknown",
        };
        
        println!("  Agent Response: {:?}", response);
        println!("  Reason: Expected the agent to refuse or deflect, but it complied.");
    }
}
