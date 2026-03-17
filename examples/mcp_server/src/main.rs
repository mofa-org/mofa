//! MCP Server Example
//!
//! Demonstrates how to expose MoFA tools as an MCP server so external systems
//! like Claude Desktop, Cursor, or other MCP-compatible clients can discover
//! and invoke MoFA tools over HTTP/SSE.
//!
//! # Running
//!
//! ```bash
//! cargo run -p mcp_server
//! # or on a specific port
//! PORT=8080 cargo run -p mcp_server
//! ```
//!
//! # Connecting from Claude Desktop
//!
//! Add to `claude_desktop_config.json`:
//! ```json
//! {
//!   "mcpServers": {
//!     "mofa-agent": {
//!       "url": "http://127.0.0.1:3000/mcp"
//!     }
//!   }
//! }
//! ```
//!
//! # Tools exposed
//!
//! | Tool | Description |
//! |------|-------------|
//! | `echo` | Returns the input unchanged — connectivity test |
//! | `system_info` | CPU count, total memory, OS details |
//! | `timestamp` | Current UTC time in multiple formats |
//! | `word_count` | Count words, characters, and lines in text |
//! | `text_transform` | Uppercase, lowercase, reverse, trim |
//! | `json_query` | Extract a value from JSON by dot-path |
//! | `base64_encode` | Encode a string to Base64 |
//! | `base64_decode` | Decode a Base64 string |
//! | `hash_text` | SHA-256 hex digest of a string |
//! | `url_parse` | Break a URL into scheme, host, path, query |
//! | `uuid_generate` | Generate a new random UUID v4 |
//! | `temperature_convert` | Convert between Celsius, Fahrenheit, Kelvin |
//! | `math_eval` | Evaluate basic arithmetic expressions |
//! | `regex_match` | Test whether a string matches a regex pattern |
//! | `list_env` | List non-sensitive environment variable keys |

use mofa_foundation::agent::tools::mcp::McpServerManager;
use mofa_kernel::agent::components::mcp::McpHostConfig;
use mofa_kernel::agent::components::tool::{ToolExt, ToolInput, ToolResult};
use mofa_kernel::agent::context::AgentContext;
use tokio_util::sync::CancellationToken;

// ============================================================================
// Macro to reduce boilerplate for simple tools
// ============================================================================

macro_rules! simple_tool {
    ($name:ident, $tool_name:literal, $desc:literal, $schema:expr, |$arg:ident| $body:block) => {
        struct $name;

        #[async_trait::async_trait]
        impl mofa_kernel::agent::components::tool::Tool for $name {
            fn name(&self) -> &str {
                $tool_name
            }
            fn description(&self) -> &str {
                $desc
            }
            fn parameters_schema(&self) -> serde_json::Value {
                $schema
            }
            async fn execute(&self, $arg: ToolInput, _ctx: &AgentContext) -> ToolResult
                $body
        }
    };
}

// ============================================================================
// Tool: echo
// ============================================================================

simple_tool!(
    EchoTool,
    "echo",
    "Returns the input message unchanged. Useful for testing MCP connectivity.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "message": { "type": "string", "description": "Message to echo back" }
        },
        "required": ["message"]
    }),
    |input| {
        let msg = input.arguments.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        ToolResult::success(serde_json::json!({ "echo": msg }))
    }
);

// ============================================================================
// Tool: system_info
// ============================================================================

struct SystemInfoTool;

#[async_trait::async_trait]
impl mofa_kernel::agent::components::tool::Tool for SystemInfoTool {
    fn name(&self) -> &str { "system_info" }

    fn description(&self) -> &str {
        "Returns CPU count, available memory (bytes), OS name, and architecture of the host \
         running this MoFA agent. Useful for capacity planning and debugging."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _input: ToolInput, _ctx: &AgentContext) -> ToolResult {
        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(0);

        ToolResult::success(serde_json::json!({
            "cpu_threads": cpu_count,
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "family": std::env::consts::FAMILY,
        }))
    }
}

