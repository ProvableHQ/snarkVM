// Copyright (c) 2019-2026 Provable Inc.
// This file is part of the snarkVM library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:

// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use snarkvm_slipstream_plugin_interface::slipstream_plugin_interface::{
    BroadcastEvent,
    BroadcastEventKind,
    SlipstreamPlugin,
};

use std::{
    ops::{Deref, DerefMut},
    path::Path,
};
use tracing::{info, warn};

/// A type alias for the result of plugin manager operations.
type JsonRpcResult<T> = Result<T, SlipstreamPluginManagerError>;

#[derive(Debug)]
pub struct LoadedSlipstreamPlugin {
    name: String,
    plugin: Box<dyn SlipstreamPlugin>,
}

impl LoadedSlipstreamPlugin {
    pub fn new(plugin: Box<dyn SlipstreamPlugin>, name: Option<String>) -> Self {
        Self { name: name.unwrap_or_else(|| plugin.name().to_owned()), plugin }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Deref for LoadedSlipstreamPlugin {
    type Target = Box<dyn SlipstreamPlugin>;

    fn deref(&self) -> &Self::Target {
        &self.plugin
    }
}

impl DerefMut for LoadedSlipstreamPlugin {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.plugin
    }
}

impl Drop for LoadedSlipstreamPlugin {
    fn drop(&mut self) {
        info!("Unloading plugin '{}'", self.name);
        self.plugin.on_unload();
    }
}

// The Plugin Manager itself
#[derive(Default, Debug)]
pub struct SlipstreamPluginManager {
    plugins: Vec<LoadedSlipstreamPlugin>,
}

impl SlipstreamPluginManager {
    pub fn new() -> Self {
        SlipstreamPluginManager { plugins: Vec::default() }
    }

    /// Unload all plugins, firing their `on_unload()` methods.
    pub fn unload(&mut self) {
        self.plugins.clear(); // Drop impl fires on_unload for each plugin.
    }

    /// Returns `true` if any loaded plugin subscribes to the given event kind.
    ///
    /// Used as a pre-serialization guard: callers skip expensive byte serialization
    /// when no plugin would receive the resulting event.
    pub fn has_subscribers(&self, kind: BroadcastEventKind) -> bool {
        self.plugins.iter().any(|p| p.plugin.subscribed_events().contains(&kind))
    }

    /// Dispatches an event to every plugin subscribed to its kind.
    /// Errors are logged as warnings but never propagated.
    pub fn broadcast(&self, event: BroadcastEvent<'_>) {
        let kind = event.kind();
        for entry in &self.plugins {
            if entry.plugin.subscribed_events().contains(&kind)
                && let Err(e) = entry.plugin.on_broadcast(event)
            {
                warn!("Slipstream plugin '{}' on_broadcast error: {e}", entry.name());
            }
        }
    }

    /// Returns the names of all loaded plugins.
    pub fn list_plugins(&self) -> JsonRpcResult<Vec<String>> {
        Ok(self.plugins.iter().map(|p| p.name().to_owned()).collect())
    }

    /// Registers a statically-linked plugin.
    ///
    /// Calls `plugin.on_load(config_file, false)` and adds the plugin to the active list.
    pub fn register(
        &mut self,
        mut plugin: Box<dyn SlipstreamPlugin>,
        config_file: impl AsRef<Path>,
    ) -> JsonRpcResult<String> {
        let name = plugin.name().to_string();

        if self.plugins.iter().any(|p| p.name() == name) {
            return Err(SlipstreamPluginManagerError::PluginAlreadyLoaded(name));
        }

        let config_file_str = config_file
            .as_ref()
            .to_str()
            .ok_or(SlipstreamPluginManagerError::InvalidPluginPath)?;

        plugin
            .on_load(config_file_str, false)
            .map_err(|e| SlipstreamPluginManagerError::PluginStartError(e.to_string()))?;

        self.plugins.push(LoadedSlipstreamPlugin::new(plugin, None));

        info!("Registered static plugin: {}", name);
        Ok(name)
    }

    /// Unloads the plugin with the given name.
    pub fn unload_plugin(&mut self, name: &str) -> JsonRpcResult<()> {
        let Some(idx) = self.plugins.iter().position(|entry| entry.name().eq(name)) else {
            return Err(SlipstreamPluginManagerError::PluginNotLoaded(name.to_string()));
        };

        self._drop_plugin(idx);
        Ok(())
    }

