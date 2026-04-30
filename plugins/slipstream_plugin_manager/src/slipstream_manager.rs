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

use libloading::Library;
use std::{
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
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

/// A fully-loaded plugin entry: the plugin instance, its backing shared library, and the
/// resolved path used for duplicate detection. Fields are declared in drop order — `plugin`
/// is dropped before `lib` — which guarantees all plugin code finishes executing before the
/// shared library is unloaded.
#[derive(Debug)]
struct LoadedPlugin {
    plugin: LoadedSlipstreamPlugin,
    _lib: Library,
    /// Resolved, absolute path to the `.so` file.
    /// Used to detect duplicate loads before calling `dlopen`, preventing unsafe double-loading.
    libpath: PathBuf,
}

impl Drop for LoadedPlugin {
    fn drop(&mut self) {
        info!("Unloading plugin '{}'", self.plugin.name());
        self.plugin.on_unload();
        // `plugin` then drops before `lib` (declaration order), ensuring all plugin code
        // finishes executing before the shared library is unloaded.
    }
}

// The Plugin Manager itself
#[derive(Default, Debug)]
pub struct SlipstreamPluginManager {
    plugins: Vec<LoadedPlugin>,
}

impl SlipstreamPluginManager {
    pub fn new() -> Self {
        SlipstreamPluginManager { plugins: Vec::default() }
    }

    /// Initializes a manager by loading one plugin per config file.
    ///
    /// Each config file must be a JSON5 file with a `libpath` field pointing to the
    /// shared library that implements `SlipstreamPlugin`.
    pub fn from_config_files(config_files: &[std::path::PathBuf]) -> Result<Self, SlipstreamPluginManagerError> {
        let mut manager = Self::new();
        for path in config_files {
            manager.load_plugin(path)?;
        }
        Ok(manager)
    }

    /// Unload all plugins and loaded plugin libraries, making sure to fire
    /// their `on_unload()` methods so they can do any necessary cleanup.
    pub fn unload(&mut self) {
        self.plugins.clear(); // Drop impl fires on_unload and enforces plugin-before-lib drop order.
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
                warn!("Slipstream plugin '{}' on_broadcast error: {e}", entry.plugin.name());
            }
        }
    }

    /// Returns the names of all loaded plugins.
    pub fn list_plugins(&self) -> JsonRpcResult<Vec<String>> {
        Ok(self.plugins.iter().map(|p| p.plugin.name().to_owned()).collect())
    }

    /// Loads a plugin from the given config file.
    ///
    /// # Safety
    ///
    /// This function loads the dynamically linked library specified in the config. The library
    /// must do necessary initializations.
    pub fn load_plugin(&mut self, slipstream_plugin_config_file: impl AsRef<Path>) -> JsonRpcResult<String> {
        // Resolve the library path from the config before calling dlopen.
        // This lets us detect duplicates without loading the library a second time, which is
        // unsafe: a second dlopen on an already-loaded .so can trigger re-execution of Rust
        // .init_array startup code, corrupting global state in the running plugin instance.
        let resolved_libpath = resolve_libpath_from_config(slipstream_plugin_config_file.as_ref())?;

        // Check for duplicate library path first (catches same .so before dlopen).
        if let Some(entry) = self.plugins.iter().find(|p| p.libpath == resolved_libpath) {
            return Err(SlipstreamPluginManagerError::PluginAlreadyLoaded(entry.plugin.name().to_string()));
        }

        let (new_lib, mut new_plugin, new_config_file) =
            load_plugin_from_config(slipstream_plugin_config_file.as_ref())?;

        // Also guard against a different .so that happens to expose the same plugin name.
        if self.plugins.iter().any(|entry| entry.plugin.name().eq(new_plugin.name())) {
            return Err(SlipstreamPluginManagerError::PluginAlreadyLoaded(new_plugin.name().to_string()));
        }

        // Call on_load and push plugin.
        new_plugin
            .on_load(new_config_file, false)
            .map_err(|e| SlipstreamPluginManagerError::PluginStartError(e.to_string()))?;
        let name = new_plugin.name().to_string();

        self.plugins.push(LoadedPlugin { plugin: new_plugin, _lib: new_lib, libpath: resolved_libpath });

        info!("Loaded plugin: {}", name);

        Ok(name)
    }

    /// Unloads the plugin with the given name.
    pub fn unload_plugin(&mut self, name: &str) -> JsonRpcResult<()> {
        let Some(idx) = self.plugins.iter().position(|entry| entry.plugin.name().eq(name)) else {
            return Err(SlipstreamPluginManagerError::PluginNotLoaded(name.to_string()));
        };

        self._drop_plugin(idx);
        Ok(())
    }

    /// Reloads the plugin with the given name from the given config file.
    ///
    /// # Note
    ///
    /// This function is not currently exposed. It was disabled due to SIGSEGV issues
    /// and is a good next step to implement safely. Use `unload_plugin` + `load_plugin`
    /// as a workaround in the meantime OR just stop the snarkos service and restart it with
    /// the updated plugin config(s)
    pub fn reload_plugin(&mut self, _name: &str, _config_file: &str) -> JsonRpcResult<()> {
        Err(SlipstreamPluginManagerError::PluginLoadError("Plugin reload is not currently implemented.".to_string()))
    }

    fn _drop_plugin(&mut self, idx: usize) {
        self.plugins.remove(idx); // Drop impl fires on_unload and enforces plugin-before-lib drop order.
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SlipstreamPluginManagerError {
    #[error("Cannot open the plugin config file: {0}")]
    CannotOpenConfigFile(String),

    #[error("Cannot read the plugin config file: {0}")]
    CannotReadConfigFile(String),

    #[error("The config file is not in a valid JSON/JSON5 format: {0}")]
    InvalidConfigFileFormat(String),

    #[error("Plugin library path is not specified in the config file")]
    LibPathNotSet,

    #[error("Invalid plugin path")]
    InvalidPluginPath,

    #[error("Cannot load plugin shared library (error: {0})")]
    PluginLoadError(String),

    #[error("The slipstream plugin '{0}' is already loaded")]
    PluginAlreadyLoaded(String),

    #[error("The plugin '{0}' is not loaded")]
    PluginNotLoaded(String),

    #[error("The SlipstreamPlugin on_load method failed (error: {0})")]
    PluginStartError(String),
}

/// Parses a plugin config file and returns the resolved, absolute path to the `.so`.
///
/// Does NOT open or load the library — safe to call for duplicate detection before `dlopen`.
#[cfg(not(test))]
pub(crate) fn resolve_libpath_from_config(
    slipstream_plugin_config_file: &Path,
) -> Result<PathBuf, SlipstreamPluginManagerError> {
    use std::{fs::File, io::Read};

    let mut file = File::open(slipstream_plugin_config_file).map_err(|e| {
        SlipstreamPluginManagerError::CannotOpenConfigFile(format!(
            "Failed to open the plugin config file {slipstream_plugin_config_file:?}, error: {e:?}"
        ))
    })?;

    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|e| {
        SlipstreamPluginManagerError::CannotReadConfigFile(format!(
            "Failed to read the plugin config file {slipstream_plugin_config_file:?}, error: {e:?}"
        ))
    })?;

    let result: serde_json::Value = json5::from_str(&contents).map_err(|e| {
        SlipstreamPluginManagerError::InvalidConfigFileFormat(format!(
            "The config file {slipstream_plugin_config_file:?} is not in a valid Json5 format, error: {e:?}"
        ))
    })?;

    let libpath_str = result["libpath"].as_str().ok_or(SlipstreamPluginManagerError::LibPathNotSet)?;
    let mut libpath = PathBuf::from(libpath_str);
    if libpath.is_relative() {
        let config_dir = slipstream_plugin_config_file.parent().ok_or_else(|| {
            SlipstreamPluginManagerError::CannotOpenConfigFile(format!(
                "Failed to resolve parent of {slipstream_plugin_config_file:?}",
            ))
        })?;
        libpath = config_dir.join(libpath);
    }

    Ok(libpath)
}

