# 多语言发布实施总结

## 概述

本文档总结了 MoFA SDK 多语言发布的实施情况，支持分发到 Rust (crates.io)、Python (PyPI)、Java (Maven Central) 和 Go (Go 模块注册表)。

## 实施状态

### ✅ 已完成

#### 1. Python 包基础设施
- **文件**: `crates/mofa-sdk/bindings/python/pyproject.toml`
  - 使用 `maturin` 的现代 Python 打包配置
  - 包元数据和依赖项
  - 原生 Rust 扩展的构建配置

- **文件**: `crates/mofa-sdk/bindings/python/MANIFEST.in`
  - 指定包含文件的包清单
  - 原生库包含规则

- **文件**: `crates/mofa-sdk/bindings/python/README.md`
  - 包文档
  - 安装说明
  - 使用示例

#### 2. Java 包基础设施
- **文件**: `crates/mofa-sdk/bindings/java/pom.xml`
  - Maven 项目配置
  - Maven Central 的 GPG 签名设置
  - Nexus 暂存配置
  - 源码和 Javadoc 附加

- **文件**: `crates/mofa-sdk/bindings/java/README.md`
  - Maven/Gradle 使用文档
  - 安装说明
  - 代码示例

#### 3. Go 模块基础设施
- **文件**: `crates/mofa-sdk/bindings/go/go.mod`
  - Go 模块声明
  - 模块路径: `github.com/mofa-org/mofa-go`

- **文件**: `crates/mofa-sdk/bindings/go/README.md`
  - Go 模块使用文档
  - `go get` 说明
  - 代码示例

#### 4. 扩展的发布脚本
- **文件**: `scripts/release.sh`
  - 新的命令行选项：
    - `--publish-pypi`: 发布 Python 包到 PyPI
    - `--publish-maven`: 发布 Java 包到 Maven Central
    - `--publish-go`: 发布 Go 模块
    - `--publish-all`: 发布到所有注册表

  - 新的发布步骤：
    - 步骤 7.5: 生成语言绑定
    - 步骤 8: 发布到 PyPI（使用 maturin + twine）
    - 步骤 9: 发布到 Maven Central（使用 Maven）
    - 步骤 10: 发布 Go 模块（通过 git tag）

#### 5. CI/CD 工作流
- **文件**: `.github/workflows/publish-all.yml`
  - git 标签触发自动多平台发布
  - 每种语言独立的作业
  - 正确的作业依赖关系（validate → publish）

### 📋 需要设置（首次发布前）

#### PyPI 设置
1. 在 https://pypi.org 创建 PyPI 账户
2. 启用 2FA 并创建 API token
3. 将 `PYPI_API_TOKEN` 添加到 GitHub Secrets

#### Maven Central 设置
1. 在 https://central.sonatype.com/ 创建 OSSRH 账户
2. 创建新的命名空间（如 `org.mofa`）
3. 生成 GPG 密钥：
   ```bash
   gpg --full-generate-key
   gpg --keyserver keyserver.ubuntu.com --send-keys YOUR_KEY_ID
   ```
4. 配置 Maven 设置（`~/.m2/settings.xml`）
5. 添加到 GitHub Secrets：
   - `MAVEN_USERNAME`: OSSRH token 用户名
   - `MAVEN_PASSWORD`: OSSRH token 密码
   - `GPG_PRIVATE_KEY`: Base64 编码的私钥
   - `GPG_PASSPHRASE`: GPG 密钥密码

#### Go 模块设置
无需特殊设置 — Go 模块通过 git 标签自动发现。

## 使用示例

### 手动发布

#### 测试 Python 发布（dry-run）：
```bash
./scripts/release.sh 0.1.0 --dry-run --publish-pypi
```

#### 发布到所有注册表：
```bash
./scripts/release.sh 0.1.0 --publish-all --git-tag
```

#### 发布到特定注册表：
```bash
./scripts/release.sh 0.1.0 --publish-pypi --publish-maven --git-tag
```

### 自动发布（GitHub Actions）

当你推送版本标签时：
```bash
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

工作流将：
1. 验证和测试
2. 发布 Rust crates 到 crates.io
3. 构建并发布 Python wheels 到 PyPI
4. 构建并发布 Java JAR 到 Maven Central
5. 创建并推送 Go 模块标签
6. 创建带有二进制文件的 GitHub release

## 验证步骤

发布后，验证每个包：

### Python:
```bash
pip install mofa-sdk==0.1.0
python -c "import mofa; print(mofa.get_version())"
```

### Java:
```bash
# 在 pom.xml 中：
# <dependency>
#   <groupId>org.mofa</groupId>
#   <artifactId>mofa-sdk</artifactId>
#   <version>0.1.0</version>
# </dependency>

mvn compile
```

### Go:
```bash
go get github.com/mofa-org/mofa-go@v0.1.0
```

## 修改/创建的文件

| 文件 | 状态 | 用途 |
|------|--------|---------|
| `scripts/release.sh` | 修改 | 添加多语言发布 |
| `crates/mofa-sdk/bindings/python/pyproject.toml` | 创建 | Python 包配置 |
| `crates/mofa-sdk/bindings/python/MANIFEST.in` | 创建 | Python 包清单 |
| `crates/mofa-sdk/bindings/python/README.md` | 创建 | Python 文档 |
| `crates/mofa-sdk/bindings/java/pom.xml` | 创建 | Maven 项目配置 |
| `crates/mofa-sdk/bindings/java/README.md` | 创建 | Java 文档 |
| `crates/mofa-sdk/bindings/go/go.mod` | 创建 | Go 模块配置 |
| `crates/mofa-sdk/bindings/go/README.md` | 创建 | Go 文档 |
| `.github/workflows/publish-all.yml` | 创建 | CI/CD 工作流 |

## 故障排除

### PyPI 发布问题
- 确保 `maturin` 和 `twine` 已安装
- 检查 `PYPI_API_TOKEN` 是否有效
- 验证 `pyproject.toml` 中的版本是否匹配

### Maven Central 问题
- 确保 GPG 密钥配置正确
- 验证 OSSRH 凭证是否正确
- 检查命名空间是否已被 Sonatype 验证

### Go 模块问题
- 确保 git 标签格式为 `go/vX.Y.Z`
- 验证标签已推送到 origin
- 给 Go 代理时间索引标签

## 注意事项

- **版本同步**：所有包使用与 Rust crate 相同的版本
- **发布顺序**：Rust → Python → Java → Go（Go 不依赖其他）
- **Go 模块**：通过 git 标签自动发现，无需手动注册
- **PyPI**：支持 test.pypi.org 用于预发布测试

---

[English](../MULTI_LANGUAGE_PUBLISHING.md) | **简体中文**
