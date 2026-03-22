//! Gateway Routing Demo - Demonstrates JWT Authentication and Advanced Routing
//!
//! This demo showcases:
//! 1. JWT Authentication with the gateway
//! 2. Failover routing between providers
//! 3. Cost-optimized routing
//!
//! Run with: cargo run -p mofa-gateway --example gateway_routing_demo
//!
//! The demo starts a server and provides curl commands to test:
//! - JWT authentication
//! - Failover routing
//! - Cost-based routing

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use mofa_gateway::gateway::routing_policy::{
    RoutingPolicy, StaticPricingRegistry, ProviderCost,
};
use mofa_gateway::inference_bridge::InferenceBridge;
use mofa_gateway::middleware::jwt_auth::JwtAuth;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// JWT Claims structure for demo
#[derive(Debug, Serialize, Deserialize)]
struct DemoClaims {
    sub: String,
    exp: usize,
    iat: usize,
}

/// Generate a valid JWT token for testing
fn generate_jwt(secret: &str, user_id: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;
    
    let claims = DemoClaims {
        sub: user_id.to_string(),
        exp: now + 3600, // 1 hour expiration
        iat: now,
    };
    
    let header = Header::new(Algorithm::HS256);
    encode(&header, &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .expect("Failed to encode JWT")
}

/// Generate an expired JWT token
fn generate_expired_jwt(secret: &str, user_id: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;
    
    let claims = DemoClaims {
        sub: user_id.to_string(),
        exp: now - 3600, // Expired 1 hour ago
        iat: now - 7200,
    };
    
    let header = Header::new(Algorithm::HS256);
    encode(&header, &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .expect("Failed to encode JWT")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    println!("{}", "=".repeat(60));
    println!("MoFA Gateway - Routing & JWT Demo");
    println!("{}", "=".repeat(60));
    
    // Demo 1: JWT Authentication
    println!("\n📋 Demo 1: JWT Authentication");
    println!("{}", "-".repeat(40));
    demo_jwt_auth().await?;
    
    // Demo 2: Routing Policies
    println!("\n📋 Demo 2: Routing Policies");
    println!("{}", "-".repeat(40));
    demo_routing_policies().await?;
    
    // Demo 3: Cost-Optimized Routing
    println!("\n📋 Demo 3: Cost-Optimized Routing");
    println!("{}", "-".repeat(40));
    demo_cost_routing().await?;
    
    println!("\n{}", "=".repeat(60));
    println!("All demos completed!");
    println!("{}", "=".repeat(60));
    
    Ok(())
}

async fn demo_jwt_auth() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔐 JWT Authentication Demo");
    println!();
    
    let secret = "my-secret-key";
    
    // Generate valid token
    let valid_token = generate_jwt(secret, "user123");
    println!("✓ Generated valid JWT token: {}...", &valid_token[..20]);
    
    // Generate expired token
    let expired_token = generate_expired_jwt(secret, "user456");
    println!("✓ Generated expired JWT token: {}...", &expired_token[..20]);
    
    // Test with JWT validator
    let jwt_auth = JwtAuth::new(secret);
    
    // Valid token should work
    match jwt_auth.validate_token(&valid_token) {
        Ok(claims) => println!("✓ Valid token accepted for user: {}", claims.sub),
        Err(e) => println!("✗ Valid token rejected: {}", e),
    }
    
    // Expired token should fail
    match jwt_auth.validate_token(&expired_token) {
        Ok(_) => println!("✗ Expired token was incorrectly accepted!"),
        Err(e) => println!("✓ Expired token correctly rejected: {}", e),
    }
    
    // Wrong secret should fail
    let wrong_jwt = JwtAuth::new("wrong-secret");
    match wrong_jwt.validate_token(&valid_token) {
        Ok(_) => println!("✗ Wrong secret token was incorrectly accepted!"),
        Err(e) => println!("✓ Wrong secret token correctly rejected"),
    }
    
    println!("\n💡 To test with the gateway, use:");
    println!("   curl -X POST http://localhost:8080/v1/chat/completions \\");
    println!("     -H \"Authorization: Bearer <token>\" \\");
    println!("     -H \"Content-Type: application/json\" \\");
    println!("     -d '{{\"model\":\"test\",\"messages\":[{{\"role\":\"user\",\"content\":\"hi\"}}]}}'");
    
    Ok(())
}

async fn demo_routing_policies() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔀 Routing Policies Demo");
    println!();
    
    // Create inference bridge with failover policy
    let bridge = InferenceBridge::new("primary")
        .with_providers(vec!["primary".to_string(), "fallback".to_string()])
        .with_routing_policy(RoutingPolicy::Failover {
            primary: "primary".to_string(),
            fallback: "fallback".to_string(),
        });
    
    println!("✓ Created InferenceBridge with failover policy");
    println!("  Primary: {}", bridge.default_provider());
    println!("  Policy: {}", bridge.policy());
    
    // Resolve provider
    let provider = bridge.resolve_provider();
    println!("✓ Resolved provider: {}", provider);
    
    // Test failover with error simulation
    println!("\n✓ Simulating failover behavior:");
    println!("  - Try primary provider");
    println!("  - If error, automatically switch to fallback");
    
    // The actual failover happens in the call_with_failover method
    // In a real scenario, you'd call the actual inference provider
    
    Ok(())
}

async fn demo_cost_routing() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n💰 Cost-Optimized Routing Demo");
    println!();
    
    // Create pricing registry with multiple providers
    let mut pricing = StaticPricingRegistry::new();
    pricing.register_provider(ProviderCost::new("openai", 2.50, 10.00));   // $2.50/1K in, $10/1K out
    pricing.register_provider(ProviderCost::new("anthropic", 3.00, 15.00)); // $3/1K in, $15/1K out
    pricing.register_provider(ProviderCost::new("gemini", 1.25, 5.00));     // $1.25/1K in, $5/1K out
    pricing.register_provider(ProviderCost::new("local", 0.0, 0.0));        // Free!
    
    println!("✓ Created pricing registry with providers:");
    for (name, cost) in [("openai", 2.50), ("anthropic", 3.00), ("gemini", 1.25), ("local", 0.0)] {
        println!("  - {}: ${}/1K input tokens", name, cost);
    }
    
    // Create bridge with cost-optimized policy
    let bridge = InferenceBridge::new("openai")
        .with_providers(vec![
            "openai".to_string(),
            "anthropic".to_string(),
            "gemini".to_string(),
            "local".to_string(),
        ])
        .with_routing_policy(RoutingPolicy::CostOptimized)
        .with_pricing(pricing);
    
    // Get cheapest provider
    let cheapest = bridge.get_cheapest_provider();
    println!("\n✓ Cost-optimized routing selects: {}", cheapest.unwrap());
    
    println!("\n💡 Cost routing always prefers the cheapest provider.");
    println!("   In this case, 'local' (free) is always selected first.");
    
    Ok(())
}
