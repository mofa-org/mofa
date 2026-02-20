//! Progress indicators for long-running operations

use indicatif::{ProgressBar as IndicatifProgressBar, ProgressStyle as IndicatifStyle};
use std::time::Duration;

/// Progress bar wrapper for CLI operations
pub struct ProgressBar {
    inner: IndicatifProgressBar,
}

impl ProgressBar {
    /// Create a new spinner for indeterminate operations
    pub fn new_spinner(message: &str) -> Self {
        let pb = IndicatifProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(100));
        pb.set_style(
            IndicatifStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message(message.to_string());
        Self { inner: pb }
    }

    /// Create a new progress bar for determinate operations
    pub fn new(length: u64, message: &str) -> Self {
        let pb = IndicatifProgressBar::new(length);
        pb.set_style(
            IndicatifStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
                )
                .unwrap()
                .progress_chars("=>-"),
        );
        pb.set_message(message.to_string());
        Self { inner: pb }
    }

    /// Update the progress bar message
    pub fn set_message(&self, message: &str) {
        self.inner.set_message(message.to_string());
    }

    /// Increment the progress by 1
    pub fn inc(&self, delta: u64) {
        self.inner.inc(delta);
    }

    /// Set the current position
    pub fn set_position(&self, pos: u64) {
        self.inner.set_position(pos);
    }

    /// Finish the progress bar with a message
    pub fn finish_with_message(&self, message: &str) {
        self.inner.finish_with_message(message.to_string());
    }

    /// Finish the progress bar successfully
    pub fn finish(&self) {
        self.inner.finish();
    }

    /// Abort the progress bar
    pub fn abandon(&self) {
        self.inner.abandon();
    }

    /// Abandon with message
    pub fn abandon_with_message(&self, message: &str) {
        self.inner.abandon_with_message(message.to_string());
    }
}

impl Drop for ProgressBar {
    fn drop(&mut self) {
        self.inner.finish_and_clear();
    }
}

/// Progress style presets
pub struct ProgressStyle;

impl ProgressStyle {
    /// Default spinner style
    pub fn spinner() -> String {
        "{spinner:.green} {msg}".to_string()
    }

    /// Default bar style
    pub fn bar() -> String {
        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}".to_string()
    }

    /// Simple bar style
    pub fn simple_bar() -> String {
        "[{bar:40}] {pos}/{len}".to_string()
    }

    /// Elapsed only style
    pub fn elapsed() -> String {
        "{elapsed_precise} {msg}".to_string()
    }
}
