# Contributing to MoFA

We welcome contributions! Here's how to get started.

## Development Setup

```bash
# Clone the repository
git clone https://github.com/mofa-org/mofa.git
cd mofa

# Build the workspace
cargo build

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run linter
cargo clippy
```

## How to Contribute

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Make your changes
4. Run `cargo fmt` and `cargo clippy` to ensure code quality
5. Run `cargo test` to verify nothing is broken
6. Commit your changes with a descriptive message
7. Push to your fork and open a Pull Request

## Code Style

- Follow standard Rust conventions
- Run `cargo fmt` before committing
- Ensure `cargo clippy` passes without warnings
- Add tests for new functionality

## Security Guidelines

We take security seriously. When contributing to MoFA, please follow these security best practices:

### Secret Management

- **NEVER commit secrets, credentials, or sensitive data** to the repository
- Use environment variables for configuration secrets
- Add `.env` files to `.gitignore`
- Use placeholder values in examples and documentation

```rust
// DO
let api_key = std::env::var("OPENAI_API_KEY")?;

// DO NOT
let api_key = "sk-abc123...";  // Never hardcode credentials
```

### Secure Coding Practices

- Validate all input parameters
- Sanitize all output data
- Use error handling (avoid `unwrap()` and `expect()` in production code)
- Follow the principle of least privilege
- Avoid unsafe code when possible
- Use type-safe interfaces

### Dependencies

- Keep dependencies up to date
- Review security advisories for dependencies
- Use `cargo-audit` to check for vulnerable dependencies:
  ```bash
  cargo install cargo-audit
  cargo audit
  ```
- Use `cargo-deny` to enforce dependency policies:
  ```bash
  cargo install cargo-deny
  cargo deny check
  ```

### Security Review Process

- All code changes are subject to review
- Security-relevant changes receive additional scrutiny
- Maintainers may request security-focused changes
- Security considerations should be documented in PR descriptions

### Reporting Security Issues

- **DO NOT report security vulnerabilities in public issues or PRs**
- Report security vulnerabilities privately through [GitHub Security Advisories](https://github.com/mofa-org/mofa/security/advisories)
- See [SECURITY.md](SECURITY.md) for detailed reporting instructions

### Testing

- Write tests for security-critical code paths
- Test for common vulnerabilities (injection attacks, buffer overflows, etc.)
- Use property-based testing for input validation
- Test error handling paths

For more information on security best practices, see our [Security Guide](docs/security.md).

## Architecture

Please read [CLAUDE.md](CLAUDE.md) for the project architecture and layering rules before making changes. Key points:

- **mofa-kernel**: Trait definitions and core types only (no concrete implementations)
- **mofa-foundation**: Concrete implementations and business logic
- **mofa-plugins**: Plugin implementations
- **mofa-sdk**: Public API surface

## Reporting Issues

- Use [GitHub Issues](https://github.com/mofa-org/mofa/issues) for bug reports
- Use [GitHub Discussions](https://github.com/mofa-org/mofa/discussions) for questions

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).
