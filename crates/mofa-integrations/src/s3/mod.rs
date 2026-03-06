//! AWS S3 / MinIO object-storage adapter
//!
//! Implements the kernel `ObjectStore` trait backed by the official
//! `aws-sdk-s3` crate. Setting `endpoint_url` makes the adapter talk to any
//! S3-compatible service such as MinIO, Ceph, or LocalStack.
//!
//! # Authentication
//!
//! Credentials are resolved in the standard AWS order:
//! environment variables → shared credentials file → IAM instance profile.
//! For MinIO or LocalStack set `AWS_ACCESS_KEY_ID` and
//! `AWS_SECRET_ACCESS_KEY` in the environment.

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::Builder as S3Builder;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use mofa_kernel::ObjectStore;
use mofa_kernel::agent::error::{AgentError, AgentResult};
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for `S3ObjectStore`
#[derive(Debug, Clone)]
pub struct S3Config {
    /// AWS region (e.g. `"us-east-1"`)
    pub region: String,
    /// Target bucket name
    pub bucket: String,
    /// Optional custom endpoint URL.
    ///
    /// Set this to point at a local MinIO instance:
    /// ```text
    /// S3Config::new("us-east-1", "my-bucket")
    ///     .with_endpoint("http://localhost:9000")
    /// ```
    pub endpoint_url: Option<String>,
    /// Force path-style addressing (required by MinIO).
    ///
    /// Enabled automatically when `endpoint_url` is set.
    pub force_path_style: bool,
}

impl S3Config {
    /// Create a minimal config with region and bucket.
    pub fn new(region: impl Into<String>, bucket: impl Into<String>) -> Self {
        Self {
            region: region.into(),
            bucket: bucket.into(),
            endpoint_url: None,
            force_path_style: false,
        }
    }

    /// Override the S3 endpoint URL (enables MinIO / LocalStack support).
    ///
    /// Automatically enables `force_path_style`.
    pub fn with_endpoint(mut self, url: impl Into<String>) -> Self {
        self.endpoint_url = Some(url.into());
        self.force_path_style = true;
        self
    }

    /// Explicitly control path-style addressing.
    pub fn with_path_style(mut self, enabled: bool) -> Self {
        self.force_path_style = enabled;
        self
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S3ObjectStore
// ─────────────────────────────────────────────────────────────────────────────

/// Object-storage adapter backed by AWS S3 (or any S3-compatible service).
///
/// Implements [`mofa_kernel::ObjectStore`] so it can be passed wherever the
/// framework expects cloud storage.
pub struct S3ObjectStore {
    client: Client,
    bucket: String,
}

impl S3ObjectStore {
    /// Build the store from an [`S3Config`], loading AWS credentials from the
    /// environment using the standard AWS SDK credential chain.
    pub async fn new(config: S3Config) -> AgentResult<Self> {
        let region = aws_sdk_s3::config::Region::new(config.region.clone());

        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region)
            .load()
            .await;

        let mut s3_builder = S3Builder::from(&sdk_config);

        if let Some(endpoint) = &config.endpoint_url {
            s3_builder = s3_builder.endpoint_url(endpoint);
        }

        if config.force_path_style {
            s3_builder = s3_builder.force_path_style(true);
        }

        let client = Client::from_conf(s3_builder.build());

        Ok(Self {
            client,
            bucket: config.bucket,
        })
    }

    fn err(msg: impl Into<String>) -> AgentError {
        AgentError::ExecutionFailed(msg.into())
    }
}

#[async_trait]
impl ObjectStore for S3ObjectStore {
    async fn put(&self, key: &str, data: Vec<u8>) -> AgentResult<()> {
        let body = ByteStream::from(data);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body)
            .send()
            .await
            .map_err(|e| Self::err(format!("S3 put failed for key '{}': {}", key, e)))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> AgentResult<Option<Vec<u8>>> {
        let result = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await;

        match result {
            Ok(output) => {
                let bytes = output
                    .body
                    .collect()
                    .await
                    .map_err(|e| {
                        Self::err(format!("S3 body read failed for key '{}': {}", key, e))
                    })?
                    .into_bytes();
                Ok(Some(bytes.to_vec()))
            }
            Err(sdk_err) => {
                // Treat a 404 (NoSuchKey) as a missing value rather than an error
                let service_err = sdk_err.into_service_error();
                if service_err.is_no_such_key() {
                    Ok(None)
                } else {
                    Err(Self::err(format!(
                        "S3 get failed for key '{}': {}",
                        key, service_err
                    )))
                }
            }
        }
    }

    async fn delete(&self, key: &str) -> AgentResult<bool> {
        // HEAD to check existence before deleting (S3 delete is idempotent)
        let exists = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .is_ok();

        if exists {
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await
                .map_err(|e| Self::err(format!("S3 delete failed for key '{}': {}", key, e)))?;
        }

        Ok(exists)
    }

    async fn list_keys(&self, prefix: &str) -> AgentResult<Vec<String>> {
        let mut keys = Vec::new();
        let mut continuation = None;

        loop {
            let mut req = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix);

            if let Some(token) = continuation.take() {
                req = req.continuation_token(token);
            }

            let output = req
                .send()
                .await
                .map_err(|e| Self::err(format!("S3 list failed: {}", e)))?;

            for obj in output.contents() {
                if let Some(k) = obj.key() {
                    keys.push(k.to_string());
                }
            }

            match output.next_continuation_token() {
                Some(token) => continuation = Some(token.to_string()),
                None => break,
            }
        }

        Ok(keys)
    }

    async fn presigned_get_url(&self, key: &str, expires_secs: u64) -> AgentResult<String> {
        let presigning_cfg = PresigningConfig::expires_in(Duration::from_secs(expires_secs))
            .map_err(|e| Self::err(format!("invalid presigning config: {}", e)))?;

        let presigned = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(presigning_cfg)
            .await
            .map_err(|e| Self::err(format!("presign failed for key '{}': {}", key, e)))?;

        Ok(presigned.uri().to_string())
    }
}
