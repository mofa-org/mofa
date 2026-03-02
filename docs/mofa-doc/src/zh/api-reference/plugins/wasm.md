# WASM 插件

使用 WebAssembly 的高性能插件。

## 概述

WASM 插件提供:
- 跨语言兼容性
- 沙箱执行
- 接近原生的性能

## 创建 WASM 插件

### 设置

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
```

### 实现

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn process(input: &str) -> String {
    // 您的实现
    format!("已处理: {}", input)
}

#[wasm_bindgen]
pub fn analyze(data: &[u8]) -> Vec<u8> {
    // 二进制数据处理
    data.to_vec()
}
```

### 构建

```bash
cargo build --target wasm32-unknown-unknown --release
```

## 加载 WASM 插件

```rust
use mofa_plugins::WasmPlugin;

let plugin = WasmPlugin::load("./plugins/my_plugin.wasm").await?;

// 调用导出函数
let result = plugin.call("process", b"input data").await?;
println!("结果: {}", String::from_utf8_lossy(&result));
```

## 安全性

WASM 插件在沙箱环境中运行:
- 无直接文件系统访问
- 无网络访问（除非明确授权）
- 内存隔离

## 另见

- [插件概念](../../concepts/plugins.md) — 插件架构
