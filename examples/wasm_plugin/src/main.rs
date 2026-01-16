//! WASM Plugin Runtime Example
//!
//! This example demonstrates the WebAssembly plugin runtime for MoFA.
//! It shows how to:
//! - Create a WASM runtime
//! - Load and compile WASM modules
//! - Execute WASM plugin functions
//! - Use the plugin manager for lifecycle management
//!
//! Run with: cargo run --example wasm_plugin

use mofa_sdk::wasm_runtime::{PluginCapability, ResourceLimits, RuntimeConfig, WasmPluginConfig, WasmPluginManager, WasmRuntime};
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info,mofa=debug")
        .init();

    info!("╔══════════════════════════════════════════════════════════════╗");
    info!("║           MoFA WASM Plugin Runtime Example                  ║");
    info!("╚══════════════════════════════════════════════════════════════╝");

    // Example 1: Basic WASM Runtime Usage
    info!("📦 Example 1: Basic WASM Runtime");
    info!("─────────────────────────────────────────────");
    basic_runtime_example().await?;

    // Example 2: Math Plugin
    info!("🔢 Example 2: Math Operations Plugin");
    info!("─────────────────────────────────────────────");
    math_plugin_example().await?;

    // Example 3: Plugin Manager
    info!("🔌 Example 3: Plugin Manager");
    info!("─────────────────────────────────────────────");
    plugin_manager_example().await?;
    // Example 4: Resource Limits
    info!("⚙️  Example 4: Resource Limits");
    info!("─────────────────────────────────────────────");
    resource_limits_example().await?;
    info!("╔══════════════════════════════════════════════════════════════╗");
    info!("║                    All examples completed!                   ║");
    info!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Basic runtime usage example
async fn basic_runtime_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create runtime with async support but disable fuel and epoch metering for simplicity
    let mut rt_config = RuntimeConfig::new();
    rt_config.execution_config.fuel_metering = false;
    rt_config.execution_config.epoch_interruption = false;
    let runtime = WasmRuntime::new(rt_config)?;

    // Simple WAT module that exports a constant
    let wat = r#"
        (module
            (func (export "answer") (result i32)
                i32.const 42
            )
            (func (export "double") (param i32) (result i32)
                local.get 0
                i32.const 2
                i32.mul
            )
        )
    "#;

    // Compile the module
    let compiled = runtime.compile_wat("basic-module", wat).await?;
    info!("  ✓ Compiled module: {}", compiled.name);
    info!("    Size: {} bytes", compiled.size_bytes);
    info!("    Compile time: {}ms", compiled.compile_time_ms);

    // Create a plugin from the compiled module
    let mut config = WasmPluginConfig::new("basic-plugin");
    config.resource_limits.max_fuel = None;  // Disable fuel metering for simplicity
    let plugin = runtime.create_plugin(&compiled, config).await?;

    info!("  ✓ Created plugin: {}", plugin.id());
    info!("    State: {:?}", plugin.state().await);

    // Initialize the plugin
    plugin.initialize().await?;
    info!("  ✓ Plugin initialized");
    info!("    State: {:?}", plugin.state().await);

    // Call the 'answer' function
    let result = plugin.call_i32("answer", &[]).await?;
    info!("  ✓ Called 'answer' function");
    info!("    Result: {}", result);

    // Call the 'double' function
    let result = plugin.call_i32("double", &[wasmtime::Val::I32(21)]).await?;
    info!("  ✓ Called 'double' function with 21");
    info!("    Result: {}", result);

    // Check metrics
    let metrics = plugin.metrics().await;
    info!("  📊 Plugin metrics:");
    info!("    Calls: {}", metrics.call_count);
    info!("    Success: {}", metrics.success_count);
    info!("    Avg time: {}ns", metrics.avg_execution_time_ns);

    // Stop the plugin
    plugin.stop().await?;
    info!("  ✓ Plugin stopped");

    Ok(())
}

