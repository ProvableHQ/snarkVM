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
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::{
        Arc,
        RwLock,
        RwLockWriteGuard,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
};
use tracing::{info, warn};

/// Events that can wait for plugin callbacks.
///
/// `broadcast` drops the newest event when this many events are already queued.
const BROADCAST_QUEUE_CAPACITY: usize = 16_384;

/// Owned copy of a [`BroadcastEvent`], so the caller can return while a worker delivers it.
enum OwnedBroadcastEvent {
    MappingUpdate { program_id: Vec<u8>, mapping_name: Vec<u8>, key: Vec<u8>, value: Vec<u8>, block_height: u32 },
    MappingRemoval { program_id: Vec<u8>, mapping_name: Vec<u8>, key: Option<Vec<u8>>, block_height: u32 },
    StakingReward { staker: Vec<u8>, validator: Vec<u8>, reward: u64, new_stake: u64, block_height: u32 },
    Block { block: Vec<u8>, block_height: u32 },
}

impl From<BroadcastEvent<'_>> for OwnedBroadcastEvent {
    fn from(event: BroadcastEvent<'_>) -> Self {
        match event {
            BroadcastEvent::MappingUpdate { program_id, mapping_name, key, value, block_height } => {
                Self::MappingUpdate {
                    program_id: program_id.to_vec(),
                    mapping_name: mapping_name.to_vec(),
                    key: key.to_vec(),
                    value: value.to_vec(),
                    block_height,
                }
            }
            BroadcastEvent::MappingRemoval { program_id, mapping_name, key, block_height } => Self::MappingRemoval {
                program_id: program_id.to_vec(),
                mapping_name: mapping_name.to_vec(),
                key: key.map(|bytes| bytes.to_vec()),
                block_height,
            },
            BroadcastEvent::StakingReward { staker, validator, reward, new_stake, block_height } => {
                Self::StakingReward {
                    staker: staker.to_vec(),
                    validator: validator.to_vec(),
                    reward,
                    new_stake,
                    block_height,
                }
            }
            BroadcastEvent::Block { block, block_height } => Self::Block { block: block.to_vec(), block_height },
        }
    }
}

impl OwnedBroadcastEvent {
    fn kind(&self) -> BroadcastEventKind {
        match self {
            Self::MappingUpdate { .. } => BroadcastEventKind::MappingUpdate,
            Self::MappingRemoval { .. } => BroadcastEventKind::MappingRemoval,
            Self::StakingReward { .. } => BroadcastEventKind::StakingReward,
            Self::Block { .. } => BroadcastEventKind::Block,
        }
    }

