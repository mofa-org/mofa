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

#[cfg(not(feature = "openai-compat"))]
fn main() {
    println!("This example requires the 'openai-compat' feature.");
    println!("Run with: cargo run -p mofa-gateway --example gateway_routing_demo --features openai-compat");
}

#[cfg(feature = "openai-compat")]
fn main() {
    // Initialize tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();
    
    println!("{}", "=".repeat(60));
    println!("MoFA Gateway - Routing & JWT Demo");
    println!("{}", "=".repeat(60));
    
    // Demo 1: JWT Authentication
    println!("\n📋 Demo 1: JWT Authentication");
    println!("{}", "-".repeat(40));
    demo_jwt_auth();
    
    // Demo 2: Routing Policies
    println!("\n📋 Demo 2: Routing Policies");
    println!("{}", "-".repeat(40));
    demo_routing_policies();
    
    // Demo 3: Cost-Optimized Routing
    println!("\n📋 Demo 3: Cost-Optimized Routing");
    println!("{}", "-".repeat(40));
    demo_cost_routing();
    
    println!("\n{}", "=".repeat(60));
    println!("All demos completed!");
    println!("{}", "=".repeat(60));
}

#[cfg(feature = "openai-compat")]
fn demo_jwt_auth() {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    use mofa_gateway::middleware::jwt_auth::JwtAuth;
    use serde::{Deserialize, Serialize};
    use std::time::{SystemTime, UNIX_EPOCH};

    println!("\n🔐 JWT Authentication Demo");
    println!();
    
    let secret = "my-secret-key";
    
    #[derive(Debug, Serialize, Deserialize)]
    struct DemoClaims {
        sub: String,
        exp: usize,
        iat: usize,
    }

    // Generate valid token
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;
    
    let valid_claims = DemoClaims {
        sub: "user123".to_string(),
        exp: now + 3600,
        iat: now,
    };
    let valid_token = encode(
        &Header::new(Algorithm::HS256),
        &valid_claims,
        &EncodingKey::from_secret(secret.as_bytes())
    ).expect("Failed to encode JWT");
    
    println!("✓ Generated valid JWT token: {}...", &valid_token[..20]);
    
    // Generate expired token
    let expired_claims = DemoClaims {
        sub: "user456".to_string(),
        exp: now - 3600,
        iat: now - 7200,
    };
    let expired_token = encode(
        &Header::new(Algorithm::HS256),
        &expired_claims,
        &EncodingKey::from_secret(secret.as_bytes())
    ).expect("Failed to encode JWT");
    
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
    println!("     -d '{\"model\":\"test\",\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}}'");
}

#[cfg(feature = "openai-compat")]
fn demo_routing_policies() {
    use mofa_gateway::gateway::routing_policy::RoutingPolicy;
    use mofa_gateway::inference_bridge::InferenceBridge;

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
}

#[cfg(feature = "openai-compat")]
fn demo_cost_routing() {
    use mofa_gateway::gateway::routing_policy::{
        RoutingPolicy, StaticPricingRegistry, ProviderCost,
    };
    use mofa_gateway::inference_bridge::InferenceBridge;

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
}
