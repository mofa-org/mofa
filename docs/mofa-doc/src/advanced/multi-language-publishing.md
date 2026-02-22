# Multi-Language Publishing

Publish MoFA bindings for multiple programming languages.

## Overview

MoFA supports publishing bindings for:
- Python (PyPI)
- Java (Maven Central)
- Go (Go modules)
- Swift (Swift Package Manager)
- Kotlin (Maven Central)

## Python (PyPI)

### Build Wheel

```bash
# Install maturin
pip install maturin

# Build wheel
maturin build --release

# Publish to PyPI
maturin publish
```

### pyproject.toml

```toml
[project]
name = "mofa"
version = "0.1.0"
description = "MoFA Python bindings"

[build-system]
requires = ["maturin>=1.0"]
build-backend = "maturin"
```

## Java (Maven Central)

### Build JAR

```bash
# Build with Gradle
./gradlew build

# Publish to Maven Central
./gradlew publish
```

### build.gradle

```groovy
plugins {
    id 'java-library'
    id 'maven-publish'
}

publishing {
    publications {
        mavenJava(MavenPublication) {
            from components.java
            groupId = 'org.mofa'
            artifactId = 'mofa-java'
            version = '0.1.0'
        }
    }
}
```

## Go

### Publish Module

```bash
# Tag release
git tag v0.1.0
git push origin v0.1.0

# Module is available at:
# github.com/mofa-org/mofa-go
```

## Swift (Swift Package Manager)

### Package.swift

```swift
let package = Package(
    name: "MoFA",
    products: [
        .library(name: "MoFA", targets: ["MoFA"]),
    ],
    targets: [
        .binaryTarget(
            name: "MoFA",
            url: "https://github.com/mofa-org/mofa-swift/releases/download/0.1.0/MoFA.xcframework.zip",
            checksum: "..."
        ),
    ]
)
```

## Versioning

Use semantic versioning (semver):
- `MAJOR.MINOR.PATCH`
- Major: Breaking changes
- Minor: New features, backward compatible
- Patch: Bug fixes

## Release Checklist

- [ ] Update version in all Cargo.toml files
- [ ] Update version in language-specific manifests
- [ ] Run full test suite
- [ ] Update CHANGELOG.md
- [ ] Create git tag
- [ ] Build all language bindings
- [ ] Publish to respective registries

## See Also

- [Cross-Language Bindings](../ffi/README.md) — FFI overview
- [Contributing](../appendix/contributing.md) — Contribution guide