    fn with_borrowed(&self, deliver: impl FnOnce(BroadcastEvent<'_>)) {
        match self {
            Self::MappingUpdate { program_id, mapping_name, key, value, block_height } => {
                deliver(BroadcastEvent::MappingUpdate {
                    program_id,
                    mapping_name,
                    key,
                    value,
                    block_height: *block_height,
                });
            }
            Self::MappingRemoval { program_id, mapping_name, key, block_height } => {
                deliver(BroadcastEvent::MappingRemoval {
                    program_id,
                    mapping_name,
                    key: key.as_deref(),
                    block_height: *block_height,
                });
            }
            Self::StakingReward { staker, validator, reward, new_stake, block_height } => {
                deliver(BroadcastEvent::StakingReward {
                    staker,
                    validator,
                    reward: *reward,
                    new_stake: *new_stake,
                    block_height: *block_height,
                });
            }
            Self::Block { block, block_height } => {
                deliver(BroadcastEvent::Block { block, block_height: *block_height });
            }
        }
    }
}

enum WorkerMessage {
    Event(OwnedBroadcastEvent),
    Flush(mpsc::Sender<()>),
    Shutdown,
}

/// Subscriber flags read by the finalizer without taking the plugin lock.
#[derive(Debug, Default)]
struct SubscriberFlags {
    mapping_update: AtomicBool,
    mapping_removal: AtomicBool,
    staking_reward: AtomicBool,
    block: AtomicBool,
}

fn plugin_subscribes(plugins: &[LoadedPlugin], kind: BroadcastEventKind) -> bool {
    plugins.iter().any(|entry| entry.plugin.subscribed_events().contains(&kind))
}

fn store_subscribers(flags: &SubscriberFlags, plugins: &[LoadedPlugin]) {
    flags.mapping_update.store(plugin_subscribes(plugins, BroadcastEventKind::MappingUpdate), Ordering::SeqCst);
    flags.mapping_removal.store(plugin_subscribes(plugins, BroadcastEventKind::MappingRemoval), Ordering::SeqCst);
    flags.staking_reward.store(plugin_subscribes(plugins, BroadcastEventKind::StakingReward), Ordering::SeqCst);
    flags.block.store(plugin_subscribes(plugins, BroadcastEventKind::Block), Ordering::SeqCst);
}

fn write_plugins(plugins: &Arc<RwLock<Vec<LoadedPlugin>>>) -> RwLockWriteGuard<'_, Vec<LoadedPlugin>> {
    plugins.write().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn broadcast_worker(plugins: Arc<RwLock<Vec<LoadedPlugin>>>, rx: mpsc::Receiver<WorkerMessage>) {
    while let Ok(message) = rx.recv() {
        match message {
            WorkerMessage::Event(event) => dispatch_event(&plugins, &event),
            WorkerMessage::Flush(ack) => {
                let _ = ack.send(());
            }
            WorkerMessage::Shutdown => break,
        }
    }
}

fn dispatch_event(plugins: &RwLock<Vec<LoadedPlugin>>, event: &OwnedBroadcastEvent) {
    let guard = plugins.read().unwrap_or_else(|poisoned| poisoned.into_inner());
    let kind = event.kind();
    event.with_borrowed(|borrowed| {
        for entry in guard.iter() {
            if !entry.plugin.subscribed_events().contains(&kind) {
                continue;
            }
            // A panic in one plugin stays on this thread. The finalizer keeps running, and this
            // worker continues with the remaining plugins and events.
            let result = catch_unwind(AssertUnwindSafe(|| entry.plugin.on_broadcast(borrowed)));
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => warn!("Slipstream plugin '{}' on_broadcast error: {error}", entry.plugin.name()),
                Err(_) => warn!("Slipstream plugin '{}' panicked in on_broadcast", entry.plugin.name()),
            }
        }
    });
}

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
    /// Canonical path of the loaded library.
    /// A second `dlopen` of a library that is already loaded can re-run its startup code.
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
#[derive(Debug)]
pub struct SlipstreamPluginManager {
    plugins: Arc<RwLock<Vec<LoadedPlugin>>>,
    subscribers: SubscriberFlags,
    tx: mpsc::SyncSender<WorkerMessage>,
    worker: Option<thread::JoinHandle<()>>,
    dropped: AtomicU64,
    worker_stopped: AtomicBool,
}

