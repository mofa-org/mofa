//! Hugging Face Model Downloader
//!
//! Handles downloading TTS models from Hugging Face Hub
//! with progress tracking, checksum validation, and retry logic.

use super::cache::{ModelCache, ModelMetadata};
use mofa_kernel::plugin::{PluginError, PluginResult};
use backoff::ExponentialBackoff;
use backoff::future::retry;
use futures::stream::StreamExt;
use md5::{Digest, Md5};
use reqwest::Client;
use std::fs;
use std::io::Read;
use std::time::{Duration, SystemTime};
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

/// Download progress callback type
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Model downloader configuration
pub struct DownloadConfig {
    /// Hugging Face model ID (e.g., "hexgrad/Kokoro-82M")
    pub model_id: String,
    /// Specific file to download (e.g., "kokoro-v0_19.onnx")
    pub filename: String,
    /// Expected checksum (MD5 hex string)
    pub checksum: Option<String>,
    /// Download timeout in seconds
    pub timeout_secs: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Progress callback (optional)
    pub progress_callback: Option<ProgressCallback>,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            model_id: "hexgrad/Kokoro-82M".to_string(),
            filename: "kokoro-v0_19.onnx".to_string(),
            checksum: None,
            timeout_secs: 600,
            max_retries: 3,
            progress_callback: None,
        }
    }
}

impl Clone for DownloadConfig {
    fn clone(&self) -> Self {
        Self {
            model_id: self.model_id.clone(),
            filename: self.filename.clone(),
            checksum: self.checksum.clone(),
            timeout_secs: self.timeout_secs,
            max_retries: self.max_retries,
            progress_callback: None, // Callbacks cannot be cloned
        }
    }
}

/// Hugging Face Hub API client
pub struct HFHubClient {
    client: Client,
    api_base: String,
    download_semaphore: std::sync::Arc<Semaphore>,
}

