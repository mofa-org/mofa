use crate::adversarial::policy::PolicyChecker;
use crate::adversarial::report::{SecurityCaseResult, SecurityReport};
use crate::adversarial::suite::AdversarialCase;

/// Run an adversarial suite against an agent-under-test, using a pluggable policy checker.
///
/// This runner is intentionally minimal: it accepts any `agent` function that maps an input prompt
/// to an output response. This keeps the harness usable across different agent implementations
/// without forcing a particular agent trait today.
pub fn run_adversarial_suite<F>(
    suite: &[AdversarialCase],
    checker: &dyn PolicyChecker,
    agent: F,
) -> SecurityReport
where
    F: Fn(&str) -> String,
{
    let mut results = Vec::with_capacity(suite.len());

    for case in suite {
        let response = agent(&case.prompt);
        let outcome = checker.evaluate(case, &response);
        results.push(SecurityCaseResult {
            case_id: case.id.clone(),
            category: case.category,
            outcome,
        });
    }

    SecurityReport { results }
}
