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

#[cfg(target_arch = "wasm32")]
compile_error!(
    "snarkvm-slipstream-plugin-manager uses libloading for dynamic plugin \
     loading, which is not supported on wasm32 targets. Do not enable the \
     `history`, `history-staking-rewards`, or `slipstream-plugins` features \
     when targeting wasm32."
);

pub mod slipstream_manager;

pub use slipstream_manager::{LoadedSlipstreamPlugin, SlipstreamPluginManager};
pub use snarkvm_slipstream_plugin_interface::{BroadcastEvent, BroadcastEventKind};
