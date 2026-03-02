//! `mofa gateway` command implementations

use colored::Colorize;
use mofa_runtime::gateway::{BackendConfig, GatewayConfig, run_gateway};

/// Execute `mofa gateway serve`
pub async fn run_serve(host: &str, port: u16, backends: &[String], rpm: u32) -> anyhow::Result<()> {
    let backend_configs = backends
        .iter()
        .enumerate()
        .map(|(idx, spec)| parse_backend_spec(idx + 1, spec))
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut cfg = GatewayConfig::new(host.to_string(), port, backend_configs);
    cfg.rate_limit.requests_per_minute = rpm;

    println!(
        "{} Starting gateway on {}:{} with {} backend(s)",
        "→".green(),
        host,
        port,
        backends.len()
    );
    run_gateway(cfg).await
}

fn parse_backend_spec(index: usize, spec: &str) -> anyhow::Result<BackendConfig> {
    let (name_part, rhs) = if let Some((name, value)) = spec.split_once('=') {
        (Some(name.trim().to_string()), value.trim())
    } else {
        (None, spec.trim())
    };

    let (url, weight) = if let Some((url, weight)) = rhs.rsplit_once('@') {
        let parsed_weight = weight
            .trim()
            .parse::<u32>()
            .map_err(|_| anyhow::anyhow!("Invalid backend weight '{}'", weight.trim()))?;
        (url.trim().to_string(), parsed_weight)
    } else {
        (rhs.to_string(), 1)
    };

    if url.is_empty() {
        anyhow::bail!("Backend URL cannot be empty");
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        anyhow::bail!("Backend URL must start with http:// or https://");
    }
    if weight == 0 {
        anyhow::bail!("Backend weight must be >= 1");
    }

    let name = name_part.unwrap_or_else(|| format!("backend-{}", index));
    Ok(BackendConfig::new(name, url, weight))
}

#[cfg(test)]
mod tests {
    use super::parse_backend_spec;

    #[test]
    fn parses_url_only() {
        let backend = parse_backend_spec(1, "http://localhost:11434").unwrap();
        assert_eq!(backend.name, "backend-1");
        assert_eq!(backend.base_url, "http://localhost:11434");
        assert_eq!(backend.weight, 1);
    }

    #[test]
    fn parses_named_weighted_backend() {
        let backend = parse_backend_spec(3, "primary=http://localhost:9001@5").unwrap();
        assert_eq!(backend.name, "primary");
        assert_eq!(backend.base_url, "http://localhost:9001");
        assert_eq!(backend.weight, 5);
    }

    #[test]
    fn rejects_invalid_weight() {
        let err = parse_backend_spec(1, "http://localhost:9001@abc").unwrap_err();
        assert!(err.to_string().contains("Invalid backend weight"));
    }

    #[test]
    fn rejects_missing_scheme() {
        let err = parse_backend_spec(1, "localhost:9001").unwrap_err();
        assert!(
            err.to_string()
                .contains("must start with http:// or https://")
        );
    }
}
