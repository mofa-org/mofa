# 版本化文档

MoFA 文档与 crate 版本一起发布。本页说明如何访问框架特定版本的文档。

## 当前文档

您正在阅读的文档反映了 [mofa 仓库](https://github.com/mofa-org/mofa) `main` 分支的最新代码。

## 访问特定版本的文档

### 在线文档（推荐）

[https://mofa.ai/mofa/](https://mofa.ai/mofa/) 上的在线文档跟踪 `main` 分支，并通过 GitHub Actions 在每次推送时自动部署。

对于特定发布版本，请在 GitHub 上导航到对应的 Git 标签，直接浏览 `docs/mofa-doc/src/` 目录：

```
https://github.com/mofa-org/mofa/tree/<tag>/docs/mofa-doc/src
```

例如，对于 `v0.1.0`：

```
https://github.com/mofa-org/mofa/tree/v0.1.0/docs/mofa-doc/src
```

### 本地构建特定版本的文档

1. 检出所需标签：

   ```bash
   git checkout v0.1.0
   ```

2. 安装 `mdbook` 和 `mdbook-mermaid`：

   ```bash
   cargo install mdbook
   cargo install mdbook-mermaid
   ```

3. 构建文档：

   ```bash
   cd docs/mofa-doc
   ./scripts/build-docs.sh
   ```

4. 在浏览器中打开 `docs/mofa-doc/book/index.html`。

### `cargo doc` API 参考

要生成源代码注释中的内联 Rust API 参考，请运行：

```bash
cargo doc --open
```

这会为工作区中的所有 crate 生成 `rustdoc` 输出，并在默认浏览器中打开索引。

## MoFA 版本策略

MoFA 遵循[语义化版本](https://semver.org/lang/zh-CN/)规范：

| 版本组件 | 含义 |
|----------|------|
| **主版本**（`X.0.0`）| 不兼容的 API 变更 |
| **次版本**（`0.X.0`）| 向后兼容的新功能 |
| **补丁版本**（`0.0.X`）| 向后兼容的错误修复 |

在 API 达到稳定性（1.0.0）之前，预发布版本标记为 `v0.x.x`。

## 范围说明：智能体中心

**智能体中心**（Agent Hub，一个可搜索的可复用智能体节点目录）**不是** MoFA Rust 实现的交付物。MoFA RS 版本尚未开始构建智能体中心生态系统。智能体中心功能属于独立的生态层，将在本仓库之外的专属工作中单独追踪。
