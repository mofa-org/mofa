# 多语言发布

为多种编程语言发布 MoFA 绑定。

## 概述

MoFA 支持为以下语言发布绑定:
- Python (PyPI)
- Java (Maven Central)
- Go (Go modules)
- Swift (Swift Package Manager)
- Kotlin (Maven Central)

## Python (PyPI)

### 构建 Wheel

```bash
# 安装 maturin
pip install maturin

# 构建 wheel
maturin build --release

# 发布到 PyPI
maturin publish
```

### pyproject.toml

```toml
[project]
name = "mofa"
version = "0.1.0"
description = "MoFA Python 绑定"

[build-system]
requires = ["maturin>=1.0"]
build-backend = "maturin"
```

## Java (Maven Central)

### 构建 JAR

```bash
# 使用 Gradle 构建
./gradlew build

# 发布到 Maven Central
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

### 发布模块

```bash
# 标记发布
git tag v0.1.0
git push origin v0.1.0

# 模块可用地址:
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

## 版本控制

使用语义化版本控制 (semver):
- `主版本.次版本.修订版本`
- 主版本: 破坏性更改
- 次版本: 新功能，向后兼容
- 修订版本: 错误修复

## 发布检查清单

- [ ] 更新所有 Cargo.toml 文件中的版本
- [ ] 更新语言特定清单中的版本
- [ ] 运行完整测试套件
- [ ] 更新 CHANGELOG.md
- [ ] 创建 git 标签
- [ ] 构建所有语言绑定
- [ ] 发布到各自的注册表

## 另见

- [跨语言绑定](../ffi/README.md) — FFI 概述
- [贡献](../appendix/contributing.md) — 贡献指南
