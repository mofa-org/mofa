# Contributing

Thank you for your interest in contributing to MoFA!

## Getting Started

### 1. Fork and Clone

```bash
git clone https://github.com/YOUR_USERNAME/mofa.git
cd mofa
```

### 2. Set Up Development Environment

```bash
# Install Rust (1.85+)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the project
cargo build

# Run tests
cargo test
```

### 3. Create a Branch

```bash
git checkout -b feature/your-feature-name
```

## Development Guidelines

### Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and fix all warnings
- Follow Rust naming conventions
- Add documentation comments (`///`) for public APIs

### Architecture

MoFA follows strict microkernel architecture. See [CLAUDE.md](https://github.com/mofa-org/mofa/blob/main/CLAUDE.md) for detailed rules:

- **Kernel layer**: Only trait definitions, no implementations
- **Foundation layer**: Concrete implementations
- **Never re-define traits** from kernel in foundation

### Commit Messages

Follow conventional commits:

```
feat: add new tool registry implementation
fix: resolve memory leak in agent context
docs: update installation guide
test: add tests for LLM client
refactor: simplify workflow execution
```

### Testing

- Write unit tests for new functionality
- Ensure all tests pass: `cargo test`
- Test with different feature flags if applicable

```bash
# Run all tests
cargo test --all-features

# Test specific crate
cargo test -p mofa-sdk

# Test with specific features
cargo test -p mofa-sdk --features openai
```

## Pull Request Process

1. **Create an issue** first for significant changes
2. **Make your changes** following the guidelines above
3. **Update documentation** if needed
4. **Run all checks**:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-features
```

5. **Submit PR** with a clear description

### PR Checklist

- [ ] Code compiles without warnings
- [ ] Tests pass
- [ ] Documentation updated
- [ ] CLAUDE.md architecture rules followed
- [ ] Commit messages follow convention

## Documentation

- Update relevant `.md` files in `docs/`
- Add inline documentation for public APIs
- Update `CHANGELOG.md` for notable changes

## Questions?

- Open an issue for bugs or feature requests
- Join [Discord](https://discord.com/invite/hKJZzDMMm9) for discussions
- Check [GitHub Discussions](https://github.com/mofa-org/mofa/discussions)

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