impl HFHubClient {
    /// Create a new Hugging Face Hub client
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(600))
                .build()
                .expect("Failed to create HTTP client"),
            api_base: "https://huggingface.co".to_string(),
            download_semaphore: std::sync::Arc::new(Semaphore::new(3)), // Max 3 concurrent downloads
        }
    }

    /// Create client with custom API base (useful for mirrors)
    pub fn with_api_base(api_base: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(600))
                .build()
                .expect("Failed to create HTTP client"),
            api_base,
            download_semaphore: std::sync::Arc::new(Semaphore::new(3)),
        }
    }

    /// Get direct download URL for a model file
    pub fn get_download_url(&self, model_id: &str, filename: &str) -> String {
        format!("{}/{}/resolve/main/{}", self.api_base, model_id, filename)
    }

    /// Download model file with progress tracking
    pub async fn download_model(
        &self,
        config: DownloadConfig,
        cache: &ModelCache,
    ) -> PluginResult<std::path::PathBuf> {
        let _permit = self
            .download_semaphore
            .acquire()
            .await
            .map_err(|e| PluginError::Other(format!("Failed to acquire download semaphore: {}", e)))?;

        info!(
            "Starting download: {} / {}",
            config.model_id, config.filename
        );

        // Check if model already exists and is valid
        if cache.exists(&config.model_id).await
            && let Some(expected_checksum) = &config.checksum
            && cache
                .validate(&config.model_id, Some(expected_checksum))
                .await?
        {
            info!("Model already cached and valid: {}", config.model_id);
            return Ok(cache.model_path(&config.model_id));
        }

        // Get download URL
        let download_url = self.get_download_url(&config.model_id, &config.filename);
        let output_path = cache.model_path(&config.model_id);

        info!("Downloading from: {}", download_url);
        info!("Saving to: {:?}", output_path);

        // Perform download with retry logic
        let file_size = self
            .download_file_with_retry(&download_url, &output_path, &config)
            .await?;

        // Calculate checksum
        let actual_checksum = self.calculate_checksum(&output_path)?;

        // Validate checksum if provided
        if let Some(expected) = &config.checksum
            && actual_checksum != *expected
        {
            error!("Checksum validation failed");
            fs::remove_file(&output_path).map_err(|e| PluginError::Other(e.to_string()))?;
            return Err(PluginError::Other(format!(
                "Downloaded file checksum mismatch. Expected: {}, Got: {}",
                expected,
                actual_checksum
            )));
        }

        // Save metadata
        let metadata = ModelMetadata {
            model_id: config.model_id.clone(),
            version: "latest".to_string(),
            file_size,
            checksum: actual_checksum,
            downloaded_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            access_count: 0,
        };

        cache.save_metadata(&metadata).await?;

        info!(
            "Download completed successfully: {} ({})",
            config.model_id,
            format_bytes(file_size)
        );

        Ok(output_path)
    }

    /// Download file with retry logic and progress tracking
    async fn download_file_with_retry(
        &self,
        url: &str,
        output_path: &std::path::PathBuf,
        config: &DownloadConfig,
    ) -> PluginResult<u64> {
        let backoff = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(config.timeout_secs)),
            max_interval: Duration::from_secs(60),
            ..Default::default()
        };

        retry(backoff, || async {
            self.download_file_once(url, output_path, config)
                .await
                .map_err(|e| {
                    warn!("Download attempt failed: {}", e);
                    backoff::Error::transient(e)
                })
        })
        .await
        .map_err(|e| PluginError::Other(format!("Failed to download file after retries: {}", e)))
    }

    /// Single download attempt
    async fn download_file_once(
        &self,
        url: &str,
        output_path: &std::path::PathBuf,
        config: &DownloadConfig,
    ) -> PluginResult<u64> {
        // Send HTTP request
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| PluginError::Other(format!("Failed to initiate download: {}", e)))?;

        if !response.status().is_success() {
            return Err(PluginError::Other(format!("Download failed with HTTP {}", response.status())));
        }

        // Get content length for progress tracking
        let total_size = response.content_length().unwrap_or(0);

        debug!("Download size: {} bytes", total_size);

        // Create output file
        let mut file = tokio::fs::File::create(output_path)
            .await
            .map_err(|e| PluginError::Other(format!("Failed to create output file: {}", e)))?;

        // Download with progress tracking
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| PluginError::Other(format!("Failed to read download chunk: {}", e)))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| PluginError::Other(format!("Failed to write to output file: {}", e)))?;

            downloaded += chunk.len() as u64;

            // Call progress callback if provided
            if let Some(ref callback) = config.progress_callback {
                callback(downloaded, total_size);
            }

            // Log progress every 10%
            if total_size > 0 {
                let progress = (downloaded as f64 / total_size as f64) * 100.0;
                if (progress as u64).is_multiple_of(10) {
                    debug!("Download progress: {:.1}%", progress);
                }
            }
        }

        file.sync_all()
            .await
            .map_err(|e| PluginError::Other(format!("Failed to sync file to disk: {}", e)))?;

        Ok(downloaded)
    }

    /// Calculate MD5 checksum of a file
    fn calculate_checksum(&self, path: &std::path::PathBuf) -> PluginResult<String> {
        let file = fs::File::open(path).map_err(|e| PluginError::Other(format!("Failed to open file {:?}: {}", path, e)))?;

        let mut hasher = Md5::new();
        let mut reader = std::io::BufReader::new(file);
        let mut buffer = [0u8; 8192];

        loop {
            let n = reader
                .read(&mut buffer)
                .map_err(|e| PluginError::Other(format!("Failed to read file for checksum: {}", e)))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }
}

impl Default for HFHubClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Format bytes in human-readable format
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hf_client_creation() {
        let client = HFHubClient::new();
        assert_eq!(client.api_base, "https://huggingface.co");
    }

    #[test]
    fn test_download_url_generation() {
        let client = HFHubClient::new();
        let url = client.get_download_url("hexgrad/Kokoro-82M", "model.onnx");
        assert_eq!(
            url,
            "https://huggingface.co/hexgrad/Kokoro-82M/resolve/main/model.onnx"
        );
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(2048), "2.00 KB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.00 MB");
    }

    #[test]
    fn test_download_config_default() {
        let config = DownloadConfig::default();
        assert_eq!(config.model_id, "hexgrad/Kokoro-82M");
        assert_eq!(config.filename, "kokoro-v0_19.onnx");
        assert_eq!(config.timeout_secs, 600);
        assert_eq!(config.max_retries, 3);
    }
}