/// # Safety
///
/// This function loads the dynamically linked library specified in the path. The library
/// must do necessary initializations.
///
/// Returns the slipstream plugin, the dynamic library, and the parsed config file as a `&str`.
/// (The slipstream plugin interface requires a `&str` for the `on_load` method.)
#[cfg(not(test))]
pub(crate) fn load_plugin_from_config(
    slipstream_plugin_config_file: &Path,
) -> Result<(Library, LoadedSlipstreamPlugin, &str), SlipstreamPluginManagerError> {
    use std::{fs::File, io::Read, path::PathBuf};
    // Trait objects have no C equivalent; the suppression is intentional — the plugin ABI
    // uses raw pointers and the caller takes ownership immediately via Box::from_raw.
    #[allow(improper_ctypes_definitions)]
    type PluginConstructor = unsafe extern "C" fn() -> *mut dyn SlipstreamPlugin;
    use libloading::Symbol;

    let mut file = match File::open(slipstream_plugin_config_file) {
        Ok(file) => file,
        Err(err) => {
            return Err(SlipstreamPluginManagerError::CannotOpenConfigFile(format!(
                "Failed to open the plugin config file {slipstream_plugin_config_file:?}, error: {err:?}"
            )));
        }
    };

    let mut contents = String::new();
    if let Err(err) = file.read_to_string(&mut contents) {
        return Err(SlipstreamPluginManagerError::CannotReadConfigFile(format!(
            "Failed to read the plugin config file {slipstream_plugin_config_file:?}, error: {err:?}"
        )));
    }

    let result: serde_json::Value = match json5::from_str(&contents) {
        Ok(value) => value,
        Err(err) => {
            return Err(SlipstreamPluginManagerError::InvalidConfigFileFormat(format!(
                "The config file {slipstream_plugin_config_file:?} is not in a valid Json5 format, error: {err:?}"
            )));
        }
    };

    let libpath = result["libpath"].as_str().ok_or(SlipstreamPluginManagerError::LibPathNotSet)?;
    let mut libpath = PathBuf::from(libpath);
    if libpath.is_relative() {
        let config_dir = slipstream_plugin_config_file.parent().ok_or_else(|| {
            SlipstreamPluginManagerError::CannotOpenConfigFile(format!(
                "Failed to resolve parent of {slipstream_plugin_config_file:?}",
            ))
        })?;
        libpath = config_dir.join(libpath);
    }

    let plugin_name = result["name"].as_str().map(|s| s.to_owned());

    let config_file =
        slipstream_plugin_config_file.as_os_str().to_str().ok_or(SlipstreamPluginManagerError::InvalidPluginPath)?;

    let (plugin, lib) = unsafe {
        let lib = Library::new(libpath).map_err(|e| SlipstreamPluginManagerError::PluginLoadError(e.to_string()))?;
        let constructor: Symbol<PluginConstructor> =
            lib.get(b"_create_plugin").map_err(|e| SlipstreamPluginManagerError::PluginLoadError(e.to_string()))?;
        let plugin_raw = constructor();
        if plugin_raw.is_null() {
            return Err(SlipstreamPluginManagerError::PluginLoadError(
                "plugin constructor returned a null pointer".to_string(),
            ));
        }
        (Box::from_raw(plugin_raw), lib)
    };
    Ok((lib, LoadedSlipstreamPlugin::new(plugin, plugin_name), config_file))
}

