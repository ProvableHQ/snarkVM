# Aleo Slipstream Plugin Interface

This crate enables a plugin to be added into a SnarkVM runtime to observe canonical state.
Plugins can record mapping updates, staking rewards, and committed blocks. The plugin must
implement the `SlipstreamPlugin` trait. See `slipstream_plugin_interface.rs` for the full
interface definition.

Streaming is off until the node calls `FinalizeStore::set_slipstream(true)`. Mapping updates
and staking rewards are emitted during canonical finalize. A block is emitted after that block
is committed. Speculative and dry-run execution does not emit events.

# Components

### `plugins/slipstream_plugin_interface`
Defines the `SlipstreamPlugin` trait — the interface all plugins must implement.

| Method | Description |
|---|---|
| `on_load` / `on_unload` | Lifecycle hooks called on startup and shutdown |
| `subscribed_events` | Returns the event types a plugin subscribes to. Defaults to `&[]` — a plugin that does not override this method receives **no callbacks**. |
| `on_broadcast` | Called once per event whose kind is in the subscribed list. |

| Event | When it fires |
|---|---|
| `MappingUpdate` | A key-value pair is written with `update_key_value` or `replace_mapping` during canonical finalize. |
| `MappingRemoval` | A key is removed with `remove_key_value`, a key is absent from a `replace_mapping`, or `remove_mapping` removes the mapping. `key` is absent when the whole mapping is removed. Removals for a replacement are emitted before that replacement's updates. |
| `StakingReward` | A staker's reward is applied during canonical finalize. |
| `Block` | A block has been committed. `block` is the little-endian encoding of the block. |

### `plugins/slipstream_plugin_manager`
Manages loaded plugins and their backing `libloading::Library` handles.

- **`LoadedSlipstreamPlugin`** — wrapper holding a boxed plugin + its name; implements `Deref`/`DerefMut`
- **`SlipstreamPluginManager`**
  - `from_config_files` — takes a slice of config file paths and loads one plugin per file
  - `install` — registers an in-process plugin
  - `load_plugin(path)` / `unload_plugin(name)` — load or unload a single plugin at runtime
  - `unload()` — fires `on_unload()` on every plugin then drops the libraries; field declaration order guarantees all plugin code finishes executing before the backing `.so` is unmapped
  - `has_subscribers()` — aggregate opt-in check; used internally to skip serialization when no plugin is interested in an event kind
  - `broadcast()` — queues the event and returns. Callbacks run on the `slipstream-broadcast` thread. A full queue drops that event
  - `flush()` — waits until every event queued before the call has been delivered
  - `list_plugins()` — returns the names of all loaded plugins

---

## Plugin Config File (JSON5)

Each dynamic plugin requires a config file:
```json5
{
  "libpath": "/path/to/libmy_plugin.so",  // required; relative paths resolve from the config file's dir, then the path is canonicalized
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
manager object. Install it into the `FinalizeStore` and enable streaming before the node begins
processing blocks:

```rust
let manager = SlipstreamPluginManager::from_config_files(&[
    PathBuf::from("/etc/aleo/plugins/my_plugin.json5"),
])?;
finalize_store.set_slipstream_plugin_manager(manager);
finalize_store.set_slipstream(true);
```

`set_slipstream(false)` stops further events. The manager stays installed.

## Shutdown

Call `manager.unload()` during graceful shutdown before aborting tasks. This fires `on_unload()`
on every plugin — the right place for flushing buffers, closing connections, etc.:

```rust
if let Some(manager) = finalize_store.slipstream_plugin_manager().write().as_mut() {
    manager.unload();
}
```

> Errors and panics from plugin callbacks (`on_broadcast`) are logged as warnings and never propagated — a misbehaving plugin will not crash the node or stop later events. A serialization failure skips that event and does not reject the block. Dropping the manager delivers events already queued, then unloads the plugin libraries.