/// Math operations plugin example
async fn math_plugin_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create runtime with async support but disable fuel and epoch metering
    let mut rt_config = RuntimeConfig::new();
    rt_config.execution_config.fuel_metering = false;
    rt_config.execution_config.epoch_interruption = false;
    let runtime = WasmRuntime::new(rt_config)?;

    // Math plugin with multiple operations
    let wat = r#"
        (module
            ;; Add two integers
            (func (export "add") (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.add
            )

            ;; Subtract
            (func (export "sub") (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.sub
            )

            ;; Multiply
            (func (export "mul") (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.mul
            )

            ;; Factorial (recursive)
            (func $factorial (export "factorial") (param i32) (result i32)
                (if (result i32) (i32.le_s (local.get 0) (i32.const 1))
                    (then (i32.const 1))
                    (else
                        (i32.mul
                            (local.get 0)
                            (call $factorial (i32.sub (local.get 0) (i32.const 1)))
                        )
                    )
                )
            )

            ;; Fibonacci
            (func $fib (export "fibonacci") (param i32) (result i32)
                (if (result i32) (i32.le_s (local.get 0) (i32.const 1))
                    (then (local.get 0))
                    (else
                        (i32.add
                            (call $fib (i32.sub (local.get 0) (i32.const 1)))
                            (call $fib (i32.sub (local.get 0) (i32.const 2)))
                        )
                    )
                )
            )
        )
    "#;

    let mut config = WasmPluginConfig::new("math-plugin");
    config.resource_limits.max_fuel = None;  // Disable fuel metering
    let config = config.with_capability(PluginCapability::ReadConfig);

    let plugin = runtime.create_plugin_from_wat(wat, config).await?;
    plugin.initialize().await?;

    info!("  ✓ Math plugin initialized");

    // Test basic operations
    let tests = vec![
        ("add", vec![10, 20], 30),
        ("sub", vec![50, 20], 30),
        ("mul", vec![6, 7], 42),
        ("factorial", vec![5], 120),
        ("fibonacci", vec![10], 55),
    ];

    for (func, args, expected) in tests {
        let vals: Vec<wasmtime::Val> = args.iter().map(|&a| wasmtime::Val::I32(a)).collect();
        let result = plugin.call_i32(func, &vals).await?;
        let status = if result == expected { "✓" } else { "✗" };
        info!(
            "    {} {}({}) = {} (expected: {})",
            status,
            func,
            args.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", "),
            result,
            expected
        );
    }

    plugin.stop().await?;
    Ok(())
}

/// Plugin manager example
async fn plugin_manager_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create runtime with async support but disable fuel and epoch metering
    let mut rt_config = RuntimeConfig::new();
    rt_config.execution_config.fuel_metering = false;
    rt_config.execution_config.epoch_interruption = false;
    let runtime = Arc::new(WasmRuntime::new(rt_config)?);
    let manager = WasmPluginManager::new(runtime);

    // Subscribe to events
    let mut event_rx = manager.subscribe();

    // Spawn event listener
    let event_handle = tokio::spawn(async move {
        let mut events = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }
        events
    });

    // Load multiple plugins
    let greet_wat = r#"
        (module
            (func (export "greet_len") (result i32)
                i32.const 13  ;; "Hello, World!".len()
            )
        )
    "#;

    let counter_wat = r#"
        (module
            (global $count (mut i32) (i32.const 0))

            (func (export "increment") (result i32)
                global.get $count
                i32.const 1
                i32.add
                global.set $count
                global.get $count
            )

            (func (export "get") (result i32)
                global.get $count
            )

            (func (export "reset")
                i32.const 0
                global.set $count
            )
        )
    "#;

    // Load plugins
    let mut greet_config = WasmPluginConfig::new("greeter");
    greet_config.resource_limits.max_fuel = None;
    let greet_handle = manager.load_wat(greet_wat, Some(greet_config)).await?;
    info!("  ✓ Loaded plugin: {}", greet_handle.id());

    let mut counter_config = WasmPluginConfig::new("counter");
    counter_config.resource_limits.max_fuel = None;
    let counter_handle = manager.load_wat(counter_wat, Some(counter_config)).await?;
    info!("  ✓ Loaded plugin: {}", counter_handle.id());

    // List plugins
    let plugins = manager.list_plugins().await;
    info!("  📋 Loaded plugins: {:?}", plugins.iter().map(|h| h.id()).collect::<Vec<_>>());

    // Initialize plugins
    manager.initialize(&greet_handle).await?;
    manager.initialize(&counter_handle).await?;
    info!("  ✓ All plugins initialized");

    // Use greeter plugin
    let len = manager.call_i32(&greet_handle, "greet_len", &[]).await?;
    info!("  📢 Greeting length: {}", len);

    // Use counter plugin
    for i in 1..=5 {
        let count = manager.call_i32(&counter_handle, "increment", &[]).await?;
        info!("  🔢 Counter increment {}: {}", i, count);
    }

    // Get plugin info
    let info = manager.get_info(&counter_handle).await?;
    info!("  ℹ️  Counter plugin info:");
    info!("      State: {:?}", info.state);
    info!("      Calls: {}", info.metrics.call_count);

    // Get manager stats
    let stats = manager.stats().await;
    info!("  📊 Manager stats:");
    info!("      Active plugins: {}", stats.active_plugins);
    info!("      Total calls: {}", stats.total_calls);
    info!("      Total execution time: {}ms", stats.total_execution_time_ms);

    // Unload all plugins
    manager.unload_all().await?;
    info!("  ✓ All plugins unloaded");

    // Check events
    let events = event_handle.await?;
    info!("  📨 Events received: {}", events.len());

    Ok(())
}

