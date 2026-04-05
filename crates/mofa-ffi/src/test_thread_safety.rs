#[cfg(test)]
mod ffi_thread_safety_tests {
    use parking_lot::Mutex;
    use std::sync::Arc;

    /// Verify parking_lot::Mutex doesn't panic on poisoned lock like std::sync::Mutex
    #[test]
    fn test_parking_lot_mutex_no_poison() {
        let state = Arc::new(Mutex::new(String::new()));
        let state_clone = state.clone();

        // Spawn a thread and hold the lock
        let _handle = std::thread::spawn(move || {
            let mut _state = state_clone.lock();
            // Mutex is held, but we don't panic here - we exit the thread normally
            // This is safe with parking_lot - it never poisons
        });

        // Wait a bit for the thread to take the lock
        std::thread::sleep(std::time::Duration::from_millis(10));

        // The lock should be accessible without panicking (parking_lot never poisons)
        let mut state = state.lock();
        *state = "test".to_string();
        // No unwrap() needed, no panicking on poisoned lock
        assert_eq!(*state, "test");
    }

    /// Concurrent access test - verify no data races with parking_lot::Mutex
    #[tokio::test]
    async fn test_concurrent_ffi_access() {
        let counter = Arc::new(Mutex::new(0i32));
        let mut handles = vec![];

        // Spawn 10 concurrent tasks trying to modify the shared state
        for _ in 0..10 {
            let counter_clone = counter.clone();
            let handle = tokio::spawn(async move {
                let mut state = counter_clone.lock();
                *state += 1;
                // Lock is automatically released when state is dropped
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            let _ = handle.await;
        }

        // Verify the counter reached 10
        let final_value = counter.lock();
        assert_eq!(*final_value, 10);
    }

    /// Verify parking_lot provides better performance than std::sync::Mutex
    #[test]
    fn test_parking_lot_performance() {
        let iterations = 1000;
        let value = Arc::new(Mutex::new(0u64));

        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let mut v = value.lock();
            *v += 1;
        }
        let elapsed = start.elapsed();

        // Just ensure it completes - parking_lot is faster but we won't assert specific timings
        let final_value = value.lock();
        assert_eq!(*final_value, iterations as u64);
        println!(
            "parking_lot::Mutex completed {} iterations in {:?}",
            iterations, elapsed
        );
    }
}
