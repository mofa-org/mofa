#![allow(missing_docs)]

use cli_production_smoke::run_full_smoke;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MoFA CLI Production Smoke Check ===\n");

    let report = run_full_smoke()?;
    let total = report.steps.len();

    for (idx, step) in report.steps.iter().enumerate() {
        let status = if step.passed { "OK" } else { "FAIL" };
        println!("[{}/{}] {} ... {}", idx + 1, total, step.name, status);
        if let Some(err) = &step.error {
            println!("  {}", err.replace('\n', "\n  "));
        }
    }

    println!();
    println!(
        "Summary: {} passed, {} failed",
        report.passed_count(),
        report.failed_count()
    );

    if report.all_passed() {
        println!("All CLI smoke checks passed.");
        Ok(())
    } else {
        return Err(format!("CLI smoke checks failed").into())
    }
}
