//! Cryptographic signature verification for skills
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use mofa_kernel::plugin::{PluginError, PluginResult};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info};
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Trust store for managing allowed public keys
#[derive(Debug, Clone, Default)]
pub struct TrustStore {
    trusted_keys: Arc<RwLock<HashSet<String>>>,
}

impl TrustStore {
    /// Create a new trust store
    pub fn new() -> Self {
        Self {
            trusted_keys: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Add a trusted public key (base64)
    pub fn add_key(&self, public_key_base64: &str) {
        if let Ok(mut keys) = self.trusted_keys.write() {
            keys.insert(public_key_base64.to_string());
        }
    }

    /// Check if a key is trusted
    pub fn is_trusted(&self, public_key_base64: &str) -> bool {
        if let Ok(keys) = self.trusted_keys.read() {
            keys.contains(public_key_base64)
        } else {
            false
        }
    }

    /// Verify the signature of a skill's content
    pub fn verify(&self, content: &str, signature_b64: &str, signer_key_b64: &str) -> PluginResult<()> {
        if let Ok(keys) = self.trusted_keys.read() {
            if !keys.is_empty() && !keys.contains(signer_key_b64) {
                return Err(PluginError::ExecutionFailed(format!("Signer key not in trust store: {}", signer_key_b64)));
            }
        }

        // Decode public key
        let pk_bytes = STANDARD.decode(signer_key_b64)
            .map_err(|e| PluginError::ExecutionFailed(format!("Invalid signer_key base64: {}", e)))?;
        
        let verifying_key = VerifyingKey::try_from(pk_bytes.as_slice())
            .map_err(|e| PluginError::ExecutionFailed(format!("Invalid Ed25519 public key: {}", e)))?;

        // Decode signature
        let sig_bytes = STANDARD.decode(signature_b64)
            .map_err(|e| PluginError::ExecutionFailed(format!("Invalid signature base64: {}", e)))?;
            
        let signature = Signature::from_slice(&sig_bytes)
            .map_err(|e| PluginError::ExecutionFailed(format!("Invalid Ed25519 signature: {}", e)))?;

        // Verify content signature
        verifying_key.verify(content.as_bytes(), &signature)
            .map_err(|_| PluginError::ExecutionFailed("Signature verification failed. The content may have been tampered with.".to_string()))?;

        debug!("Successfully verified skill signature");
        Ok(())
    }
}
