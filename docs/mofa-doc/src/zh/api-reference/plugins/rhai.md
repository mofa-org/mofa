# Rhai 脚本

使用 Rhai 脚本语言的运行时插件。

## 概述

Rhai 是一种嵌入式脚本语言，支持:
- 热重载插件
- 动态业务逻辑
- 安全沙箱

## 基本脚本

```rhai
// plugins/greet.rhai

fn process(input) {
    let name = input["name"];
    `你好, ${name}!`
}

fn on_init() {
    print("问候插件已加载!");
}
```

## API 参考

### 内置函数

```rhai
// JSON
let data = json::parse(input);
let text = json::stringify(data);

// 字符串
let upper = text.to_upper_case();
let parts = text.split(",");
let trimmed = text.trim();

// 集合
let list = [];
list.push(item);
let first = list[0];
let len = list.len();

// 数学
let result = math::sqrt(16);
let rounded = math::round(3.7);

// 时间
let now = time::now();
let formatted = time::format(now, "%Y-%m-%d");
```

### HTTP（启用时）

```rhai
let response = http::get("https://api.example.com/data");
let json = json::parse(response.body);
```

## 加载脚本

```rust
use mofa_plugins::{RhaiPlugin, RhaiPluginManager};

let mut manager = RhaiPluginManager::new();

// 从文件加载
let plugin = RhaiPlugin::from_file("./plugins/my_plugin.rhai").await?;
manager.register(plugin).await?;

// 调用函数
let result = manager.call("process", json!({"name": "World"})).await?;
```

## 热重载

```rust
use mofa_plugins::HotReloadWatcher;

let watcher = HotReloadWatcher::new("./plugins/")?;

watcher.on_change(|path| async move {
    println!("正在重载: {:?}", path);
    manager.reload(&path).await?;
    Ok(())
});
```

## 另见

- [插件概念](../../concepts/plugins.md) — 插件架构
