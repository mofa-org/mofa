# Gateway Proxy Examples

Practical examples demonstrating how to use the MoFA Gateway HTTP proxy for mofa-local-llm.

## Examples

### Rust Examples

Rust examples are located in `crates/mofa-gateway/examples/`:

#### `proxy_with_custom_config.rs`
Demonstrates custom proxy configuration with environment variables.

```bash
export MOFA_LOCAL_LLM_URL="http://localhost:9000"
cd crates/mofa-gateway
cargo run --example proxy_with_custom_config
```

#### `proxy_client_examples.rs`
Comprehensive Rust client examples with 7 different use cases:
- List models
- Get model information
- Simple chat completion
- Chat with system message
- Multi-turn conversation
- Error handling
- Health checks

```bash
cd crates/mofa-gateway
cargo run --example proxy_client_examples
```

### Python Example

#### `proxy_python_client.py`
Python client using OpenAI SDK - 6 practical examples.

**Prerequisites:**
```bash
pip install openai
```

**Usage:**
```bash
python examples/proxy/proxy_python_client.py
```

### JavaScript Example

#### `proxy_javascript_client.js`
JavaScript/Node.js client using OpenAI SDK - 6 practical examples.

**Prerequisites:**
```bash
npm install openai
```

**Usage:**
```bash
node examples/proxy/proxy_javascript_client.js
```

## Prerequisites

1. **Start mofa-local-llm server:**
   ```bash
   cd mofa-local-llm
   cargo run --release
   ```

2. **Start gateway:**
   ```bash
   cd mofa/crates/mofa-gateway
   cargo run --example gateway_local_llm_proxy
   ```

## Documentation

For complete documentation, see:
- [PROXY.md](../../crates/mofa-gateway/PROXY.md) - Full proxy documentation
- [CRATE_OVERVIEW.md](../../crates/mofa-gateway/CRATE_OVERVIEW.md) - Gateway overview

## Quick Start

```bash
# 1. Start backend (in another terminal)
cd mofa-local-llm && cargo run --release

# 2. Start gateway (in another terminal)
cd mofa/crates/mofa-gateway
cargo run --example gateway_local_llm_proxy

# 3. Run examples (from workspace root)
cd mofa

# Rust example
cargo run -p mofa-gateway --example proxy_client_examples

# Python example
python examples/proxy/proxy_python_client.py

# JavaScript example
node examples/proxy/proxy_javascript_client.js
```

## Features Demonstrated

- OpenAI-compatible API endpoints
- Multiple client libraries (Rust, Python, JavaScript)
- Error handling and retries
- Health checking
- Custom configuration
- Environment variable usage
- Multi-turn conversations
- System messages
- Usage statistics

## See Also

- [Gateway Examples](../../crates/mofa-gateway/examples/) - More gateway examples
- [Task 13 Plan](../../TASK13_PLAN.md) - Implementation details
- [Next Tasks](../../NEXT_TASKS.md) - Future enhancements
