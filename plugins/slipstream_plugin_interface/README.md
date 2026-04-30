# Aleo Slipstream Plugin Interface

This crate enables a plugin to be added into a SnarkVM runtime to take actions at the time of
mapping updates at block finalization; for example, saving historical mapping state and staking
data to an external database. The plugin must implement the `SlipstreamPlugin` trait. See
`slipstream_plugin_interface.rs` for the full interface definition.

> **Feature flag:** compile with `--features slipstream-plugins` to enable plugin support.
> Plugin callbacks fire only during **canonical finalize** — speculative and dry-run executions
> are never observed by plugins.

# Components

### `plugins/slipstream_plugin_interface`
Defines the `SlipstreamPlugin` trait — the interface all plugins must implement.

| Method | Description |
|---|---|
| `on_load` / `on_unload` | Lifecycle hooks called on startup and shutdown |
| `subscribed_events` | Returns the event types a plugin subscribes to. Defaults to `&[]` — a plugin that does not override this method receives **no callbacks**. |
| `on_broadcast` | Called once per key-value update (and once per entry in a `replace_mapping` batch). Only fires for event kinds in the subscribed list. |

### `plugins/slipstream_plugin_manager`
Manages loaded plugins and their backing `libloading::Library` handles.

- **`LoadedSlipstreamPlugin`** — wrapper holding a boxed plugin + its name; implements `Deref`/`DerefMut`
- **`SlipstreamPluginManager`**
  - `from_config_files` — takes a slice of config file paths and loads one plugin per file
  - `load_plugin(path)` / `unload_plugin(name)` — load or unload a single plugin at runtime
  - `unload()` — fires `on_unload()` on every plugin then drops the libraries; field declaration order guarantees all plugin code finishes executing before the backing `.so` is unmapped
  - `has_subscribers()` — aggregate opt-in check; used internally to skip serialization when no plugin is interested in an event kind
  - `broadcast()` — fan-out broadcast to all interested plugins
  - `list_plugins()` — returns the names of all loaded plugins

---

## Plugin Config File (JSON5)

Each plugin requires a config file:
```json5
{
  "libpath": "/path/to/libmy_plugin.so",  // required; relative paths resolve from the config file's dir
  "name": "my_plugin"                      // optional; overrides the plugin's name() return value
}
```

---

## Plugin Library Convention

The shared library (`.so` / `.dylib` / `.dll`) must export a C function:
```rust
#[no_mangle]
pub extern "C" fn _create_plugin() -> *mut dyn SlipstreamPlugin {
    Box::into_raw(Box::new(MyPlugin::new()))
}
```

---

## Broadcast Event Format

All byte-slice fields in `BroadcastEvent` are serialized in **little-endian** format (via
`to_bytes_le()`). Plugin implementations must deserialize accordingly.

---

## Startup

`SlipstreamPluginManager::from_config_files()` takes a slice of config file paths and returns a
manager object. Install it into the `FinalizeStore` before the node begins processing blocks:

```rust
let manager = SlipstreamPluginManager::from_config_files(&[
    PathBuf::from("/etc/aleo/plugins/my_plugin.json5"),
])?;
finalize_store.set_slipstream_plugin_manager(manager);
```

## Shutdown

Call `manager.unload()` during graceful shutdown before aborting tasks. This fires `on_unload()`
on every plugin — the right place for flushing buffers, closing connections, etc.:

```rust
if let Some(manager) = finalize_store.slipstream_plugin_manager().write().as_mut() {
    manager.unload();
}
```

> Errors from plugin callbacks (`on_broadcast`) are logged as warnings and never propagated — a misbehaving plugin will not crash the node.