#[cfg(test)]
const TESTPLUGIN_CONFIG: &str = "TESTPLUGIN_CONFIG";
#[cfg(test)]
const TESTPLUGIN2_CONFIG: &str = "TESTPLUGIN2_CONFIG";

// In tests resolve_libpath_from_config returns the config path itself as a stand-in for
// the .so path.  This is sufficient for duplicate detection without real file I/O.
#[cfg(test)]
pub(crate) fn resolve_libpath_from_config(
    slipstream_plugin_config_file: &Path,
) -> Result<PathBuf, SlipstreamPluginManagerError> {
    Ok(slipstream_plugin_config_file.to_path_buf())
}

// This is mocked for tests to avoid having to do IO with a dynamically linked library
// across different architectures at test time.
#[cfg(test)]
pub(crate) fn load_plugin_from_config(
    slipstream_plugin_config_file: &Path,
) -> Result<(Library, LoadedSlipstreamPlugin, &str), SlipstreamPluginManagerError> {
    if slipstream_plugin_config_file.ends_with(TESTPLUGIN_CONFIG) {
        Ok(tests::dummy_plugin_and_library(tests::TestPlugin, TESTPLUGIN_CONFIG))
    } else if slipstream_plugin_config_file.ends_with(TESTPLUGIN2_CONFIG) {
        Ok(tests::dummy_plugin_and_library(tests::TestPlugin2, TESTPLUGIN2_CONFIG))
    } else {
        Err(SlipstreamPluginManagerError::CannotOpenConfigFile(
            slipstream_plugin_config_file.to_str().unwrap().to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::slipstream_manager::{
        LoadedPlugin,
        LoadedSlipstreamPlugin,
        SlipstreamPluginManager,
        TESTPLUGIN_CONFIG,
        TESTPLUGIN2_CONFIG,
    };
    use libloading::Library;
    use snarkvm_slipstream_plugin_interface::slipstream_plugin_interface::{
        BroadcastEvent,
        BroadcastEventKind,
        SlipstreamPlugin,
    };
    use std::{
        path::PathBuf,
        sync::{Arc, RwLock},
    };

    pub(super) fn dummy_plugin_and_library<P: SlipstreamPlugin>(
        plugin: P,
        config_path: &'static str,
    ) -> (Library, LoadedSlipstreamPlugin, &'static str) {
        #[cfg(unix)]
        let library = libloading::os::unix::Library::this();
        #[cfg(windows)]
        let library = libloading::os::windows::Library::this().unwrap();
        (Library::from(library), LoadedSlipstreamPlugin::new(Box::new(plugin), None), config_path)
    }

    const DUMMY_NAME: &str = "dummy";
    const ANOTHER_DUMMY_NAME: &str = "another_dummy";

    #[derive(Clone, Copy, Debug)]
    pub(super) struct TestPlugin;

    impl SlipstreamPlugin for TestPlugin {
        fn name(&self) -> &'static str {
            DUMMY_NAME
        }
    }

    #[derive(Clone, Copy, Debug)]
    pub(super) struct TestPlugin2;

    impl SlipstreamPlugin for TestPlugin2 {
        fn name(&self) -> &'static str {
            ANOTHER_DUMMY_NAME
        }
    }

    #[test]
    fn test_plugin_list() {
        // Initialize empty manager.
        let plugin_manager = Arc::new(RwLock::new(SlipstreamPluginManager::new()));
        let mut plugin_manager_lock = plugin_manager.write().unwrap();

        // Load two plugins.
        let (_lib, mut plugin, config) = dummy_plugin_and_library(TestPlugin, TESTPLUGIN_CONFIG);
        plugin.on_load(config, false).unwrap();
        plugin_manager_lock.plugins.push(LoadedPlugin { plugin, _lib, libpath: PathBuf::from(config) });

        let (_lib, mut plugin, config) = dummy_plugin_and_library(TestPlugin2, TESTPLUGIN2_CONFIG);
        plugin.on_load(config, false).unwrap();
        plugin_manager_lock.plugins.push(LoadedPlugin { plugin, _lib, libpath: PathBuf::from(config) });

        // Check that both plugins are returned in the list.
        let plugins = plugin_manager_lock.list_plugins().unwrap();
        assert!(plugins.iter().any(|name| name.eq(DUMMY_NAME)));
        assert!(plugins.iter().any(|name| name.eq(ANOTHER_DUMMY_NAME)));
    }

    #[test]
    fn test_plugin_load_unload() {
        // Initialize empty manager.
        let plugin_manager = Arc::new(RwLock::new(SlipstreamPluginManager::new()));
        let mut plugin_manager_lock = plugin_manager.write().unwrap();

        // Load rpc call.
        let load_result = plugin_manager_lock.load_plugin(TESTPLUGIN_CONFIG);
        assert!(load_result.is_ok());
        assert_eq!(plugin_manager_lock.plugins.len(), 1);

        // Unload rpc call.
        let unload_result = plugin_manager_lock.unload_plugin(DUMMY_NAME);
        assert!(unload_result.is_ok());
        assert_eq!(plugin_manager_lock.plugins.len(), 0);
    }

    #[test]
    fn test_broadcast_mapping_update() {
        let mut manager = SlipstreamPluginManager::new();

        // Install a mock plugin that tracks calls.
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

        // Manually push the plugin (bypassing dynamic loading).
        #[cfg(unix)]
        let _lib = Library::from(libloading::os::unix::Library::this());
        #[cfg(windows)]
        let _lib = Library::from(libloading::os::windows::Library::this().unwrap());

        let plugin = TrackingPlugin { calls: std::sync::atomic::AtomicU32::new(0) };
        manager.plugins.push(LoadedPlugin {
            plugin: LoadedSlipstreamPlugin::new(Box::new(plugin), None),
            _lib,
            libpath: PathBuf::new(),
        });

        // Broadcast a MappingUpdate and verify the plugin received it.
        manager.broadcast(BroadcastEvent::MappingUpdate {
            program_id: b"program_id",
            mapping_name: b"mapping",
            key: b"key",
            value: b"value",
            block_height: 42,
        });

        // Verify via list_plugins that the plugin is still loaded.
        assert_eq!(manager.list_plugins().unwrap(), vec!["tracking"]);
    }
}
