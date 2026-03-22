//! JWT Authentication middleware for the MoFA Gateway.
//!
//! Provides simple JWT token validation using HS256 algorithm.
//! Extracts and validates Bearer tokens from the Authorization header.

use serde::{Deserialize, Serialize};
use axum::{
    body::Body,
    extract::Request,
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};

/// JWT claims structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (usually user ID).
    pub sub: String,
    /// Expiration time (Unix timestamp).
    pub exp: usize,
    /// Issued at time (optional).
    #[serde(default)]
    pub iat: Option<usize>,
    /// Issuer (optional).
    #[serde(default)]
    pub iss: Option<String>,
    /// Any additional claims.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// JWT authentication configuration.
#[derive(Debug, Clone)]
pub struct JwtAuth {
    /// Secret key for JWT validation (HMAC).
    secret: String,
    /// Expected issuer (optional).
    issuer: Option<String>,
    /// Validation options.
    validation: Validation,
}

impl JwtAuth {
    /// Create a new JWT authenticator with the given secret.
    pub fn new(secret: impl Into<String>) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        
        Self {
            secret: secret.into(),
            issuer: None,
            validation,
        }
    }

    /// Create a new JWT authenticator with secret and expected issuer.
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        let issuer_str = issuer.into();
        self.issuer = Some(issuer_str.clone());
        self.validation.set_issuer(&[issuer_str]);
        self
    }

    /// Validate a JWT token and return the claims.
    pub fn validate_token(&self, token: &str) -> Result<Claims, JwtError> {
        let header = decode_header(token)
            .map_err(|e| JwtError::InvalidToken(format!("Failed to decode header: {}", e)))?;

        if header.alg != Algorithm::HS256 {
            return Err(JwtError::InvalidToken(format!(
                "Unsupported algorithm: {:?}",
                header.alg
            )));
        }

        let decoding_key = DecodingKey::from_secret(self.secret.as_bytes());
        
        let token_data = decode::<Claims>(token, &decoding_key, &self.validation)
            .map_err(|e| JwtError::InvalidToken(format!("Token validation failed: {}", e)))?;

        Ok(token_data.claims)
    }

    /// Extract and validate JWT token from Authorization header.
    pub fn extract_from_header(
        &self,
        authorization: Option<&axum::http::HeaderValue>,
    ) -> Result<Claims, JwtError> {
        let auth_header = authorization
            .and_then(|h| h.to_str().ok())
            .ok_or(JwtError::MissingToken)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(JwtError::InvalidToken("Missing Bearer prefix".to_string()))?
            .trim();

        if token.is_empty() {
            return Err(JwtError::InvalidToken("Empty token".to_string()));
        }

        self.validate_token(token)
    }
}

/// JWT authentication errors.
#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    /// Token is missing from the request.
    #[error("Missing authorization token")]
    MissingToken,
    
    /// Token is invalid or expired.
    #[error("Invalid token: {0}")]
    InvalidToken(String),
    
    /// Token validation failed.
    #[error("Token validation failed: {0}")]
    ValidationError(String),
}

/// Response body for authentication errors.
#[derive(serde::Serialize)]
struct AuthErrorBody {
    error: String,
    error_type: String,
}

impl IntoResponse for JwtError {
    fn into_response(self) -> axum::response::Response<Body> {
        let (status, error, error_type): (StatusCode, String, String) = match self {
            JwtError::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "Missing authorization token".to_string(),
                "authentication_error".to_string(),
            ),
            JwtError::InvalidToken(msg) => (
                StatusCode::UNAUTHORIZED,
                msg,
                "invalid_token".to_string(),
            ),
            JwtError::ValidationError(msg) => (
                StatusCode::UNAUTHORIZED,
                msg,
                "token_validation_error".to_string(),
            ),
        };

        let body = AuthErrorBody {
            error,
            error_type,
        };

        (status, Json(body)).into_response()
    }
}

/// Middleware function that validates JWT tokens.
/// 
/// Extracts the Authorization header, validates the JWT,
/// and adds the claims to the request extensions.
pub async fn jwt_auth_middleware(
    request: Request<Body>,
    next: Next,
    jwt_auth: JwtAuth,
) -> axum::response::Response<Body> {
    let auth_header = request.headers().get(AUTHORIZATION);
    
    match jwt_auth.extract_from_header(auth_header) {
        Ok(claims) => {
            // Add claims to request extensions for later use
            let mut req = request;
            req.extensions_mut().insert(claims);
            next.run(req).await
        }
        Err(e) => e.into_response(),
    }
}

/// Helper to generate a simple JWT token for testing.
/// 
/// Note: This is for testing only. In production, use a proper JWT library.
#[cfg(test)]
pub fn generate_test_token(secret: &str, sub: &str, exp: usize) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    
    let claims = Claims {
        sub: sub.to_string(),
        exp,
        iat: Some(chrono::Utc::now().timestamp() as usize),
        iss: None,
        extra: std::collections::HashMap::new(),
    };
    
    let header = Header::new(Algorithm::HS256);
    encode(&header, &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .expect("Failed to encode test token")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_auth_valid_token() {
        let secret = "test-secret-key";
        let jwt = JwtAuth::new(secret);
        
        // Generate a valid token
        let token = generate_test_token(secret, "user123", 
            (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize);
        
        let claims = jwt.validate_token(&token).unwrap();
        assert_eq!(claims.sub, "user123");
    }

    #[test]
    fn test_jwt_auth_invalid_secret() {
        let jwt = JwtAuth::new("correct-secret");
        let token = generate_test_token("wrong-secret", "user123",
            (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize);
        
        let result = jwt.validate_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_jwt_auth_expired_token() {
        let secret = "test-secret";
        let jwt = JwtAuth::new(secret);
        
        // Generate an expired token (expired 1 hour ago)
        let token = generate_test_token(secret, "user123",
            (chrono::Utc::now() - chrono::Duration::hours(1)).timestamp() as usize);
        
        let result = jwt.validate_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_from_header_missing() {
        let jwt = JwtAuth::new("secret");
        let result = jwt.extract_from_header(None);
        assert!(matches!(result, Err(JwtError::MissingToken)));
    }

    #[test]
    fn test_extract_from_header_invalid_format() {
        let jwt = JwtAuth::new("secret");
        
        // Test without Bearer prefix
        let header = Some(axum::http::HeaderValue::from_static("Basic abc123"));
        let result = jwt.extract_from_header(header.as_ref());
        assert!(matches!(result, Err(JwtError::InvalidToken(_))));
    }

    #[test]
    fn test_extract_from_header_success() {
        let secret = "test-secret";
        let jwt = JwtAuth::new(secret);
        
        let token = generate_test_token(secret, "user123",
            (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize);
        
        let header = Some(
            axum::http::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap()
        );
        
        let result = jwt.extract_from_header(header.as_ref());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().sub, "user123");
    }
}
