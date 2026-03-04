# MoFA Documentation

This directory contains the mdBook documentation for MoFA (Modular Framework for Agents).

## Building

### Prerequisites

Install mdBook and required plugins:

```bash
cargo install mdbook
cargo install mdbook-mermaid
```

### Build (EN + ZH)

```bash
cd docs/mofa-doc
./scripts/build-docs.sh
```

The output will be in:
- `book/` (English)
- `book/zh/` (Simplified Chinese)

### Serve Locally

```bash
# Build EN + ZH and serve as one site (recommended)
./scripts/preview-docs.sh 3000
```

Open:
- English: `http://localhost:3000`
- Chinese: `http://localhost:3000/zh/`

Notes:
- `mdbook serve` only serves one mdBook config at a time, so `mdbook serve` (English) does not include `/zh/*`.
- If you only need English live-reload while writing docs, use `mdbook serve` separately.

## Structure

```
docs/mofa-doc/
├── book.toml              # Configuration
├── book.zh.toml           # Chinese configuration
├── scripts/               # Build/deploy helper scripts
├── src/                   # Content
│   ├── SUMMARY.md         # Table of contents
│   ├── introduction.md    # Introduction
│   ├── getting-started/   # Getting started guides
│   ├── concepts/          # Core concepts
│   ├── tutorial/          # Step-by-step tutorial
│   ├── guides/            # How-to guides
│   ├── api-reference/     # API documentation
│   ├── examples/          # Example documentation
│   ├── ffi/               # Cross-language bindings
│   ├── advanced/          # Advanced topics
│   ├── crates/            # Crate documentation
│   ├── zh/                # Chinese translation
│   └── appendix/          # Appendix
```

## Deployment

The documentation is automatically deployed to GitHub Pages via GitHub Actions when changes are pushed to the `main` branch.

Workflow:
- `.github/workflows/deploy-docs.yml`
- Build command: `./scripts/build-docs.sh`
- Publish directory: `docs/mofa-doc/book`

### Chinese Site Deployment (GitHub Pages)

The Chinese site is deployed as a subpath of the same Pages site:
- English: `https://mofa.ai/mofa/`
- Chinese: `https://mofa.ai/mofa/zh/`

Build output requirements before publishing:
- English pages exist under `book/`
- Chinese pages exist under `book/zh/`
- `book/zh/index.html` is present

Quick verification:

```bash
cd docs/mofa-doc
./scripts/check-internal-links.sh
./scripts/build-docs.sh
test -f book/index.html && echo "EN OK"
test -f book/zh/index.html && echo "ZH OK"
```

Recommended pre-PR validation:

```bash
cd docs/mofa-doc
./scripts/check-internal-links.sh
./scripts/build-docs.sh
```

### Manual Deployment

```bash
./scripts/build-docs.sh
# Copy the full book/ directory to your hosting root
# Ensure book/zh/ is included, otherwise Chinese pages will 404
```

## Contributing

1. Edit files in `src/`
2. Run `mdbook serve` to preview
3. Submit a pull request

### Guidelines

- Use English for primary documentation
- Add Chinese translations in `src/zh/`
- Follow the existing structure
- Include code examples where appropriate
- Keep pages focused and concise

## License

Apache License 2.0
