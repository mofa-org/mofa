# Rhai Scripts

Runtime plugins using the Rhai scripting language.

## Overview

Rhai is an embedded scripting language that enables:
- Hot-reloadable plugins
- Dynamic business logic
- Safe sandboxing

## Basic Script

```rhai
// plugins/greet.rhai

fn process(input) {
    let name = input["name"];
    `Hello, ${name}!`
}

fn on_init() {
    print("Greeting plugin loaded!");
}
```

## API Reference

### Built-in Functions

```rhai
// JSON
let data = json::parse(input);
let text = json::stringify(data);

// String
let upper = text.to_upper_case();
let parts = text.split(",");
let trimmed = text.trim();

// Collections
let list = [];
list.push(item);
let first = list[0];
let len = list.len();

// Math
let result = math::sqrt(16);
let rounded = math::round(3.7);

// Time
let now = time::now();
let formatted = time::format(now, "%Y-%m-%d");
```

### HTTP (when enabled)

```rhai
let response = http::get("https://api.example.com/data");
let json = json::parse(response.body);
```

## Loading Scripts

```rust
use mofa_plugins::{RhaiPlugin, RhaiPluginManager};

let mut manager = RhaiPluginManager::new();

// Load from file
let plugin = RhaiPlugin::from_file("./plugins/my_plugin.rhai").await?;
manager.register(plugin).await?;

// Call function
let result = manager.call("process", json!({"name": "World"})).await?;
```

## Hot Reloading

```rust
use mofa_plugins::HotReloadWatcher;

let watcher = HotReloadWatcher::new("./plugins/")?;

watcher.on_change(|path| async move {
    println!("Reloading: {:?}", path);
    manager.reload(&path).await?;
    Ok(())
});
```

## See Also

- [Plugins Concept](../../concepts/plugins.md) — Plugin architecture
