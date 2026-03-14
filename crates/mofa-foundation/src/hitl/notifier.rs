//! Review Notifier
//!
//! Multi-channel notification system for review requests

use crate::hitl::error::FoundationHitlError;
use crate::hitl::webhook::{WebhookConfig, WebhookDelivery};
use mofa_kernel::hitl::ReviewRequest;
use std::sync::Arc;

/// Notification channel types
#[derive(Debug, Clone)]
pub enum NotificationChannel {
    /// Webhook notification
    Webhook(WebhookConfig),
    /// Event bus (future: for internal event system)
    EventBus,
    /// Log only (for development)
    Log,
}

/// Review notifier
pub struct ReviewNotifier {
    channels: Vec<NotificationChannel>,
    webhook_delivery: Option<Arc<WebhookDelivery>>,
}

impl ReviewNotifier {
    /// Create a new review notifier
    pub fn new(channels: Vec<NotificationChannel>) -> Self {
        let webhook_delivery = channels.iter().find_map(|ch| match ch {
            NotificationChannel::Webhook(config) => {
                Some(Arc::new(WebhookDelivery::new(config.clone())))
            }
            _ => None,
        });

        Self {
            channels,
            webhook_delivery,
        }
    }

    /// Notify about a new review request
    pub async fn notify_review_created(
        &self,
        review: &ReviewRequest,
    ) -> Result<(), FoundationHitlError> {
        for channel in &self.channels {
            match channel {
                NotificationChannel::Webhook(_) => {
                    if let Some(ref webhook) = self.webhook_delivery {
                        if let Err(e) = webhook.deliver(review, "review.created").await {
                            tracing::warn!("Webhook notification failed: {}", e);
                            // Continue with other channels
                        }
                    }
                }
                NotificationChannel::EventBus => {
                    // Future: emit to event bus
                    tracing::debug!("Event bus notification (not implemented yet)");
                }
                NotificationChannel::Log => {
                    tracing::info!(
                        "Review created: {} (execution: {})",
                        review.id.as_str(),
                        review.execution_id
                    );
                }
            }
        }
        Ok(())
    }

    /// Notify about a review resolution
    pub async fn notify_review_resolved(
        &self,
        review: &ReviewRequest,
    ) -> Result<(), FoundationHitlError> {
        for channel in &self.channels {
            match channel {
                NotificationChannel::Webhook(_) => {
                    if let Some(ref webhook) = self.webhook_delivery {
                        if let Err(e) = webhook.deliver(review, "review.resolved").await {
                            tracing::warn!("Webhook notification failed: {}", e);
                        }
                    }
                }
                NotificationChannel::EventBus => {
                    tracing::debug!("Event bus notification (not implemented yet)");
                }
                NotificationChannel::Log => {
                    tracing::info!(
                        "Review resolved: {} (status: {:?})",
                        review.id.as_str(),
                        review.status
                    );
                }
            }
        }
        Ok(())
    }
}

impl Default for ReviewNotifier {
    fn default() -> Self {
        Self::new(vec![NotificationChannel::Log])
    }
}
