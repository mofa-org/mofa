# Versioned Documentation

MoFA documentation is versioned alongside the crate releases. This page explains how to access documentation for specific versions of the framework.

## Current Documentation

The documentation you are reading reflects the latest code on the `main` branch of the [mofa repository](https://github.com/mofa-org/mofa).

## Accessing Version-Specific Docs

### Hosted Docs (Recommended)

The live documentation site at [https://mofa.ai/mofa/](https://mofa.ai/mofa/) tracks the `main` branch and is deployed automatically on every push via GitHub Actions.

For a specific release, navigate to the corresponding Git tag on GitHub and browse the `docs/mofa-doc/src/` directory directly:

```
https://github.com/mofa-org/mofa/tree/<tag>/docs/mofa-doc/src
```

For example, for `v0.1.0`:

```
https://github.com/mofa-org/mofa/tree/v0.1.0/docs/mofa-doc/src
```

### Building Docs Locally for a Specific Version

1. Check out the desired tag:

   ```bash
   git checkout v0.1.0
   ```

2. Install `mdbook` and `mdbook-mermaid`:

   ```bash
   cargo install mdbook
   cargo install mdbook-mermaid
   ```

3. Build the docs:

   ```bash
   cd docs/mofa-doc
   ./scripts/build-docs.sh
   ```

4. Open `docs/mofa-doc/book/index.html` in your browser.

### `cargo doc` API Reference

For the inline Rust API reference generated from source comments, run:

```bash
cargo doc --open
```

This generates `rustdoc` output for all crates in the workspace and opens the index in your default browser.

## MoFA Versioning Policy

MoFA follows [Semantic Versioning](https://semver.org/):

| Version component | Meaning |
|-------------------|---------|
| **Major** (`X.0.0`) | Breaking API changes |
| **Minor** (`0.X.0`) | New features, backward-compatible |
| **Patch** (`0.0.X`) | Bug fixes, backward-compatible |

Pre-release builds are tagged as `v0.x.x` until the API reaches stability (1.0.0).

## Scope Note: Agent Hub

The **Agent Hub** (a searchable catalog of reusable agent nodes) is **not** a deliverable of the MoFA Rust implementation. The MoFA RS version has not yet started building the agent hub ecosystem. Agent Hub functionality belongs to a separate ecosystem layer and will be tracked in its own dedicated effort outside this repository.
