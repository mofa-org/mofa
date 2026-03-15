//! Concurrency testing framework for deadlock detection
//!
//! Provides tools to stress-test concurrent code and detect deadlocks in MoFA.

use std::future::Future;
use tracing::{error, info};

/// Builder for concurrent stress tests with deadlock detection
///
/// # Example
///
/// ```rust,no_run
/// use mofa_testing::concurrency::ConcurrencyTestBuilder;
///
/// #[tokio::test]
/// async fn test_concurrent_operations() {
///     ConcurrencyTestBuilder::new()
///         .iterations(100)
///         .concurrent_tasks(50)
///         .run(|| Box::pin(async {
///             // Test operations here
///         }))
///         .await;
/// }
/// ```
pub struct ConcurrencyTestBuilder {
    iterations: usize,
    concurrent_tasks: usize,
    enable_deadlock_detection: bool,
}

impl ConcurrencyTestBuilder {
    /// Create a new concurrency test builder with defaults
    pub fn new() -> Self {
        Self {
            iterations: 100,
            concurrent_tasks: 50,
            enable_deadlock_detection: true,
        }
    }

    /// Set number of iterations (how many times the test suite runs)
    pub fn iterations(mut self, iterations: usize) -> Self {
        self.iterations = iterations;
        self
    }

    /// Set number of concurrent tasks spawned per iteration
    pub fn concurrent_tasks(mut self, count: usize) -> Self {
        self.concurrent_tasks = count;
        self
    }

    /// Enable/disable runtime deadlock detection
    pub fn deadlock_detection(mut self, enabled: bool) -> Self {
        self.enable_deadlock_detection = enabled;
        self
    }

    /// Run the test function concurrently with deadlock detection
    ///
    /// # Panics
    ///
    /// Panics if a deadlock is detected during test execution.
    pub async fn run<F>(&self, test_fn: F) -> Self
    where
        F: Fn() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync,
    {
        info!(
            "Starting concurrency stress test: {} iterations, {} concurrent tasks",
            self.iterations, self.concurrent_tasks
        );

        for iteration in 0..self.iterations {
            // Spawn concurrent tasks
            let mut tasks = vec![];
            for _ in 0..self.concurrent_tasks {
                let task_future = test_fn();
                let handle = tokio::spawn(task_future);
                tasks.push(handle);
            }

            // Wait for all tasks to complete
            let mut panic_count = 0;
            for (idx, handle) in tasks.into_iter().enumerate() {
                match handle.await {
                    Ok(_) => {}
                    Err(e) => {
                        panic_count += 1;
                        error!(
                            "Task {} in iteration {} panicked: {:?}",
                            idx, iteration, e
                        );
                    }
                }
            }

            if panic_count > 0 {
                panic!(
                    "Iteration {}: {} tasks panicked out of {} concurrent tasks",
                    iteration, panic_count, self.concurrent_tasks
                );
            }

            // Check for deadlocks after each iteration using parking_lot
            if self.enable_deadlock_detection {
                let deadlocks = parking_lot::deadlock::check_deadlock();
                if !deadlocks.is_empty() {
                    error!("Deadlock detected in iteration {}", iteration);
                    for (i, threads) in deadlocks.iter().enumerate() {
                        error!("Deadlock #{}", i);
                        for t in threads {
                            error!(
                                "Thread {:?}\nBacktrace: {:?}",
                                t.thread_id(),
                                t.backtrace()
                            );
                        }
                    }
                    panic!("Iteration {}: Deadlock detected with {} threads", iteration, deadlocks.len());
                }
            }

            if iteration % 10 == 0 {
                info!("Completed iteration {}/{}", iteration, self.iterations);
            }
        }

        info!("Concurrency stress test completed successfully");
        Self {
            iterations: self.iterations,
            concurrent_tasks: self.concurrent_tasks,
            enable_deadlock_detection: self.enable_deadlock_detection,
        }
    }
}

impl Default for ConcurrencyTestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builder_creation() {
        let builder = ConcurrencyTestBuilder::new()
            .iterations(10)
            .concurrent_tasks(5);

        assert_eq!(builder.iterations, 10);
        assert_eq!(builder.concurrent_tasks, 5);
    }

    #[tokio::test]
    async fn test_simple_concurrent_operations() {
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        ConcurrencyTestBuilder::new()
            .iterations(10)
            .concurrent_tasks(10)
            .deadlock_detection(false)
            .run(|| {
                let counter = std::sync::Arc::clone(&counter);
                Box::pin(async move {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                })
            })
            .await;

        assert_eq!(
            counter.load(std::sync::atomic::Ordering::SeqCst),
            100
        );
    }
}
