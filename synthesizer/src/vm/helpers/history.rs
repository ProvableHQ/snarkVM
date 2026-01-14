// Copyright (c) 2019-2025 Provable Inc.
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

use serde::{Deserialize, Serialize};

#[cfg(feature = "rocks")]
use snarkvm_ledger_store::helpers::rocksdb::internal::{DataMap, HistoryMap, MapID};
#[cfg(feature = "rocks")]
use snarkvm_ledger_store::helpers::{Map, MapRead};

use aleo_std::StorageMode;
use anyhow::{Context, Result};
use serde_json;
use std::fmt::{Display, Formatter};

#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "lowercase")]
pub enum MappingName {
    /// The `bonded` mapping.
    Bonded,
    /// The `delegated` mapping.
    Delegated,
    /// The `metadata` mapping.
    Metadata,
    /// The `unbonding` mapping.
    Unbonding,
    /// The `withdraw` mapping.
    Withdraw,
}

impl Display for MappingName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bonded => write!(f, "bonded"),
            Self::Delegated => write!(f, "delegated"),
            Self::Metadata => write!(f, "metadata"),
            Self::Unbonding => write!(f, "unbonding"),
            Self::Withdraw => write!(f, "withdraw"),
        }
    }
}

#[cfg(feature = "rocks")]
pub struct History {
    /// The RocksDB DataMap for storing mapping data indexed by (block height, mapping name).
    mapping_data: DataMap<(u32, MappingName), Vec<u8>>,
}

#[cfg(feature = "rocks")]
impl History {
    /// Initializes a new instance of `History`.
    pub fn new(network: u16, storage_mode: &StorageMode) -> Result<Self> {
        // Open the DataMap for history storage
        let mapping_data = snarkvm_ledger_store::helpers::rocksdb::internal::RocksDB::open_map(
            network,
            storage_mode.clone(),
            MapID::History(HistoryMap::MappingData),
        )?;

        Ok(Self { mapping_data })
    }

    /// Stores a mapping from a given block in the history storage as serialized bytes.
    pub fn store_mapping<T>(&self, height: u32, mapping: MappingName, data: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        // Serialize the data to JSON for backwards compatibility and readability
        let json_data = serde_json::to_vec(data)?;
        
        // Store in RocksDB with composite key (height, mapping)
        self.mapping_data.insert((height, mapping), json_data)?;

        Ok(())
    }

    /// Loads the JSON string for a mapping from a given block from the history storage.
    pub fn load_mapping(&self, height: u32, mapping: MappingName) -> Result<String> {
        // Retrieve the serialized data from RocksDB
        let json_bytes = self.mapping_data.get_confirmed(&(height, mapping))?
            .with_context(|| format!("History data not found for block {} and '{}' mapping", height, mapping))?;
        
        // Convert bytes to string
        let json_string = String::from_utf8(json_bytes.into_owned())
            .with_context(|| format!("Failed to parse history data for block {} and '{}' mapping as UTF-8", height, mapping))?;
        
        Ok(json_string)
    }
}
