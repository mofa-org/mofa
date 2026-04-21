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
use mofa_kernel::storage::ObjectMetadata;
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

    async fn get_metadata(&self, key: &str) -> AgentResult<Option<ObjectMetadata>> {
        use aws_sdk_s3::operation::head_object::HeadObjectError;

        let result = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await;

        match result {
            Ok(output) => {
                let size = u64::try_from(output.content_length().unwrap_or(0)).unwrap_or(0);
                let content_type = output.content_type().map(|s| s.to_string());
                let last_modified = output.last_modified().and_then(|dt| {
                    chrono::DateTime::from_timestamp(dt.secs(), dt.subsec_nanos())
                        .map(|d: chrono::DateTime<chrono::Utc>| d.to_rfc3339())
                });
                Ok(Some(ObjectMetadata {
                    size,
                    content_type,
                    last_modified,
                }))
            }
            Err(sdk_err) => {
                let service_err = sdk_err.into_service_error();
                if matches!(service_err, HeadObjectError::NotFound(_)) {
                    Ok(None)
                } else {
                    Err(Self::err(format!(
                        "S3 head_object failed for key '{}': {}",
                        key, service_err
                    )))
                }
            }
        }
    }

    async fn presigned_put_url(
        &self,
        key: &str,
        expires_secs: u64,
        content_type: Option<&str>,
    ) -> AgentResult<String> {
        let presigning_cfg = PresigningConfig::expires_in(Duration::from_secs(expires_secs))
            .map_err(|e| Self::err(format!("invalid presigning config: {}", e)))?;

        let mut req = self.client.put_object().bucket(&self.bucket).key(key);

        if let Some(ct) = content_type {
            req = req.content_type(ct);
        }

        let presigned = req
            .presigned(presigning_cfg)
            .await
            .map_err(|e| Self::err(format!("presign PUT failed for key '{}': {}", key, e)))?;

        Ok(presigned.uri().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── S3Config builder tests (no network required) ──────────────────────────

    #[test]
    fn default_config_has_no_endpoint() {
        let cfg = S3Config::new("us-east-1", "my-bucket");
        assert_eq!(cfg.region, "us-east-1");
        assert_eq!(cfg.bucket, "my-bucket");
        assert!(cfg.endpoint_url.is_none());
        assert!(!cfg.force_path_style);
    }

    #[test]
    fn with_endpoint_sets_url_and_enables_path_style() {
        let cfg = S3Config::new("us-east-1", "my-bucket").with_endpoint("http://localhost:9000");
        assert_eq!(cfg.endpoint_url.as_deref(), Some("http://localhost:9000"));
        assert!(cfg.force_path_style, "path style must be enabled for MinIO");
    }

    #[test]
    fn with_path_style_can_be_disabled_explicitly() {
        let cfg = S3Config::new("us-east-1", "bucket")
            .with_endpoint("http://localhost:9000")
            .with_path_style(false);
        assert!(cfg.endpoint_url.is_some());
        assert!(!cfg.force_path_style);
    }

    #[test]
    fn accepts_string_and_str_for_region_and_bucket() {
        let _a = S3Config::new("us-west-2", "bucket-a");
        let _b = S3Config::new(String::from("eu-central-1"), String::from("bucket-b"));
    }

    // ── presigned_put_url default fallback (kernel ObjectStore trait) ─────────

    #[tokio::test]
    async fn default_presigned_put_returns_err() {
        use async_trait::async_trait;
        use mofa_kernel::ObjectStore;
        use mofa_kernel::agent::error::AgentResult;

        struct NoOpStore;

        #[async_trait]
        impl ObjectStore for NoOpStore {
            async fn put(&self, _: &str, _: Vec<u8>) -> AgentResult<()> {
                unimplemented!()
            }
            async fn get(&self, _: &str) -> AgentResult<Option<Vec<u8>>> {
                unimplemented!()
            }
            async fn delete(&self, _: &str) -> AgentResult<bool> {
                unimplemented!()
            }
            async fn list_keys(&self, _: &str) -> AgentResult<Vec<String>> {
                unimplemented!()
            }
            async fn presigned_get_url(&self, _: &str, _: u64) -> AgentResult<String> {
                unimplemented!()
            }
        }

        let store = NoOpStore;
        let result = store.presigned_put_url("key", 3600, None).await;
        assert!(
            result.is_err(),
            "default impl must return Err for unsupported stores"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("presigned_put_url"),
            "error should mention the method name"
        );
    }
}