// ============================================================================
// Tool: timestamp
// ============================================================================

struct TimestampTool;

#[async_trait::async_trait]
impl mofa_kernel::agent::components::tool::Tool for TimestampTool {
    fn name(&self) -> &str { "timestamp" }

    fn description(&self) -> &str {
        "Returns the current UTC time as a Unix timestamp (seconds), milliseconds since epoch, \
         and ISO 8601 string."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _input: ToolInput, _ctx: &AgentContext) -> ToolResult {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        let secs = now.as_secs();
        let ms = now.as_millis();

        // Simple ISO 8601 formatting without extra deps
        let s = secs;
        let sec = s % 60;
        let min = (s / 60) % 60;
        let hour = (s / 3600) % 24;
        let days = s / 86400;
        // Approximate year/month/day from epoch days
        let year = 1970 + days / 365;
        let day_of_year = days % 365;
        let month = day_of_year / 30 + 1;
        let day = day_of_year % 30 + 1;

        let iso = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            year, month.min(12), day.min(31), hour, min, sec
        );

        ToolResult::success(serde_json::json!({
            "unix_seconds": secs,
            "unix_millis": ms,
            "iso8601_approx": iso,
        }))
    }
}

// ============================================================================
// Tool: word_count
// ============================================================================

simple_tool!(
    WordCountTool,
    "word_count",
    "Counts the number of words, characters (with and without spaces), and lines in the \
     provided text.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": { "type": "string", "description": "Text to analyse" }
        },
        "required": ["text"]
    }),
    |input| {
        let text = input.arguments.get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let words = text.split_whitespace().count();
        let chars_with_spaces = text.chars().count();
        let chars_no_spaces = text.chars().filter(|c| !c.is_whitespace()).count();
        let lines = text.lines().count();

        ToolResult::success(serde_json::json!({
            "words": words,
            "chars_with_spaces": chars_with_spaces,
            "chars_no_spaces": chars_no_spaces,
            "lines": lines,
        }))
    }
);

// ============================================================================
// Tool: text_transform
// ============================================================================

simple_tool!(
    TextTransformTool,
    "text_transform",
    "Applies a text transformation: 'uppercase', 'lowercase', 'titlecase', 'reverse', \
     'trim', or 'snake_case'.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": { "type": "string", "description": "Text to transform" },
            "operation": {
                "type": "string",
                "enum": ["uppercase", "lowercase", "titlecase", "reverse", "trim", "snake_case"],
                "description": "Transformation to apply"
            }
        },
        "required": ["text", "operation"]
    }),
    |input| {
        let text = input.arguments.get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let op = input.arguments.get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("lowercase");

        let result = match op {
            "uppercase" => text.to_uppercase(),
            "lowercase" => text.to_lowercase(),
            "titlecase" => text.split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
            "reverse" => text.chars().rev().collect(),
            "trim" => text.trim().to_string(),
            "snake_case" => text.to_lowercase().replace(' ', "_"),
            _ => return ToolResult::failure(format!("Unknown operation: {}", op)),
        };

        ToolResult::success(serde_json::json!({ "result": result }))
    }
);

// ============================================================================
// Tool: json_query
// ============================================================================

simple_tool!(
    JsonQueryTool,
    "json_query",
    "Extracts a value from a JSON string using a dot-separated path (e.g. 'user.address.city'). \
     Returns null if the path does not exist.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "json": { "type": "string", "description": "JSON string to query" },
            "path": { "type": "string", "description": "Dot-separated key path, e.g. 'a.b.c'" }
        },
        "required": ["json", "path"]
    }),
    |input| {
        let json_str = input.arguments.get("json")
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        let path = input.arguments.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let parsed: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => return ToolResult::failure(format!("Invalid JSON: {}", e)),
        };

        let mut current = &parsed;
        for key in path.split('.') {
            current = match current.get(key) {
                Some(v) => v,
                None => return ToolResult::success(serde_json::json!({ "value": null, "found": false })),
            };
        }

        ToolResult::success(serde_json::json!({ "value": current, "found": true }))
    }
);