impl Default for SlipstreamPluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SlipstreamPluginManager {
    pub fn new() -> Self {
        let plugins = Arc::new(RwLock::new(Vec::new()));
        let (tx, rx) = mpsc::sync_channel(BROADCAST_QUEUE_CAPACITY);
        let plugins_for_worker = Arc::clone(&plugins);
        // `spawn` fails only when the OS cannot allocate a thread.
        let worker = thread::Builder::new()
            .name("slipstream-broadcast".to_owned())
            .spawn(move || broadcast_worker(plugins_for_worker, rx))
            .expect("OS refused to spawn the slipstream broadcast thread");
        Self {
            plugins,
            subscribers: SubscriberFlags::default(),
            tx,
            worker: Some(worker),
            dropped: AtomicU64::new(0),
            worker_stopped: AtomicBool::new(false),
        }
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
    ///
    /// Queued events are delivered before the plugins are dropped.
    pub fn unload(&mut self) {
        self.flush();
        let plugins = Arc::clone(&self.plugins);
        let mut guard = write_plugins(&plugins);
        guard.clear(); // Drop impl fires on_unload and enforces plugin-before-lib drop order.
        store_subscribers(&self.subscribers, &guard);
    }

    /// Registers an in-process plugin.
    ///
    /// `on_load` runs with an empty config path. The plugin name must be unique.
    pub fn install(&mut self, plugin: impl SlipstreamPlugin + 'static) -> JsonRpcResult<String> {
        let lib = in_process_library()?;
        let mut loaded = LoadedSlipstreamPlugin::new(Box::new(plugin), None);
        let plugins = Arc::clone(&self.plugins);
        let already_loaded = {
            let guard = write_plugins(&plugins);
            guard.iter().any(|entry| entry.plugin.name() == loaded.name())
        };
        if already_loaded {
            return Err(SlipstreamPluginManagerError::PluginAlreadyLoaded(loaded.name().to_string()));
        }
        loaded.on_load("", false).map_err(|error| SlipstreamPluginManagerError::PluginStartError(error.to_string()))?;
        let name = loaded.name().to_string();
        let mut guard = write_plugins(&plugins);
        guard.push(LoadedPlugin { plugin: loaded, _lib: lib, libpath: PathBuf::from(format!("in-process:{name}")) });
        store_subscribers(&self.subscribers, &guard);
        info!("Loaded plugin: {name}");
        Ok(name)
    }

    /// Returns `true` if any loaded plugin subscribes to the given event kind.
    ///
    /// Used as a pre-serialization guard: callers skip expensive byte serialization
    /// when no plugin would receive the resulting event.
    pub fn has_subscribers(&self, kind: BroadcastEventKind) -> bool {
        match kind {
            BroadcastEventKind::MappingUpdate => self.subscribers.mapping_update.load(Ordering::SeqCst),
            BroadcastEventKind::MappingRemoval => self.subscribers.mapping_removal.load(Ordering::SeqCst),
            BroadcastEventKind::StakingReward => self.subscribers.staking_reward.load(Ordering::SeqCst),
            BroadcastEventKind::Block => self.subscribers.block.load(Ordering::SeqCst),
        }
    }

    /// Queues an event for every plugin subscribed to its kind.
    ///
    /// The call returns once the queue accepts the event. Callbacks run on the slipstream
    /// broadcast thread. A full queue drops the event. Callback errors are logged and are not
    /// returned. A plugin callback must not call [`Self::flush`] or drop this manager: the worker
    /// would wait for itself.
    pub fn broadcast(&self, event: BroadcastEvent<'_>) {
        match self.tx.try_send(WorkerMessage::Event(OwnedBroadcastEvent::from(event))) {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(_)) => {
                let dropped = self.dropped.fetch_add(1, Ordering::Relaxed) + 1;
                if dropped.is_power_of_two() {
                    warn!("Slipstream broadcast queue is full; dropped {dropped} events");
                }
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                if !self.worker_stopped.swap(true, Ordering::Relaxed) {
                    warn!("Slipstream broadcast worker is stopped; dropping events");
                }
            }
        }
    }

    /// Blocks until every event queued before this call has been delivered.
    ///
    /// A plugin callback must not call this method: the worker would wait for itself.
    pub fn flush(&self) {
        let (ack_tx, ack_rx) = mpsc::channel();
        if self.tx.send(WorkerMessage::Flush(ack_tx)).is_ok() {
            let _ = ack_rx.recv();
        }
    }

    /// Returns the names of all loaded plugins.
    pub fn list_plugins(&self) -> JsonRpcResult<Vec<String>> {
        let plugins = Arc::clone(&self.plugins);
        let guard = plugins.read().unwrap_or_else(|poisoned| poisoned.into_inner());
        Ok(guard.iter().map(|p| p.plugin.name().to_owned()).collect())
    }

