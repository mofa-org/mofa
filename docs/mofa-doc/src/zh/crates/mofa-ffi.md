# mofa-ffi

多语言的外部函数接口绑定。

## 目的

`mofa-ffi` 提供:
- 用于 Python、Java、Go、Swift、Kotlin 的 UniFFI 绑定
- PyO3 原生 Python 绑定
- 跨语言类型转换

## 支持的语言

| 语言 | 方法 | 状态 |
|----------|--------|--------|
| Python | UniFFI / PyO3 | 稳定 |
| Java | UniFFI | 测试版 |
| Go | UniFFI | 测试版 |
| Swift | UniFFI | 测试版 |
| Kotlin | UniFFI | 测试版 |

## 用法

### 构建绑定

```bash
# 构建所有绑定
cargo build -p mofa-ffi --features uniffi

# 仅构建 Python
cargo build -p mofa-ffi --features python
```

### 生成绑定

```bash
# Python
cargo run -p mofa-ffi --features uniffi -- generate python

# Java
cargo run -p mofa-ffi --features uniffi -- generate java
```

## 功能标志

| 标志 | 描述 |
|------|-------------|
| `uniffi` | 启用 UniFFI 绑定 |
| `python` | 启用 PyO3 Python 绑定 |

## 另见

- [跨语言绑定](../ffi/README.md) — FFI 概述
- [Python 绑定](../ffi/python.md) — Python 用法
