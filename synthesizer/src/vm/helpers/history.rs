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

//! # History Storage
//!
//! This module provides two implementations for storing historical mapping data:
//!
//! ## Original File-Based (`history` feature)
//! - Stores each mapping as a separate JSON file
//! - Simple and human-readable
//! - Can consume significant disk space
//!
//! ## RocksDB-Based (`history-all-mappings` feature)
//! - Stores mappings in RocksDB only when they change
//! - Significantly reduces disk space usage
//! - Supports tracking mappings from all deployed programs
//! - **Performance Note**: Loading a mapping may require searching backwards through
//!   up to 1000 blocks to find the most recent version since mappings are only
//!   stored when they change.
//!
//! ## Future: Tracking All Programs
//! Currently, only credits.aleo mappings are tracked automatically in finalize.rs.
//! To extend tracking to all deployed programs:
//! 1. Hook into the finalize operation processing
//! 2. Extract program_id and mapping_name from FinalizeOperations
//! 3. Call `history.store_mapping_if_changed()` for each modified mapping
//! 4. This would require access to the actual mapping data after finalize operations

use serde::{Deserialize, Serialize};

#[cfg(not(feature = "history-all-mappings"))]
use aleo_std::{StorageMode, aleo_ledger_dir};
#[cfg(feature = "history-all-mappings")]
use snarkvm_ledger_store::helpers::rocksdb::internal::{DataMap, HistoryMap, MapID};
#[cfg(feature = "history-all-mappings")]
use snarkvm_ledger_store::helpers::{Map, MapRead};

use aleo_std::StorageMode;
use anyhow::{Context, Result};
#[cfg(feature = "history-all-mappings")]
use console::program::{Identifier, ProgramID};
use serde_json;
use std::{
    fmt::{Display, Formatter},
    path::PathBuf,
};

#[cfg(not(feature = "history-all-mappings"))]
/// Returns the path where a `history` directory may be stored.
pub fn history_directory_path(network: u16, storage_mode: &StorageMode) -> PathBuf {
    const HISTORY_DIRECTORY_NAME: &str = "history";

    // Create the name of the history directory.
    let directory_name = match &storage_mode {
        StorageMode::Development(id) => format!(".{HISTORY_DIRECTORY_NAME}-{network}-{id}"),
        StorageMode::Production | StorageMode::Custom(_) => format!("{HISTORY_DIRECTORY_NAME}-{network}"),
        StorageMode::Test(_) => unimplemented!(),
    };

    // Obtain the path to the ledger.
    let mut path = aleo_ledger_dir(network, storage_mode);
    // Go to the folder right above the ledger.
    path.pop();
    // Append the history directory's name.
    path.push(directory_name);

    path
}

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

// Original file-based implementation (when history-all-mappings is not enabled)
#[cfg(not(feature = "history-all-mappings"))]
pub struct History {
    /// The path to the history directory.
    path: PathBuf,
}

#[cfg(not(feature = "history-all-mappings"))]
impl History {
    /// Initializes a new instance of `History`.
    pub fn new(network: u16, storage_mode: &StorageMode) -> Self {
        Self { path: history_directory_path(network, storage_mode) }
    }

    /// Stores a mapping from a given block in the history directory as JSON.
    pub fn store_mapping<T>(&self, height: u32, mapping: MappingName, data: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        // Get the path to the block directory.
        let path = self.block_path(height);
        // Create the block directory if it does not exist.
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }

        // Write the entry to the block directory.
        let path = path.join(format!("block-{height}-{mapping}.json"));
        std::fs::write(path, serde_json::to_string_pretty(data)?)?;

        Ok(())
    }

    /// Loads the JSON string for a mapping from a given block from the history directory.
    pub fn load_mapping(&self, height: u32, mapping: MappingName) -> Result<String> {
        // Get the path to the block directory.
        let path = self.block_path(height);
        // Get the path to the block file.
        let path = path.join(format!("block-{height}-{mapping}.json"));

        // Read the file.
        let data = std::fs::read_to_string(path)?;

        Ok(data)
    }

    // A helper function to get the path to the block directory.
    fn block_path(&self, height: u32) -> PathBuf {
        // Get the path the directory group.
        let group = Self::group(height);
        let path = self.path.join(format!("group-{group}"));
        // Get the path to the block directory.
        path.join(format!("block-{height}"))
    }

    // A helper function to calculate the group number for a given block height.
    fn group(height: u32) -> u32 {
        height.saturating_div(u16::MAX as u32)
    }
}

// New RocksDB-based implementation (when history-all-mappings is enabled)
#[cfg(feature = "history-all-mappings")]
pub struct History {
    /// The RocksDB DataMap for storing mapping data indexed by (program_id, mapping_name, block height).
    /// Only stores mappings when they change from the previous block.
    mapping_data: DataMap<(String, String, u32), Vec<u8>>,
}