    /// Loads a plugin from the given config file.
    ///
    /// # Safety
    ///
    /// This function loads the dynamically linked library specified in the config. The library
    /// must do necessary initializations.
    pub fn load_plugin(&mut self, slipstream_plugin_config_file: impl AsRef<Path>) -> JsonRpcResult<String> {
        let config_file = slipstream_plugin_config_file.as_ref();
        // One read of the config. The loader opens `spec.libpath` from this read.
        // A second dlopen of a library that is already loaded can re-run its startup code.
        let spec = read_plugin_spec(config_file)?;

        let plugins = Arc::clone(&self.plugins);
        let duplicate = {
            let guard = write_plugins(&plugins);
            guard.iter().find(|entry| entry.libpath == spec.libpath).map(|entry| entry.plugin.name().to_string())
        };
        if let Some(name) = duplicate {
            return Err(SlipstreamPluginManagerError::PluginAlreadyLoaded(name));
        }

        let (new_lib, mut new_plugin) = open_plugin(&spec)?;

        // Also guard against a different library that exposes the same plugin name.
        let duplicate_name = {
            let guard = write_plugins(&plugins);
            guard.iter().any(|entry| entry.plugin.name().eq(new_plugin.name()))
        };
        if duplicate_name {
            return Err(SlipstreamPluginManagerError::PluginAlreadyLoaded(new_plugin.name().to_string()));
        }

        let config_file = config_file.as_os_str().to_str().ok_or(SlipstreamPluginManagerError::InvalidPluginPath)?;
        new_plugin
            .on_load(config_file, false)
            .map_err(|e| SlipstreamPluginManagerError::PluginStartError(e.to_string()))?;
        let name = new_plugin.name().to_string();

        let mut guard = write_plugins(&plugins);
        guard.push(LoadedPlugin { plugin: new_plugin, _lib: new_lib, libpath: spec.libpath });
        store_subscribers(&self.subscribers, &guard);

        info!("Loaded plugin: {}", name);

        Ok(name)
    }