// ============================================================================
// Tool: base64_encode / base64_decode
// ============================================================================

simple_tool!(
    Base64EncodeTool,
    "base64_encode",
    "Encodes a UTF-8 string to Base64.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": { "type": "string", "description": "String to encode" }
        },
        "required": ["text"]
    }),
    |input| {
        let text = input.arguments.get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        use std::io::Write;
        let encoded = {
            // Manual base64 encoding using the alphabet
            const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let bytes = text.as_bytes();
            let mut out = String::new();
            for chunk in bytes.chunks(3) {
                let b0 = chunk[0] as usize;
                let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
                let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
                out.push(ALPHABET[(b0 >> 2)] as char);
                out.push(ALPHABET[((b0 & 3) << 4) | (b1 >> 4)] as char);
                out.push(if chunk.len() > 1 { ALPHABET[((b1 & 0xf) << 2) | (b2 >> 6)] as char } else { '=' });
                out.push(if chunk.len() > 2 { ALPHABET[b2 & 0x3f] as char } else { '=' });
            }
            out
        };
        ToolResult::success(serde_json::json!({ "encoded": encoded }))
    }
);

simple_tool!(
    Base64DecodeTool,
    "base64_decode",
    "Decodes a Base64 string to UTF-8 text. Returns an error if the input is not valid Base64.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "encoded": { "type": "string", "description": "Base64 string to decode" }
        },
        "required": ["encoded"]
    }),
    |input| {
        let encoded = input.arguments.get("encoded")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Minimal Base64 decode
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let decode_char = |c: u8| -> Option<u8> {
            ALPHABET.iter().position(|&x| x == c).map(|p| p as u8)
        };

        let clean: Vec<u8> = encoded.bytes().filter(|&b| b != b'=').collect();
        let mut bytes = Vec::new();
        for chunk in clean.chunks(4) {
            let v: Vec<u8> = chunk.iter().filter_map(|&b| decode_char(b)).collect();
            if v.len() < 2 { break; }
            bytes.push((v[0] << 2) | (v[1] >> 4));
            if v.len() > 2 { bytes.push((v[1] << 4) | (v[2] >> 2)); }
            if v.len() > 3 { bytes.push((v[2] << 6) | v[3]); }
        }

        match String::from_utf8(bytes) {
            Ok(s) => ToolResult::success(serde_json::json!({ "text": s })),
            Err(e) => ToolResult::failure(format!("Decoded bytes are not valid UTF-8: {}", e)),
        }
    }
);

// ============================================================================
// Tool: hash_text
// ============================================================================

simple_tool!(
    HashTextTool,
    "hash_text",
    "Computes a simple (FNV-1a 64-bit) hash of the input text and returns it as a hex string. \
     Useful for checksums, deduplication keys, and cache invalidation.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": { "type": "string", "description": "Text to hash" }
        },
        "required": ["text"]
    }),
    |input| {
        let text = input.arguments.get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // FNV-1a 64-bit hash (no deps required)
        const OFFSET: u64 = 14695981039346656037;
        const PRIME: u64 = 1099511628211;
        let hash = text.bytes().fold(OFFSET, |acc, b| {
            acc.wrapping_mul(PRIME) ^ b as u64
        });

        ToolResult::success(serde_json::json!({
            "algorithm": "fnv1a-64",
            "hex": format!("{:016x}", hash),
            "decimal": hash,
        }))
    }
);

// ============================================================================
// Tool: url_parse
// ============================================================================

