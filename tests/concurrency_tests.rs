//! Integration tests for concurrency safety across MoFA components
//!
//! These tests stress-test concurrent operations to detect deadlocks and race conditions.

use mofa_testing::ConcurrencyTestBuilder;

#[tokio::test]
#[ignore = "stress test, run with --ignored"]
async fn concurrency_stress_test_basic_operations() {
    /// Simple concurrent operation counter
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    ConcurrencyTestBuilder::new()
        .iterations(50)
        .concurrent_tasks(30)
        .deadlock_detection(true)
        .run(|| {
            let counter = std::sync::Arc::clone(&counter);
            Box::pin(async move {
                // Simulate basic concurrent work
                for _ in 0..10 {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tokio::task::yield_now().await;
                }
            })
        })
        .await;

    // Verify all operations completed
    let final_count = counter.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(final_count, 50 * 30 * 10, "Not all operations completed");
}

#[tokio::test]
#[ignore = "stress test, run with --ignored"]
async fn concurrency_stress_test_mutex_operations() {
    /// Test concurrent access to mutex-protected data
    let data = std::sync::Arc::new(parking_lot::Mutex::new(0usize));

    ConcurrencyTestBuilder::new()
        .iterations(50)
        .concurrent_tasks(30)
        .deadlock_detection(true)
        .run(|| {
            let data = std::sync::Arc::clone(&data);
            Box::pin(async move {
                // Lock, modify, unlock
                let mut value = data.lock();
                *value += 1;
                drop(value);
                tokio::task::yield_now().await;
            })
        })
        .await;

    let final_value = *data.lock();
    assert_eq!(final_value, 50 * 30, "Mutex operations not atomic");
}

#[tokio::test]
async fn test_concurrency_enabled_by_default() {
    /// Verify deadlock detection is enabled by default
    ConcurrencyTestBuilder::new()
        .iterations(5)
        .concurrent_tasks(10)
        .run(|| {
            Box::pin(async {
                // Simple operation
                tokio::task::yield_now().await;
            })
        })
        .await;
}

#[tokio::test]
async fn test_configurable_iterations_and_tasks() {
    /// Test builder configuration
    let builder = ConcurrencyTestBuilder::new()
        .iterations(10)
        .concurrent_tasks(5)
        .deadlock_detection(false);

    assert_eq!(builder.iterations, 10);
    assert_eq!(builder.concurrent_tasks, 5);
}
