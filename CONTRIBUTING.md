# Contributing to MoFA

Thank you for your interest in contributing to MoFA! This document covers everything you need to get started as a contributor to the Rust implementation of MoFA.

## Table of Contents

- [Rust Toolchain Setup](#rust-toolchain-setup)
- [Common Commands](#common-commands)
- [Architecture Overview](#architecture-overview)
- [Microkernel Layer Rules](#microkernel-layer-rules)
- [Branch Naming Conventions](#branch-naming-conventions)
- [Commit Message Conventions](#commit-message-conventions)
- [Pull Request Guidelines](#pull-request-guidelines)
- [Reporting Issues and Discussions](#reporting-issues-and-discussions)
- [License](#license)

---

## Rust Toolchain Setup

MoFA requires a recent stable Rust toolchain. We target **Rust edition 2024**.

```bash
# Install rustup (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install the latest stable toolchain
rustup toolchain install stable
rustup default stable

# Verify installation
rustc --version   # should be 1.85+ for edition 2024 support
cargo --version

# Clone the repository
git clone https://github.com/mofa-org/mofa.git
cd mofa
```

---

## Common Commands

```bash
# Build the entire workspace
cargo build
cargo build --release

# Build a specific crate
cargo build -p mofa-sdk
cargo build -p mofa-cli

# Run all tests
cargo test

# Run tests for a specific crate (preferred for focused development)
cargo test -p mofa-sdk
cargo test -p mofa-runtime
cargo test -p mofa-plugins

# Run a specific test by name
cargo test -p mofa-sdk -- test_name

# Format code (run before every commit)
cargo fmt

# Check formatting without modifying files
cargo fmt --check

# Run the linter (must pass with no warnings before opening a PR)
cargo clippy

# Fast compilation check (no output artifacts)
cargo check

# Run the CLI
cargo run -p mofa-cli -- mofa --help
```

---

## Architecture Overview

Before making changes, please read:

- **[CLAUDE.md](CLAUDE.md)** — Full architecture description, workspace structure, feature flags, and layering rules.
- **[docs/architecture.md](docs/architecture.md)** — High-level design document.

### Workspace Structure

```
mofa/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── mofa-kernel/        # Microkernel core — traits and core types ONLY
│   ├── mofa-foundation/    # Concrete implementations and business logic
│   ├── mofa-runtime/       # Runtime system (message bus, registry, event loop)
│   ├── mofa-plugins/       # Plugin system (dual-layer architecture)
│   ├── mofa-cli/           # CLI tool (`mofa` command)
│   ├── mofa-sdk/           # Main SDK — public API surface
│   ├── mofa-ffi/           # FFI bindings (UniFFI for Python, Java, Go, Kotlin, Swift)
│   ├── mofa-macros/        # Procedural macros
│   ├── mofa-monitoring/    # Monitoring and observability
│   └── mofa-extra/         # Additional utilities
└── examples/               # 27+ usage examples
```

---

## Microkernel Layer Rules

MoFA enforces a strict layered architecture. Violating these rules will block a PR.

| Layer | Allowed | Forbidden |
|---|---|---|
| `mofa-kernel` | Trait definitions, core data types (`AgentInput`, `AgentOutput`, `AgentState`) | Concrete implementations, business logic |
| `mofa-foundation` | Concrete implementations, business-specific types | Re-defining kernel traits, depending on `mofa-foundation` from kernel |
| `mofa-plugins` | Plugin adapters and concrete plugin implementations | — |
| `mofa-sdk` | Re-exports of user-facing APIs | Heavy logic (delegate to lower crates) |

**Dependency direction** (arrows = "may depend on"):

```
mofa-sdk → mofa-runtime → mofa-foundation → mofa-kernel
mofa-plugins → mofa-foundation → mofa-kernel
```

`mofa-kernel` must **never** depend on `mofa-foundation` (circular dependency).

### Quick checklist before opening a PR

- [ ] New trait definitions live in `mofa-kernel`.
- [ ] Concrete `struct` implementations live in `mofa-foundation` or `mofa-plugins`.
- [ ] `mofa-foundation` does **not** re-define a trait already present in `mofa-kernel`.
- [ ] No new circular dependencies introduced (`cargo check` catches these).

---

## Branch Naming Conventions

| Type | Pattern | Example |
|---|---|---|
| New feature | `feature/<short-description>` | `feature/rhai-hot-reload` |
| Bug fix | `fix/<short-description>` | `fix/registry-deadlock` |
| Documentation | `docs/<short-description>` | `docs/add-contributing` |
| Refactor | `refactor/<short-description>` | `refactor/kernel-trait-split` |
| Chore / CI | `chore/<short-description>` | `chore/update-dependencies` |

Use **lowercase kebab-case** for all branch names.

---

## Commit Message Conventions

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <short summary>

[optional body]

[optional footer]
```

**Types:** `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`, `ci`

**Scope** should be the crate name without the `mofa-` prefix (e.g., `kernel`, `foundation`, `sdk`, `cli`).

**Examples:**

```
feat(sdk): add secretary agent draft PR workflow
fix(runtime): resolve deadlock in AgentRegistry under high concurrency
docs(kernel): clarify trait definition placement rules
refactor(foundation): extract SimpleToolRegistry to its own module
```

- Keep the summary line under **72 characters**.
- Use the imperative mood ("add", "fix", "remove" — not "added" or "fixes").
- Reference issues in the footer: `Closes #42` or `Related to #17`.

---

## Pull Request Guidelines

### Before opening a PR

1. **Fork** the repository and work on your own fork.
2. Base your branch on the latest `main`.
3. Run the full quality gate locally:
   ```bash
   cargo fmt --check
   cargo clippy
   cargo test
   ```
4. Make sure every commit compiles on its own (`cargo check` per commit).

### Draft PRs

Open a **draft PR** early when:
- You want early feedback on direction before the implementation is complete.
- The change is large and you want to discuss the approach first.

Mark it as "Ready for review" only when `cargo fmt --check`, `cargo clippy`, and `cargo test` all pass.

### PR description template

```markdown
## Summary
<!-- What does this PR do? -->

## Motivation
<!-- Why is this change needed? -->

## Changes
<!-- Bullet list of what was changed and in which crate. -->

## Related Issues
<!-- Link to a related issue if present -->

## Testing
<!-- How was this tested? New unit tests? Manual verification? -->

## Checklist
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo test` passes
- [ ] Architecture layer rules respected (see CONTRIBUTING.md)
- [ ] Relevant documentation updated
```

### Review process

- At least **one maintainer approval** is required to merge.
- Address all review comments before requesting a re-review.
- Prefer small, focused PRs over large monolithic ones — they get reviewed faster.

---

## Reporting Issues and Discussions

- **Bug reports & feature requests** → [GitHub Issues](https://github.com/mofa-org/mofa/issues)
  - Search for existing issues before opening a new one.
  - For bugs, include: Rust version (`rustc --version`), OS, and a minimal reproducer.
- **Questions, ideas, and general discussion** → [GitHub Discussions](https://github.com/mofa-org/mofa/discussions)
- **Security vulnerabilities** → Do **not** open a public issue. Email the maintainers directly (see [README.md](README.md)).
- **Community chat** → [Discord](https://discord.com/invite/hKJZzDMMm9)

---

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).