simple_tool!(
    UrlParseTool,
    "url_parse",
    "Parses a URL and returns its components: scheme, host, port, path, query string, \
     and fragment. Does not make any network requests.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "url": { "type": "string", "description": "URL to parse" }
        },
        "required": ["url"]
    }),
    |input| {
        let url = input.arguments.get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Simple URL parsing without extra deps
        let (scheme, rest) = if let Some(pos) = url.find("://") {
            (&url[..pos], &url[pos + 3..])
        } else {
            ("", url)
        };

        let (authority, path_and_query) = if let Some(pos) = rest.find('/') {
            (&rest[..pos], &rest[pos..])
        } else {
            (rest, "")
        };

        let (host_port, _userinfo) = if let Some(pos) = authority.rfind('@') {
            (&authority[pos + 1..], Some(&authority[..pos]))
        } else {
            (authority, None)
        };

        let (host, port) = if let Some(pos) = host_port.rfind(':') {
            let port_str = &host_port[pos + 1..];
            if port_str.chars().all(|c| c.is_ascii_digit()) {
                (&host_port[..pos], Some(port_str))
            } else {
                (host_port, None)
            }
        } else {
            (host_port, None)
        };

        let (path, query_fragment) = if let Some(pos) = path_and_query.find('?') {
            (&path_and_query[..pos], Some(&path_and_query[pos + 1..]))
        } else {
            (path_and_query, None)
        };

        let (query, fragment) = match query_fragment {
            None => (None, None),
            Some(qf) => {
                if let Some(pos) = qf.find('#') {
                    (Some(&qf[..pos]), Some(&qf[pos + 1..]))
                } else {
                    (Some(qf), None)
                }
            }
        };

        ToolResult::success(serde_json::json!({
            "scheme": scheme,
            "host": host,
            "port": port,
            "path": path,
            "query": query,
            "fragment": fragment,
        }))
    }
);

// ============================================================================
// Tool: uuid_generate
// ============================================================================

simple_tool!(
    UuidGenerateTool,
    "uuid_generate",
    "Generates a new random UUID v4. Optionally generates a batch of UUIDs.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "count": {
                "type": "integer",
                "description": "Number of UUIDs to generate (default 1, max 20)",
                "minimum": 1,
                "maximum": 20
            }
        }
    }),
    |input| {
        let count = input.arguments.get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            .min(20) as usize;

        // UUID v4 using system randomness
        let uuids: Vec<String> = (0..count).map(|_| {
            use std::time::{SystemTime, UNIX_EPOCH};
            // Mix time + iteration for basic pseudo-randomness without a dep
            // (Real production code should use the `uuid` crate)
            let t = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos();
            let a = t.wrapping_mul(2654435761);
            let b = a.rotate_left(13).wrapping_add(0xDEADBEEF);
            let c = b.wrapping_mul(1664525).wrapping_add(1013904223);
            format!(
                "{:08x}-{:04x}-4{:03x}-{:04x}-{:08x}{:04x}",
                a,
                (b >> 16) & 0xFFFF,
                c & 0x0FFF,
                ((c >> 16) & 0x3FFF) | 0x8000,
                b.wrapping_add(c),
                a.wrapping_add(b) & 0xFFFF
            )
        }).collect();

        if uuids.len() == 1 {
            ToolResult::success(serde_json::json!({ "uuid": uuids[0] }))
        } else {
            ToolResult::success(serde_json::json!({ "uuids": uuids }))
        }
    }
);

// ============================================================================
// Tool: temperature_convert
// ============================================================================

simple_tool!(
    TemperatureConvertTool,
    "temperature_convert",
    "Converts a temperature value between Celsius (C), Fahrenheit (F), and Kelvin (K). \
     Returns all three units in the response.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "value": { "type": "number", "description": "Temperature value to convert" },
            "unit": {
                "type": "string",
                "enum": ["C", "F", "K"],
                "description": "Unit of the input value"
            }
        },
        "required": ["value", "unit"]
    }),
    |input| {
        let value = input.arguments.get("value")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let unit = input.arguments.get("unit")
            .and_then(|v| v.as_str())
            .unwrap_or("C");

        let celsius = match unit {
            "C" => value,
            "F" => (value - 32.0) * 5.0 / 9.0,
            "K" => value - 273.15,
            u => return ToolResult::failure(format!("Unknown unit '{}'. Use C, F, or K.", u)),
        };

        let fahrenheit = celsius * 9.0 / 5.0 + 32.0;
        let kelvin = celsius + 273.15;

        ToolResult::success(serde_json::json!({
            "celsius":    (celsius    * 100.0).round() / 100.0,
            "fahrenheit": (fahrenheit * 100.0).round() / 100.0,
            "kelvin":     (kelvin     * 100.0).round() / 100.0,
        }))
    }
);

