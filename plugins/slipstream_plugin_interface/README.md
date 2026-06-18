# Aleo Slipstream Plugin Interface

Slipstream is a plugin system that streams canonical mapping updates and staking rewards from
snarkVM's finalize stage to external services (databases, metrics pipelines, etc.) in real time,
without modifying node code.

> **Feature flag:** compile with `--features slipstream-plugins` to enable plugin support.
> Plugin callbacks fire only during **canonical finalize** — speculative and dry-run executions
> are never observed by plugins.

---

## How plugins are loaded

Plugins are **statically linked** into the `snarkos` binary at compile time. There is no dynamic
`.so`/`.dylib` loading and no C FFI boundary. Each plugin crate calls `inventory::submit!` at link
time to register a `PluginRegistration` factory. snarkOS iterates
`inventory::iter::<PluginRegistration>()` at startup, matches each config file's `name` field
against the registered factories, and calls `register()` on the manager for each match.

---

## Components

### `plugins/slipstream_plugin_interface`

Defines the `SlipstreamPlugin` trait and the `PluginRegistration` factory type.

**`SlipstreamPlugin` trait:**

| Method | Description |
|---|---|
| `name` | Returns the plugin's static name string. Must match the `name` field in the config file. |
| `on_load(config_file, is_reload)` | Called once at startup. Reads config, connects to external services, verifies schema, replays any WAL. Node aborts if this returns `Err`. |
| `on_unload` | Called on graceful shutdown. Flush buffers, close connections. |
| `subscribed_events` | Returns the `BroadcastEventKind`s this plugin wants to receive. Defaults to `&[]` — a plugin that does not override this receives **no callbacks** and pays no serialization cost. |
| `on_broadcast(event)` | Called inline on the finalize thread for each subscribed event. Errors are logged as warnings and never propagated to consensus. |

**`PluginRegistration`:**

```rust
pub struct PluginRegistration {
    pub name: &'static str,
    pub factory: fn() -> Box<dyn SlipstreamPlugin>,
}
inventory::collect!(PluginRegistration);
```

Plugin crates register themselves at link time:

```rust
inventory::submit! {
    PluginRegistration {
        name: "my-plugin",
        factory: || Box::new(MyPlugin::new()),
    }
}
```

---

### `plugins/slipstream_plugin_manager`

Owns all active plugins and drives their lifecycle.

**`LoadedSlipstreamPlugin`** — wrapper holding a boxed plugin and its name. `Drop` calls
`on_unload()` automatically, so removing a plugin from `self.plugins` always triggers cleanup.

**`SlipstreamPluginManager`:**

| Method | Description |
|---|---|
| `register(plugin, config_file)` | Calls `on_load`, then adds the plugin to the active list. Returns `Err` if a plugin with the same name is already loaded or `on_load` fails. |
| `unload()` | Clears `self.plugins`; `Drop` on each entry fires `on_unload()`. |
| `unload_plugin(name)` | Removes and drops a single plugin by name. |
| `has_subscribers(kind)` | Returns `true` if any plugin subscribes to the given event kind. Used as a pre-serialization guard to skip byte serialization when no plugin would receive the event. |
| `broadcast(event)` | Fan-out: calls `on_broadcast` on every plugin subscribed to the event's kind. Errors are logged as warnings, never propagated. |
| `list_plugins()` | Returns the names of all active plugins. |

---

## Plugin Config File (JSON5)

Each plugin requires a JSON5 config file passed to `snarkos start --slipstream-plugins`:

```json5
{
  // Required: must match the name registered via inventory::submit! in the plugin crate.
  name: "my-plugin",

  // Plugin-specific fields — read by the plugin's own on_load implementation.
  connection_string: "postgres://user:pass@localhost/aleo",
  batch_size: 100,
}
```

---

## Startup

snarkOS reads each config file, looks up the matching `PluginRegistration` factory via
`inventory::iter`, and calls `register()`:

```rust
let plugin = factory();
manager.register(plugin, config_path)?;
```

`on_load` is called synchronously on the startup thread before the consensus engine starts. If
`on_load` returns `Err` for any plugin, the node exits immediately — there is no fallback.

---

## Shutdown

`manager.unload()` fires `on_unload()` on every plugin in reverse registration order:

```rust
if let Some(manager) = finalize_store.slipstream_plugin_manager().write().as_mut() {
    manager.unload();
}
```

---

## Broadcast Event Format

All `&[u8]` fields in `BroadcastEvent` carry **little-endian** byte representations of the
corresponding snarkVM console types (serialized via `ToBytes`). Plugin implementations must
deserialize accordingly.

```rust
pub enum BroadcastEvent<'a> {
    MappingUpdate { program_id: &'a [u8], mapping_name: &'a [u8], key: &'a [u8], value: &'a [u8], block_height: u32 },
    StakingReward  { staker: &'a [u8], validator: &'a [u8], reward: u64, new_stake: u64, block_height: u32 },
}
```

`BroadcastEvent` is `Copy`, so the same value can be fanned out to multiple plugins without cloning.

---

## Writing a Plugin

1. Add dependencies:

```toml
[dependencies]
snarkvm-slipstream-plugin-interface = { git = "https://github.com/ProvableHQ/snarkVM.git" }
inventory = "0.3"
```

2. Implement the trait and self-register:

```rust
use snarkvm_slipstream_plugin_interface::slipstream_plugin_interface::{
    SlipstreamPlugin, PluginRegistration,
};

#[derive(Debug)]
struct MyPlugin;

impl SlipstreamPlugin for MyPlugin {
    fn name(&self) -> &'static str { "my-plugin" }
    // override on_load, on_broadcast, on_unload as needed
}

inventory::submit! {
    PluginRegistration {
        name: "my-plugin",
        factory: || Box::new(MyPlugin::new()),
    }
}
```

3. Add the plugin crate as an optional dependency under the `slipstream-plugins` feature in
   `snarkOS/node/Cargo.toml`:

```toml
[dependencies.my-plugin]
path = "../../my-plugin"
optional = true

[features]
slipstream-plugins = [
    "snarkvm/slipstream-plugins",
    "dep:my-plugin",
]
```

See `slipstream-plugin-postgres` for a complete reference implementation.