    /// Reload is not currently implemented.
    pub fn reload_plugin(&mut self, _name: &str, _config_file: &str) -> JsonRpcResult<()> {
        Err(SlipstreamPluginManagerError::PluginLoadError(
            "Plugin reload is not currently implemented.".to_string(),
        ))
    }

    fn _drop_plugin(&mut self, idx: usize) {
        self.plugins.remove(idx); // Drop impl fires on_unload.
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SlipstreamPluginManagerError {
    #[error("Invalid plugin path")]
    InvalidPluginPath,

    #[error("Cannot load plugin (error: {0})")]
    PluginLoadError(String),

    #[error("The slipstream plugin '{0}' is already loaded")]
    PluginAlreadyLoaded(String),

    #[error("The plugin '{0}' is not loaded")]
    PluginNotLoaded(String),

    #[error("The SlipstreamPlugin on_load method failed (error: {0})")]
    PluginStartError(String),
}

#[cfg(test)]
mod tests {
    use crate::slipstream_manager::{LoadedSlipstreamPlugin, SlipstreamPluginManager};
    use snarkvm_slipstream_plugin_interface::slipstream_plugin_interface::{
        BroadcastEvent,
        BroadcastEventKind,
        SlipstreamPlugin,
    };
    use std::sync::{Arc, RwLock};

    const DUMMY_NAME: &str = "dummy";
    const ANOTHER_DUMMY_NAME: &str = "another_dummy";

    #[derive(Clone, Copy, Debug)]
    struct TestPlugin;

    impl SlipstreamPlugin for TestPlugin {
        fn name(&self) -> &'static str {
            DUMMY_NAME
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct TestPlugin2;

    impl SlipstreamPlugin for TestPlugin2 {
        fn name(&self) -> &'static str {
            ANOTHER_DUMMY_NAME
        }
    }

    #[test]
    fn test_plugin_list() {
        let plugin_manager = Arc::new(RwLock::new(SlipstreamPluginManager::new()));
        let mut lock = plugin_manager.write().unwrap();

        lock.plugins.push(LoadedSlipstreamPlugin::new(Box::new(TestPlugin), None));
        lock.plugins.push(LoadedSlipstreamPlugin::new(Box::new(TestPlugin2), None));

        let plugins = lock.list_plugins().unwrap();
        assert!(plugins.iter().any(|name| name.eq(DUMMY_NAME)));
        assert!(plugins.iter().any(|name| name.eq(ANOTHER_DUMMY_NAME)));
    }

    #[test]
    fn test_plugin_register_unload() {
        let plugin_manager = Arc::new(RwLock::new(SlipstreamPluginManager::new()));
        let mut lock = plugin_manager.write().unwrap();

        let result = lock.register(Box::new(TestPlugin), "dummy_config");
        assert!(result.is_ok());
        assert_eq!(lock.plugins.len(), 1);

        let result = lock.unload_plugin(DUMMY_NAME);
        assert!(result.is_ok());
        assert_eq!(lock.plugins.len(), 0);
    }

    #[test]
    fn test_broadcast_mapping_update() {
        let mut manager = SlipstreamPluginManager::new();

        #[derive(Debug)]
        struct TrackingPlugin {
            calls: std::sync::atomic::AtomicU32,
        }
        impl SlipstreamPlugin for TrackingPlugin {
            fn name(&self) -> &'static str {
                "tracking"
            }

            fn subscribed_events(&self) -> &[BroadcastEventKind] {
                &[BroadcastEventKind::MappingUpdate]
            }

            fn on_broadcast(&self, _event: BroadcastEvent<'_>) -> anyhow::Result<()> {
                self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }
        }

        manager.plugins.push(LoadedSlipstreamPlugin::new(
            Box::new(TrackingPlugin { calls: std::sync::atomic::AtomicU32::new(0) }),
            None,
        ));

        manager.broadcast(BroadcastEvent::MappingUpdate {
            program_id: b"program_id",
            mapping_name: b"mapping",
            key: b"key",
            value: b"value",
            block_height: 42,
        });

        assert_eq!(manager.list_plugins().unwrap(), vec!["tracking"]);
    }
}