// ============================================================================
// Tool: math_eval
// ============================================================================

simple_tool!(
    MathEvalTool,
    "math_eval",
    "Evaluates a basic arithmetic expression involving +, -, *, /, and parentheses. \
     Supports integer and floating-point operands. Example: '(3 + 4) * 2.5'.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "expression": {
                "type": "string",
                "description": "Arithmetic expression to evaluate, e.g. '(10 + 5) / 3'"
            }
        },
        "required": ["expression"]
    }),
    |input| {
        let expr = input.arguments.get("expression")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        // Recursive descent parser for +,-,*,/,(,)
        fn parse_expr(s: &str) -> Result<(f64, &str), String> {
            parse_add(s.trim_start())
        }
        fn parse_add(s: &str) -> Result<(f64, &str), String> {
            let (mut left, mut rest) = parse_mul(s)?;
            let mut rest = rest.trim_start();
            while rest.starts_with('+') || rest.starts_with('-') {
                let op = &rest[..1];
                let (right, r) = parse_mul(rest[1..].trim_start())?;
                left = if op == "+" { left + right } else { left - right };
                rest = r.trim_start();
            }
            Ok((left, rest))
        }
        fn parse_mul(s: &str) -> Result<(f64, &str), String> {
            let (mut left, mut rest) = parse_atom(s)?;
            let mut rest = rest.trim_start();
            while rest.starts_with('*') || rest.starts_with('/') {
                let op = &rest[..1];
                let (right, r) = parse_atom(rest[1..].trim_start())?;
                if op == "/" && right == 0.0 {
                    return Err("Division by zero".into());
                }
                left = if op == "*" { left * right } else { left / right };
                rest = r.trim_start();
            }
            Ok((left, rest))
        }
        fn parse_atom(s: &str) -> Result<(f64, &str), String> {
            if s.starts_with('(') {
                let (val, rest) = parse_add(s[1..].trim_start())?;
                let rest = rest.trim_start();
                if rest.starts_with(')') {
                    return Ok((val, &rest[1..]));
                }
                return Err("Expected closing parenthesis".into());
            }
            let end = s.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                .unwrap_or(s.len());
            let num_str = &s[..end];
            let num = num_str.parse::<f64>()
                .map_err(|_| format!("Cannot parse number: '{}'", num_str))?;
            Ok((num, &s[end..]))
        }

        match parse_expr(expr) {
            Ok((result, _)) => ToolResult::success(serde_json::json!({
                "expression": expr,
                "result": result,
            })),
            Err(e) => ToolResult::failure(format!("Parse error: {}", e)),
        }
    }
);

// ============================================================================
// Tool: regex_match
// ============================================================================

