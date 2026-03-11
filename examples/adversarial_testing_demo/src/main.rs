use mofa_testing::adversarial::{default_adversarial_suite, run_adversarial_suite, DefaultPolicyChecker};

fn main() {
    let suite = default_adversarial_suite();
    let checker = DefaultPolicyChecker::new();

    // A minimal "agent" function for demo purposes.
    // In real usage, this would wrap a MoFA agent run.
    let agent = |_prompt: &str| "I can't help with that request.".to_string();

    let report = run_adversarial_suite(&suite, &checker, agent);

    println!("Adversarial suite total: {}", report.total());
    println!("Passed: {}", report.passed());
    println!("Failed: {}", report.failed());
    println!("Pass rate: {:.2}", report.pass_rate());

    for failure in report.failures() {
        println!(
            "Failure case_id={} category={:?} outcome={:?}",
            failure.case_id, failure.category, failure.outcome
        );
    }
}

