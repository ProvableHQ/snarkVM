// Copyright 2024 Aleo Network Foundation
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

#[cfg(feature = "network")]
pub use snarkvm_circuit_network as circuit_network;
#[cfg(feature = "console")]
pub use snarkvm_console as console;
#[cfg(feature = "network")]
pub use snarkvm_console::network as console_network;
#[cfg(feature = "curves")]
pub use snarkvm_curves as curves;
#[cfg(feature = "fields")]
pub use snarkvm_fields as fields;
#[cfg(feature = "ledger")]
pub use snarkvm_ledger_block as ledger_block;
#[cfg(feature = "ledger")]
pub use snarkvm_ledger_query as ledger_query;
#[cfg(feature = "ledger")]
pub use snarkvm_ledger_store as ledger_store;
#[cfg(feature = "synthesizer")]
pub use snarkvm_synthesizer as synthesizer;
#[cfg(feature = "utilities")]
pub use snarkvm_utilities as utilities;

#[cfg(test)]
mod tests;