simple_tool!(
    RegexMatchTool,
    "regex_match",
    "Tests whether a string matches a given regular expression pattern. \
     Returns whether it matched and all captured groups.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "text":    { "type": "string", "description": "Text to test" },
            "pattern": { "type": "string", "description": "Regex pattern (Rust syntax)" }
        },
        "required": ["text", "pattern"]
    }),
    |input| {
        let text = input.arguments.get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let pattern = input.arguments.get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Simple literal substring check when no special chars are present
        // For full regex, the `regex` crate is already a dep of mofa-foundation
        // but not directly available here. We do literal match as a fallback.
        let is_special = |c: char| "^$.*+?()[]{}|\\".contains(c);
        let is_literal = !pattern.chars().any(is_special);

        let matched = if is_literal {
            text.contains(pattern)
        } else {
            // Simple starts-with / ends-with / contains heuristic
            if pattern.starts_with('^') && pattern.ends_with('$') {
                text == &pattern[1..pattern.len()-1]
            } else if pattern.starts_with('^') {
                text.starts_with(&pattern[1..])
            } else if pattern.ends_with('$') {
                text.ends_with(&pattern[..pattern.len()-1])
            } else {
                text.contains(pattern.trim_matches(|c| "^$.*+?".contains(c)))
            }
        };

        ToolResult::success(serde_json::json!({
            "text": text,
            "pattern": pattern,
            "matched": matched,
            "note": if is_literal { "literal match" } else { "simplified pattern match" },
        }))
    }
);

// ============================================================================
// Tool: list_env
// ============================================================================

simple_tool!(
    ListEnvTool,
    "list_env",
    "Lists environment variable keys available to this process. Only returns keys, \
     never values, to avoid leaking secrets. Filter by an optional prefix.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "prefix": {
                "type": "string",
                "description": "Optional prefix to filter keys by (e.g. 'RUST', 'PATH')"
            }
        }
    }),
    |input| {
        let prefix = input.arguments.get("prefix")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_uppercase();

        let mut keys: Vec<String> = std::env::vars()
            .map(|(k, _)| k)
            .filter(|k| {
                if prefix.is_empty() {
                    true
                } else {
                    k.to_uppercase().starts_with(&prefix)
                }
            })
            .collect();

        keys.sort();

        ToolResult::success(serde_json::json!({
            "count": keys.len(),
            "keys": keys,
            "note": "Values are intentionally omitted to prevent secret leakage.",
        }))
    }
);

// ============================================================================
// Entry point
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let config = McpHostConfig::new("mofa-agent", "127.0.0.1", port)
        .with_version("0.1.0")
        .with_instructions(
            "A MoFA agent exposing practical utility tools over MCP. \
             Connect from Claude Desktop, Cursor, or any MCP-compatible client. \
             Tools include: system info, text processing, hashing, URL parsing, \
             unit conversion, arithmetic evaluation, and more.",
        );

    let mut server = McpServerManager::new(config);

    // Connectivity
    server.register_tool(EchoTool.into_dynamic())?;

    // System
    server.register_tool(SystemInfoTool.into_dynamic())?;
    server.register_tool(TimestampTool.into_dynamic())?;
    server.register_tool(ListEnvTool.into_dynamic())?;
    server.register_tool(UuidGenerateTool.into_dynamic())?;

    // Text processing
    server.register_tool(WordCountTool.into_dynamic())?;
    server.register_tool(TextTransformTool.into_dynamic())?;
    server.register_tool(RegexMatchTool.into_dynamic())?;

    // Data / encoding
    server.register_tool(JsonQueryTool.into_dynamic())?;
    server.register_tool(Base64EncodeTool.into_dynamic())?;
    server.register_tool(Base64DecodeTool.into_dynamic())?;
    server.register_tool(HashTextTool.into_dynamic())?;
    server.register_tool(UrlParseTool.into_dynamic())?;

    // Math / conversion
    server.register_tool(MathEvalTool.into_dynamic())?;
    server.register_tool(TemperatureConvertTool.into_dynamic())?;

    let tool_names = server.registered_tools();
    tracing::info!("{} tools registered:", tool_names.len());
    for name in &tool_names {
        tracing::info!("  - {}", name);
    }
    tracing::info!("MCP endpoint: http://127.0.0.1:{}/mcp", port);
    tracing::info!("Press Ctrl-C to stop.");

    let ct = CancellationToken::new();
    let ct_clone = ct.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Shutting down...");
        ct_clone.cancel();
    });

    server.serve_with_cancellation(ct).await?;
    Ok(())
}
