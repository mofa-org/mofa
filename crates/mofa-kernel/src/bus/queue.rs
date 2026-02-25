use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, Semaphore};

use crate::bus::config::BackpressureStrategy;
use crate::bus::metrics::EventBusMetrics;
use crate::message::MessagePriority;

/// A custom async queue that implements different backpressure strategies.
#[derive(Clone)]
pub struct EventQueue {
    queue: Arc<Mutex<VecDeque<(MessagePriority, Vec<u8>)>>>,
    notify: Arc<Notify>,
    semaphore: Arc<Semaphore>,
    strategy: BackpressureStrategy,
    metrics: Arc<EventBusMetrics>,
    capacity: usize,
}

impl EventQueue {
    pub fn new(
        capacity: usize,
        strategy: BackpressureStrategy,
        metrics: Arc<EventBusMetrics>,
    ) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            notify: Arc::new(Notify::new()),
            semaphore: Arc::new(Semaphore::new(capacity)),
            strategy,
            metrics,
            capacity,
        }
    }

    /// Send a message into the queue, applying the configured backpressure strategy.
    pub async fn send(&self, priority: MessagePriority, msg: Vec<u8>) -> anyhow::Result<()> {
        match self.strategy {
            BackpressureStrategy::Block => {
                // Wait until we get a permit
                let permit = self.semaphore.acquire().await.unwrap();
                permit.forget(); // The receiver will add the permit back

                let mut q = self.queue.lock().unwrap();
                q.push_back((priority, msg));
                self.metrics.increment_buffer();
                self.notify.notify_one();
            }
            BackpressureStrategy::DropOldest => {
                if let Ok(permit) = self.semaphore.try_acquire() {
                    permit.forget();
                    let mut q = self.queue.lock().unwrap();
                    q.push_back((priority, msg));
                    self.metrics.increment_buffer();
                    self.notify.notify_one();
                } else {
                    let mut q = self.queue.lock().unwrap();
                    // Drop the oldest message
                    q.pop_front();
                    self.metrics.record_drop();
                    q.push_back((priority, msg));
                    self.notify.notify_one();
                }
            }
            BackpressureStrategy::DropLowPriority => {
                if let Ok(permit) = self.semaphore.try_acquire() {
                    permit.forget();
                    let mut q = self.queue.lock().unwrap();
                    q.push_back((priority, msg));
                    self.metrics.increment_buffer();
                    self.notify.notify_one();
                } else {
                    let mut q = self.queue.lock().unwrap();
                    // We need to drop a message.
                    // Find the oldest message in the queue with priority <= new message priority
                    // (Note: in MessagePriority: Critical=0, High=1, Low=2. So higher number = lower priority)
                    let maybe_idx = q.iter().position(|(p, _)| *p >= priority);
                    
                    if let Some(idx) = maybe_idx {
                        // Drop the lower/equal priority message
                        q.remove(idx);
                        q.push_back((priority, msg));
                        self.metrics.record_drop();
                        self.notify.notify_one();
                    } else {
                        // All messages in queue are higher priority, so drop the new message
                        self.metrics.record_drop();
                    }
                }
            }
        }
        Ok(())
    }

    /// Receive a message from the queue, blocking until one is available.
    pub async fn recv(&self) -> anyhow::Result<(MessagePriority, Vec<u8>)> {
        loop {
            // Get notified future *before* checking the queue to avoid missed wakeups
            let notified = self.notify.notified();

            let item = {
                let mut q = self.queue.lock().unwrap();
                q.pop_front()
            };

            if let Some(msg) = item {
                // Add the permit back to the semaphore since we consumed an item
                self.semaphore.add_permits(1);
                self.metrics.decrement_buffer();
                return Ok(msg);
            }

            // Await notification if queue was empty
            notified.await;
        }
    }
}