    /// Unloads the plugin with the given name.
    pub fn unload_plugin(&mut self, name: &str) -> JsonRpcResult<()> {
        self.flush();
        let plugins = Arc::clone(&self.plugins);
        let mut guard = write_plugins(&plugins);
        let Some(idx) = guard.iter().position(|entry| entry.plugin.name().eq(name)) else {
            return Err(SlipstreamPluginManagerError::PluginNotLoaded(name.to_string()));
        };

        guard.remove(idx); // Drop impl fires on_unload and enforces plugin-before-lib drop order.
        store_subscribers(&self.subscribers, &guard);
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
}

impl Drop for SlipstreamPluginManager {
    fn drop(&mut self) {
        // Queued events run before `Shutdown`. Joining the worker finishes those callbacks before
        // the loaded libraries are dropped.
        let _ = self.tx.send(WorkerMessage::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
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

fn in_process_library() -> Result<Library, SlipstreamPluginManagerError> {
    #[cfg(unix)]
    {
        Ok(Library::from(libloading::os::unix::Library::this()))
    }
    #[cfg(windows)]
    {
        libloading::os::windows::Library::this()
            .map(Library::from)
            .map_err(|error| SlipstreamPluginManagerError::PluginLoadError(error.to_string()))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(SlipstreamPluginManagerError::PluginLoadError(
            "in-process plugins are unsupported on this platform".to_string(),
        ))
    }
}

/// Plugin identity taken from one read of a config file.
struct PluginSpec {
    /// Canonical path of the library named by `libpath`.
    libpath: PathBuf,
    name: Option<String>,
}

/// Reads a plugin config once and returns the canonical library path from that read.
fn read_plugin_spec(config_file: &Path) -> Result<PluginSpec, SlipstreamPluginManagerError> {
    use std::{fs::File, io::Read};

    let mut file = File::open(config_file).map_err(|error| {
        SlipstreamPluginManagerError::CannotOpenConfigFile(format!(
            "Failed to open the plugin config file {config_file:?}, error: {error:?}"
        ))
    })?;

    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|error| {
        SlipstreamPluginManagerError::CannotReadConfigFile(format!(
            "Failed to read the plugin config file {config_file:?}, error: {error:?}"
        ))
    })?;

    let result: serde_json::Value = json5::from_str(&contents).map_err(|error| {
        SlipstreamPluginManagerError::InvalidConfigFileFormat(format!(
            "The config file {config_file:?} is not in a valid Json5 format, error: {error:?}"
        ))
    })?;

    let libpath_str = result["libpath"].as_str().ok_or(SlipstreamPluginManagerError::LibPathNotSet)?;
    let mut libpath = PathBuf::from(libpath_str);
    if libpath.is_relative() {
        let config_dir = config_file.parent().ok_or_else(|| {
            SlipstreamPluginManagerError::CannotOpenConfigFile(format!("Failed to resolve parent of {config_file:?}"))
        })?;
        libpath = config_dir.join(libpath);
    }
    let libpath = std::fs::canonicalize(&libpath).map_err(|error| {
        SlipstreamPluginManagerError::PluginLoadError(format!(
            "Cannot resolve plugin library path {}: {error}",
            libpath.display()
        ))
    })?;

    Ok(PluginSpec { libpath, name: result["name"].as_str().map(|name| name.to_owned()) })
}

/// Opens the library at `spec.libpath`.
///
/// # Safety
///
/// The library must run its own initialization. The caller owns the returned plugin.
#[cfg(not(test))]
fn open_plugin(spec: &PluginSpec) -> Result<(Library, LoadedSlipstreamPlugin), SlipstreamPluginManagerError> {
    // Trait objects have no C equivalent; the suppression is intentional — the plugin ABI
    // uses raw pointers and the caller takes ownership immediately via Box::from_raw.
    #[allow(improper_ctypes_definitions)]
    type PluginConstructor = unsafe extern "C" fn() -> *mut dyn SlipstreamPlugin;
    use libloading::Symbol;

    let (plugin, lib) = unsafe {
        let lib = Library::new(&spec.libpath)
            .map_err(|error| SlipstreamPluginManagerError::PluginLoadError(error.to_string()))?;
        let constructor: Symbol<PluginConstructor> = lib
            .get(b"_create_plugin")
            .map_err(|error| SlipstreamPluginManagerError::PluginLoadError(error.to_string()))?;
        let plugin_raw = constructor();
        if plugin_raw.is_null() {
            return Err(SlipstreamPluginManagerError::PluginLoadError(
                "plugin constructor returned a null pointer".to_string(),
            ));
        }
        (Box::from_raw(plugin_raw), lib)
    };
    Ok((lib, LoadedSlipstreamPlugin::new(plugin, spec.name.clone())))
}

/// Tests construct an in-process plugin instead of calling `dlopen` on the config's library file.
#[cfg(test)]
fn open_plugin(spec: &PluginSpec) -> Result<(Library, LoadedSlipstreamPlugin), SlipstreamPluginManagerError> {
    let lib = in_process_library()?;
    let plugin = tests::plugin_for_name(spec.name.as_deref());
    Ok((lib, LoadedSlipstreamPlugin::new(plugin, spec.name.clone())))
}

#[cfg(test)]
mod tests {
    use crate::slipstream_manager::{
        BROADCAST_QUEUE_CAPACITY,
        LoadedPlugin,
        LoadedSlipstreamPlugin,
        SlipstreamPluginManager,
        SlipstreamPluginManagerError,
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

    pub(super) fn plugin_for_name(name: Option<&str>) -> Box<dyn SlipstreamPlugin> {
        match name {
            Some(ANOTHER_DUMMY_NAME) => Box::new(TestPlugin2),
            _ => Box::new(TestPlugin),
        }
    }

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

    /// Removes `dir` when the test returns, including on failure.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let nanos =
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
            let dir = std::env::temp_dir().join(format!("slipstream-{label}-{}-{nanos}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_plugin_config(dir: &std::path::Path, library_name: &str, config_name: &str, plugin_name: &str) -> PathBuf {
        let library = dir.join(library_name);
        if !library.exists() {
            std::fs::write(&library, b"").unwrap();
        }
        let config = dir.join(config_name);
        std::fs::write(&config, format!("{{ libpath: \"./{library_name}\", name: \"{plugin_name}\" }}\n")).unwrap();
        config
    }

    #[test]
    fn test_plugin_list() {
        // Initialize empty manager.
        let plugin_manager = Arc::new(RwLock::new(SlipstreamPluginManager::new()));
        let plugin_manager_lock = plugin_manager.write().unwrap();

        // Load two plugins.
        let (_lib, mut plugin, config) = dummy_plugin_and_library(TestPlugin, "TESTPLUGIN_CONFIG");
        plugin.on_load(config, false).unwrap();
        plugin_manager_lock.plugins.write().unwrap().push(LoadedPlugin {
            plugin,
            _lib,
            libpath: PathBuf::from(config),
        });

        let (_lib, mut plugin, config) = dummy_plugin_and_library(TestPlugin2, "TESTPLUGIN2_CONFIG");
        plugin.on_load(config, false).unwrap();
        plugin_manager_lock.plugins.write().unwrap().push(LoadedPlugin {
            plugin,
            _lib,
            libpath: PathBuf::from(config),
        });

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

        let dir = TempDir::new("load");
        let config = write_plugin_config(&dir.0, "plugin.so", "one.json5", DUMMY_NAME);

        // Load rpc call.
        let load_result = plugin_manager_lock.load_plugin(&config);
        assert!(load_result.is_ok());
        assert_eq!(plugin_manager_lock.plugins.read().unwrap().len(), 1);

        // Unload rpc call.
        let unload_result = plugin_manager_lock.unload_plugin(DUMMY_NAME);
        assert!(unload_result.is_ok());
        assert_eq!(plugin_manager_lock.plugins.read().unwrap().len(), 0);
    }

    #[test]
    fn test_equivalent_library_paths_load_once() {
        let dir = TempDir::new("libpath");
        let first = write_plugin_config(&dir.0, "plugin.so", "first.json5", DUMMY_NAME);
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.0.join("plugin.so"), dir.0.join("link.so")).unwrap();
        #[cfg(unix)]
        let second = write_plugin_config(&dir.0, "link.so", "second.json5", ANOTHER_DUMMY_NAME);
        #[cfg(not(unix))]
        let second = write_plugin_config(&dir.0, "plugin.so", "second.json5", ANOTHER_DUMMY_NAME);

        let mut manager = SlipstreamPluginManager::new();
        manager.load_plugin(&first).unwrap();
        // The test creates plugin.so before this call.
        let canonical = std::fs::canonicalize(dir.0.join("plugin.so")).unwrap();
        assert_eq!(manager.plugins.read().unwrap()[0].libpath, canonical);

        let error = manager.load_plugin(&second).unwrap_err();
        assert!(matches!(error, SlipstreamPluginManagerError::PluginAlreadyLoaded(name) if name == DUMMY_NAME));
    }

    #[test]
    fn test_broadcast_mapping_update() {
        let manager = SlipstreamPluginManager::new();

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
        manager.plugins.write().unwrap().push(LoadedPlugin {
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

    #[test]
    fn test_install_broadcasts_subscribed_events() {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

        #[derive(Debug)]
        struct CountingPlugin {
            calls: std::sync::Arc<std::sync::atomic::AtomicU32>,
        }

        impl SlipstreamPlugin for CountingPlugin {
            fn name(&self) -> &'static str {
                "counting"
            }

            fn subscribed_events(&self) -> &[BroadcastEventKind] {
                const EVENTS: &[BroadcastEventKind] = &[
                    BroadcastEventKind::MappingUpdate,
                    BroadcastEventKind::MappingRemoval,
                    BroadcastEventKind::StakingReward,
                    BroadcastEventKind::Block,
                ];
                EVENTS
            }

            fn on_broadcast(&self, _event: BroadcastEvent<'_>) -> anyhow::Result<()> {
                self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }
        }

        let mut manager = SlipstreamPluginManager::new();
        manager.install(CountingPlugin { calls: std::sync::Arc::clone(&calls) }).unwrap();

        manager.broadcast(BroadcastEvent::MappingUpdate {
            program_id: b"credits.aleo",
            mapping_name: b"account",
            key: b"key",
            value: b"value",
            block_height: 1,
        });
        manager.broadcast(BroadcastEvent::MappingRemoval {
            program_id: b"credits.aleo",
            mapping_name: b"account",
            key: Some(b"key"),
            block_height: 1,
        });
        manager.broadcast(BroadcastEvent::StakingReward {
            staker: &[0; 32],
            validator: &[1; 32],
            reward: 10,
            new_stake: 110,
            block_height: 1,
        });
        manager.broadcast(BroadcastEvent::Block { block: b"block", block_height: 1 });
        manager.flush();
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 4);
        assert!(manager.has_subscribers(BroadcastEventKind::MappingRemoval));
        assert!(manager.has_subscribers(BroadcastEventKind::Block));
        manager.unload_plugin("counting").unwrap();
        assert!(!manager.has_subscribers(BroadcastEventKind::MappingRemoval));
        assert!(!manager.has_subscribers(BroadcastEventKind::Block));
    }

    /// Drops the release sender on unwind so a failed test cannot leave the worker blocked.
    struct ReleaseOnDrop(Option<std::sync::mpsc::Sender<()>>);

    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            self.0.take();
        }
    }

    #[test]
    fn test_broadcast_returns_before_plugin_callback_finishes() {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();

        #[derive(Debug)]
        struct BlockingPlugin {
            calls: std::sync::Arc<std::sync::atomic::AtomicU32>,
            entered: std::sync::mpsc::Sender<()>,
            release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
        }

        impl SlipstreamPlugin for BlockingPlugin {
            fn name(&self) -> &'static str {
                "blocking"
            }

            fn subscribed_events(&self) -> &[BroadcastEventKind] {
                &[BroadcastEventKind::Block]
            }

            fn on_broadcast(&self, _event: BroadcastEvent<'_>) -> anyhow::Result<()> {
                if self.calls.load(std::sync::atomic::Ordering::SeqCst) == 0 {
                    let _ = self.entered.send(());
                    // The sender is dropped if the test fails. `recv` then returns and the worker can exit.
                    let _ = self.release.lock().unwrap().recv();
                }
                self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }
        }

        let mut manager = SlipstreamPluginManager::new();
        manager
            .install(BlockingPlugin {
                calls: std::sync::Arc::clone(&calls),
                entered: entered_tx,
                release: std::sync::Mutex::new(release_rx),
            })
            .unwrap();
        // Declared after the manager so unwind drops the sender before joining the worker.
        let release = ReleaseOnDrop(Some(release_tx));

        manager.broadcast(BroadcastEvent::Block { block: b"block", block_height: 1 });
        // The plugin blocks in its first callback. This receive runs only if `broadcast` has returned.
        entered_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();

        for _ in 0..BROADCAST_QUEUE_CAPACITY {
            manager.broadcast(BroadcastEvent::Block { block: b"block", block_height: 1 });
        }
        manager.broadcast(BroadcastEvent::Block { block: b"dropped", block_height: 1 });

        drop(release);
        manager.flush();
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            u32::try_from(BROADCAST_QUEUE_CAPACITY).unwrap() + 1
        );
    }
}