#[cfg(feature = "history-all-mappings")]
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

    /// Stores a mapping from a given block only if it has changed from the previous block.
    /// 
    /// # Performance Note
    /// This function checks if the data has changed by comparing with the previous block.
    /// When loading mappings with `load_mapping`, you may need to iterate backwards through
    /// block heights or use binary search to find the most recent version of a mapping.
    pub fn store_mapping_if_changed<T, N: console::prelude::Network>(
        &self,
        program_id: &ProgramID<N>,
        mapping_name: &Identifier<N>,
        height: u32,
        data: &T,
    ) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        // Serialize the data to JSON for comparison and storage
        let json_data = serde_json::to_vec(data)?;
        
        // Create the key for storage
        let key = (program_id.to_string(), mapping_name.to_string(), height);
        
        // Check if this is height 0 or if the data has changed from the previous block
        let should_store = if height == 0 {
            true
        } else {
            // Try to get the previous block's data
            let prev_key = (program_id.to_string(), mapping_name.to_string(), height - 1);
            match self.mapping_data.get_confirmed(&prev_key)? {
                Some(prev_data) => {
                    // Only store if data has changed
                    prev_data.as_ref() != json_data.as_slice()
                }
                // If no previous data exists, we need to search backwards to find the last stored version
                None => {
                    self.find_last_stored_data(program_id, mapping_name, height - 1)?
                        .map(|prev_data| prev_data != json_data)
                        .unwrap_or(true) // Store if no previous data found
                }
            }
        };

        if should_store {
            // Store in RocksDB with composite key (program_id, mapping_name, height)
            self.mapping_data.insert(key, json_data)?;
        }

        Ok(())
    }

    /// Helper function to find the last stored data for a mapping by searching backwards.
    /// Returns None if no previous data is found.
    fn find_last_stored_data<N: console::prelude::Network>(
        &self,
        program_id: &ProgramID<N>,
        mapping_name: &Identifier<N>,
        from_height: u32,
    ) -> Result<Option<Vec<u8>>> {
        // Search backwards up to a reasonable limit (e.g., 1000 blocks)
        const MAX_SEARCH_DEPTH: u32 = 1000;
        
        for h in (from_height.saturating_sub(MAX_SEARCH_DEPTH)..=from_height).rev() {
            let key = (program_id.to_string(), mapping_name.to_string(), h);
            if let Some(data) = self.mapping_data.get_confirmed(&key)? {
                return Ok(Some(data.into_owned()));
            }
        }
        
        Ok(None)
    }

    /// Loads the JSON string for a mapping from a given block or the most recent version.
    /// 
    /// # Performance Note
    /// If the exact block height doesn't have data, this function searches backwards
    /// up to 1000 blocks to find the most recent version. This is because mappings are
    /// only stored when they change.
    pub fn load_mapping<N: console::prelude::Network>(
        &self,
        program_id: &ProgramID<N>,
        mapping_name: &Identifier<N>,
        height: u32,
    ) -> Result<String> {
        // Try to get data at the exact height first
        let key = (program_id.to_string(), mapping_name.to_string(), height);
        
        let json_bytes = match self.mapping_data.get_confirmed(&key)? {
            Some(data) => data,
            None => {
                // Search backwards to find the most recent version
                self.find_last_stored_data(program_id, mapping_name, height)?
                    .with_context(|| {
                        format!(
                            "History data not found for program '{}', mapping '{}' at or before block {}",
                            program_id, mapping_name, height
                        )
                    })?
                    .into()
            }
        };
        
        // Convert bytes to string
        let json_string = String::from_utf8(json_bytes.into_owned()).with_context(|| {
            format!(
                "Failed to parse history data for program '{}', mapping '{}' at block {} as UTF-8",
                program_id, mapping_name, height
            )
        })?;
        
        Ok(json_string)
    }

    /// Legacy method for compatibility with credits.aleo MappingName enum.
    /// Only stores if the data has changed from the previous block.
    pub fn store_mapping<T>(&self, height: u32, mapping: MappingName, data: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        // For backward compatibility, use "credits.aleo" as the program ID
        let program_id_str = "credits.aleo";
        let mapping_name_str = mapping.to_string();
        
        // Serialize the data to JSON for comparison and storage
        let json_data = serde_json::to_vec(data)?;
        
        // Create the key for storage
        let key = (program_id_str.to_string(), mapping_name_str.clone(), height);
        
        // Check if data has changed from the previous block
        let should_store = if height == 0 {
            true
        } else {
            let prev_key = (program_id_str.to_string(), mapping_name_str, height - 1);
            match self.mapping_data.get_confirmed(&prev_key)? {
                Some(prev_data) => prev_data.as_ref() != json_data.as_slice(),
                None => true, // Store if no previous data
            }
        };

        if should_store {
            self.mapping_data.insert(key, json_data)?;
        }

        Ok(())
    }
}
