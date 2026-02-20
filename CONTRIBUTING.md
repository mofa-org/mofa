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