/// Resource limits example
async fn resource_limits_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create runtime with restrictive limits (disable fuel for simpler demo)
    let mut rt_config = RuntimeConfig::new();
    rt_config.execution_config.fuel_metering = false;
    rt_config.execution_config.epoch_interruption = false;
    let rt_config = rt_config.with_resource_limits(ResourceLimits {
        max_memory_pages: 16,      // 1MB max memory
        max_table_elements: 1000,
        max_instances: 5,
        max_execution_time_ms: 1000, // 1 second timeout
        max_fuel: None,              // Disable fuel metering
        max_call_depth: 100,
    });

    let runtime = WasmRuntime::new(rt_config)?;

    info!("  ✓ Runtime created with restrictive limits");
    info!("    Max memory: 1MB (16 pages)");
    info!("    Max execution time: 1000ms");

    // Module that uses memory
    let memory_wat = r#"
        (module
            (memory (export "memory") 1)  ;; 1 page = 64KB

            (func (export "get_memory_size") (result i32)
                memory.size
            )

            (func (export "write_byte") (param i32 i32)
                local.get 0
                local.get 1
                i32.store8
            )

            (func (export "read_byte") (param i32) (result i32)
                local.get 0
                i32.load8_u
            )
        )
    "#;

    let mut config = WasmPluginConfig::new("memory-test");
    config.resource_limits.max_fuel = None;
    config.resource_limits.max_memory_pages = 16;

    let plugin = runtime.create_plugin_from_wat(memory_wat, config).await?;
    plugin.initialize().await?;

    // Check memory size
    let size = plugin.call_i32("get_memory_size", &[]).await?;
    info!("  📏 Initial memory size: {} page(s) ({}KB)", size, size * 64);

    // Write and read data
    plugin.call_void("write_byte", &[wasmtime::Val::I32(0), wasmtime::Val::I32(42)]).await?;
    let value = plugin.call_i32("read_byte", &[wasmtime::Val::I32(0)]).await?;
    info!("  ✓ Written 42 to address 0, read back: {}", value);

    plugin.stop().await?;

    // Display runtime stats
    let stats = runtime.stats().await;
    info!("  📊 Runtime stats:");
    info!("      Modules compiled: {}", stats.modules_compiled);
    info!("      Total compile time: {}ms", stats.total_compile_time_ms);
    if let Some(cache) = &stats.cache_stats {
        info!("      Cache entries: {}", cache.entries);
        info!("      Cache hit rate: {:.1}%", cache.hit_rate() * 100.0);
    }

    Ok(())
}
