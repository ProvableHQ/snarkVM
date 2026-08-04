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

use anyhow::Result;
use std::any::Any;

/// Discriminant-only companion to [`BroadcastEvent`], used by
/// [`SlipstreamPlugin::subscribed_events`] to declare which event types a plugin
/// wishes to receive without carrying data payloads.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BroadcastEventKind {
    /// Mapping key-value update during canonical finalize.
    MappingUpdate,
    /// Staking reward distribution during canonical finalize.
    StakingReward,
}

/// A single event dispatched to plugins via [`SlipstreamPlugin::on_broadcast`].
///
/// All `&[u8]` fields carry little-endian byte representations of the
/// corresponding snarkVM console types (serialized via `ToBytes`).
///
/// Derives `Copy` — every field is `Copy` (`&[u8]`, `u32`, `u64`) — so the
/// same value can be passed to multiple plugins in a dispatch loop without cloning.
#[derive(Copy, Clone, Debug)]
pub enum BroadcastEvent<'a> {
    /// A mapping key-value pair was inserted or updated during canonical finalize.
    MappingUpdate { program_id: &'a [u8], mapping_name: &'a [u8], key: &'a [u8], value: &'a [u8], block_height: u32 },
    /// A staking reward was distributed to a staker during canonical finalize.
    StakingReward { staker: &'a [u8], validator: &'a [u8], reward: u64, new_stake: u64, block_height: u32 },
}

impl BroadcastEvent<'_> {
    /// Returns the discriminant of this event.
    pub fn kind(&self) -> BroadcastEventKind {
        match self {
            BroadcastEvent::MappingUpdate { .. } => BroadcastEventKind::MappingUpdate,
            BroadcastEvent::StakingReward { .. } => BroadcastEventKind::StakingReward,
        }
    }
}

/// The interface for Aleo Slipstream plugins. A plugin must implement
/// the `SlipstreamPlugin` trait to work with the runtime. In addition,
/// the dynamic library must export a `C` function `_create_plugin` that
/// creates the implementation of the plugin.
pub trait SlipstreamPlugin: Any + Send + Sync + std::fmt::Debug {
    /// Returns the name of the plugin.
    fn name(&self) -> &'static str;

    /// The callback called when a plugin is loaded by the system, used for
    /// doing whatever initialization is required by the plugin. The
    /// `_config_file` contains the name of the config file (JSON format) with
    /// a `libpath` field indicating the full path of the shared library.
    fn on_load(&mut self, _config_file: &str, _is_reload: bool) -> Result<()> {
        Ok(())
    }

    /// The callback called right before a plugin is unloaded by the system.
    /// Used for doing cleanup before unload.
    fn on_unload(&mut self) {}

    /// Returns the event kinds this plugin subscribes to.
    ///
    /// The manager checks this before serializing and dispatching each event,
    /// so plugins that return an empty slice pay no serialization cost. Defaults
    /// to no subscriptions.
    fn subscribed_events(&self) -> &[BroadcastEventKind] {
        &[]
    }

    /// Receives a single broadcast event from the plugin manager.
    ///
    /// Only invoked when the event's kind appears in [`subscribed_events`].
    fn on_broadcast(&self, _event: BroadcastEvent<'_>) -> Result<()> {
        Ok(())
    }
}
