# 贡献

感谢您有兴趣为 MoFA 做出贡献！

## 开始

### 1. Fork 和克隆

```bash
git clone https://github.com/YOUR_USERNAME/mofa.git
cd mofa
```

### 2. 设置开发环境

```bash
# 安装 Rust (1.85+)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 构建项目
cargo build

# 运行测试
cargo test
```

### 3. 创建分支

```bash
git checkout -b feature/your-feature-name
```

## 开发指南

### 代码风格

- 提交前运行 `cargo fmt`
- 运行 `cargo clippy` 并修复所有警告
- 遵循 Rust 命名约定
- 为公共 API 添加文档注释 (`///`)

### 架构

MoFA 遵循严格的微内核架构。详见 [CLAUDE.md](https://github.com/mofa-org/mofa/blob/main/CLAUDE.md):

- **内核层**: 仅 trait 定义，无实现
- **基础层**: 具体实现
- **永远不要在基础层重新定义**内核层的 trait

### 提交消息

遵循约定式提交:

```
feat: 添加新的工具注册表实现
fix: 解决智能体上下文中的内存泄漏
docs: 更新安装指南
test: 为 LLM 客户端添加测试
refactor: 简化工作流执行
```

### 测试

- 为新功能编写单元测试
- 确保所有测试通过: `cargo test`
- 如果适用，测试不同的功能标志

```bash
# 运行所有测试
cargo test --all-features

# 测试特定 crate
cargo test -p mofa-sdk

# 用特定功能测试
cargo test -p mofa-sdk --features openai
```

## Pull Request 流程

1. 对于重大更改，**先创建 issue**
2. **进行更改**，遵循上述指南
3. **更新文档**（如需要）
4. **运行所有检查**:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-features
```

5. **提交 PR**，附带清晰的描述

### PR 检查清单

- [ ] 代码编译无警告
- [ ] 测试通过
- [ ] 文档已更新
- [ ] 遵循 CLAUDE.md 架构规则
- [ ] 提交消息遵循约定

## 文档

- 更新 `docs/` 中的相关 `.md` 文件
- 为公共 API 添加内联文档
- 更新 `CHANGELOG.md` 记录重要更改

## 有问题？

- 提交 issue 报告 bug 或功能请求
- 加入 [Discord](https://discord.com/invite/hKJZzDMMm9) 参与讨论
- 查看 [GitHub Discussions](https://github.com/mofa-org/mofa/discussions)

## 许可证

通过贡献，您同意您的贡献将根据 Apache License 2.0 许可。
