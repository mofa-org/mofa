use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::agent::AgentResult;

/// Provides the current health status of a component or backend.
/// Allows checking a resource to determine its availability.
#[async_trait::async_trait]
pub trait HealthProbe: Send + Sync {
    /// Check the health of the underlying component.
    async fn check_health(&self) -> AgentResult<HealthStatus>;
}

/// The state of a health probe at a given time point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HealthStatus {
    /// The component is fully functional.
    Healthy,
    /// The component is functional but experiencing issues (e.g. high latency).
    Degraded,
    /// The component is completely unavailable or failing.
    Unhealthy,
    /// The status of the component could not be determined.
    Unknown,
}

/// Event emitted when a backend's health transitions to a new state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEvent {
    /// The previous health status.
    pub previous: HealthStatus,
    /// The newly transitioned health status.
    pub current: HealthStatus,
}

/// A background checker that periodically queries a `HealthProbe` and emits
/// `HealthEvent`s on an `mpsc::Sender` whenever the state transitions.
pub struct PeriodicHealthChecker {
    probe: Box<dyn HealthProbe>,
    interval: Duration,
    timeout: Duration,
}

impl PeriodicHealthChecker {
    /// Creates a new `PeriodicHealthChecker` with the given `probe`.
    pub fn new(probe: Box<dyn HealthProbe>, interval: Duration, timeout: Duration) -> Self {
        Self {
            probe,
            interval,
            timeout,
        }
    }

    /// Spawns the checker as a detached `tokio::task`.
    /// Note: The task runs indefinitely until the returned `Receiver` is dropped
    /// AND the sender channel fills up, or if explicitly cancelled by the caller
    /// via other mechanisms (e.g. `JoinHandle::abort()`).
    pub fn start(self) -> mpsc::Receiver<HealthEvent> {
        let (tx, rx) = mpsc::channel(16);
        let probe = self.probe;
        let interval = self.interval;
        let timeout = self.timeout;

        tokio::spawn(async move {
            let mut current_state = HealthStatus::Unknown;

            loop {
                tokio::time::sleep(interval).await;

                // Perform the health check with a timeout
                let check_result = tokio::time::timeout(timeout, probe.check_health()).await;

                let next_state = match check_result {
                    Ok(Ok(status)) => status,
                    Ok(Err(_)) | Err(_) => HealthStatus::Unhealthy,
                };

                // Emit event if state changed
                if next_state != current_state {
                    let event = HealthEvent {
                        previous: current_state.clone(),
                        current: next_state.clone(),
                    };

                    current_state = next_state;

                    // If the receiver is dropped, this fails and we exit the loop
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
        });

        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::time::sleep;

    /// A mock probe for testing.
    /// It fails (`Unhealthy`) the first `fail_count` times, then returns `Healthy`.
    struct MockProbe {
        fail_count: Arc<Mutex<usize>>,
    }

    #[async_trait::async_trait]
    impl HealthProbe for MockProbe {
        async fn check_health(&self) -> AgentResult<HealthStatus> {
            let mut fails = self.fail_count.lock().unwrap();
            if *fails > 0 {
                *fails -= 1;
                Ok(HealthStatus::Unhealthy)
            } else {
                Ok(HealthStatus::Healthy)
            }
        }
    }

    #[tokio::test]
    async fn test_periodic_health_checker_transitions() {
        let fail_count = Arc::new(Mutex::new(3));
        let probe = Box::new(MockProbe {
            fail_count: Arc::clone(&fail_count),
        });

        let interval = Duration::from_millis(10);
        let timeout = Duration::from_millis(5);

        let checker = PeriodicHealthChecker::new(probe, interval, timeout);
        let mut rx = checker.start();

        // 1st transition: Unknown -> Unhealthy
        let event1 = rx.recv().await.expect("Channel closed early");
        assert_eq!(event1.previous, HealthStatus::Unknown);
        assert_eq!(event1.current, HealthStatus::Unhealthy);

        // After 3 intervals, it should recover...
        // 2nd transition: Unhealthy -> Healthy
        let event2 = rx.recv().await.expect("Channel closed early");
        assert_eq!(event2.previous, HealthStatus::Unhealthy);
        assert_eq!(event2.current, HealthStatus::Healthy);
    }
}
