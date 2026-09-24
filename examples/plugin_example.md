# StarForge Plugin Example

Build a plugin using the `starforge-plugin-sdk` crate.

## Setup

Create a new crate:

```bash
cargo new --lib my-starforge-plugin
cd my-starforge-plugin
```

Add to `Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
starforge-plugin-sdk = { path = "../crates/starforge-plugin-sdk" }
```

## Implement the plugin

```rust
use starforge_plugin_sdk::{export_plugin, PluginMeta, StarForgePlugin};

#[derive(Default)]
struct HelloPlugin;

impl StarForgePlugin for HelloPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta {
            name: "hello",
            version: "0.1.0",
            description: "Prints a greeting",
        }
    }

    fn run(&self, args: &[String]) -> Result<(), String> {
        let name = args.first().map(|s| s.as_str()).unwrap_or("world");
        println!("Hello, {}!", name);
        Ok(())
    }
}

export_plugin!(HelloPlugin);
```

## Build

```bash
cargo build --release
# Output: target/release/libmy_starforge_plugin.dylib (macOS)
#         target/release/libmy_starforge_plugin.so   (Linux)
```

## Load

Place the compiled `.so` / `.dylib` in `~/.starforge/plugins/` and StarForge will discover it on startup.
