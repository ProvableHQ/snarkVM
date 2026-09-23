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

//! The storage schema version recorded in each ledger database.
//!
//! `RocksDB::open` reads the version before anything else uses the database. A version from a
//! newer build is refused. An older version is brought forward by the migrations between it and
//! [`STORAGE_VERSION`].
//!
//! Ledgers written before this record existed have no version key. Those databases are
//! [`StorageVersion::V0`].

use super::{MapID, MetadataMap, PREFIX_LEN, ProgramMap};

use anyhow::{Result, anyhow, bail};

/// The on-disk schema versions a ledger can be at.
///
/// `V0` is the version of every database that has no schema record. Later variants are produced
/// by migrations, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StorageVersion {
    /// No schema record, or a database stamped by the build that introduced the record.
    V0 = 0,
    /// History tables exist, and [`MetadataKey::HistorySyncedHeight`] is the next height to index.
    V1 = 1,
}

/// The storage schema version this build writes and understands.
pub const STORAGE_VERSION: StorageVersion = StorageVersion::V1;

/// The well-known keys of the storage metadata map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum MetadataKey {
    /// The [`StorageVersion`] the database was last written under, as a little-endian `u32`.
    StorageVersion = 0,
    /// The next block height history indexing will process, as a little-endian `u32`.
    ///
    /// A height `h` is indexed when `h` is strictly less than this value. `0` means no height is
    /// indexed.
    HistorySyncedHeight = 1,
}

impl StorageVersion {
    /// Decodes a version this build knows how to open.
    fn from_u32(version: u32) -> Result<Self> {
        match version {
            0 => Ok(Self::V0),
            1 => Ok(Self::V1),
            version => bail!("Unknown storage schema version {version}"),
        }
    }

    /// Encodes this version for the metadata map.
    fn to_bytes(self) -> [u8; 4] {
        (self as u32).to_le_bytes()
    }
}

/// Returns the raw key prefix under which every entry of the given map is stored.
pub(crate) fn map_prefix(network_id: u16, map_id: MapID) -> [u8; PREFIX_LEN] {
    let mut prefix = [0u8; PREFIX_LEN];
    prefix[..2].copy_from_slice(&network_id.to_le_bytes());
    prefix[2..].copy_from_slice(&u16::from(map_id).to_le_bytes());
    prefix
}

/// Returns the raw database key for a metadata entry.
fn metadata_key(network_id: u16, key: MetadataKey) -> Vec<u8> {
    let mut raw = map_prefix(network_id, MapID::Metadata(MetadataMap::Metadata)).to_vec();
    raw.push(key as u8);
    raw
}

/// Reads the stored version as a `u32`. A missing record is `0` ([`StorageVersion::V0`]).
fn read_storage_version(database: &rocksdb::DB, network_id: u16) -> Result<u32> {
    match database.get(metadata_key(network_id, MetadataKey::StorageVersion))? {
        Some(bytes) => {
            let bytes: [u8; 4] =
                bytes.as_slice().try_into().map_err(|_| anyhow!("Malformed storage version record"))?;
            Ok(u32::from_le_bytes(bytes))
        }
        None => Ok(0),
    }
}

/// Reads the storage schema version. A missing record is [`StorageVersion::V0`].
pub(crate) fn get_storage_version(database: &rocksdb::DB, network_id: u16) -> Result<StorageVersion> {
    StorageVersion::from_u32(read_storage_version(database, network_id)?)
}

/// Writes the storage schema version.
pub(crate) fn set_storage_version(database: &rocksdb::DB, network_id: u16, version: StorageVersion) -> Result<()> {
    Ok(database.put(metadata_key(network_id, MetadataKey::StorageVersion), version.to_bytes())?)
}

/// Reads the next history height. A missing record is `0`.
pub(crate) fn read_history_synced_height(database: &rocksdb::DB, network_id: u16) -> Result<u32> {
    match database.get(metadata_key(network_id, MetadataKey::HistorySyncedHeight))? {
        Some(bytes) => {
            let bytes: [u8; 4] = bytes.as_slice().try_into().map_err(|_| anyhow!("Malformed history sync cursor"))?;
            Ok(u32::from_le_bytes(bytes))
        }
        None => Ok(0),
    }
}

/// Writes the next history height. The write is not part of a finalize atomic batch.
pub(crate) fn set_history_synced_height(database: &rocksdb::DB, network_id: u16, height: u32) -> Result<()> {
    Ok(database.put(metadata_key(network_id, MetadataKey::HistorySyncedHeight), height.to_le_bytes())?)
}

/// The mapping-history prefixes written by storage schema v0.
const LEGACY_MAPPING_HISTORY: [ProgramMap; 2] = [ProgramMap::MappingUpdate, ProgramMap::MappingUpdateHeights];

/// Brings `database` up to [`STORAGE_VERSION`].
///
/// A version newer than this build is an error. [`StorageVersion::V0`] advances to
/// [`StorageVersion::V1`] when the legacy mapping-history prefixes are empty: the history sync
/// cursor is set to `0`, so no height is indexed yet. A v0 database that still has those keys is
/// refused and left unchanged.
pub(crate) fn migrate_storage(database: &rocksdb::DB, network_id: u16) -> Result<()> {
    let version = read_storage_version(database, network_id)?;
    if version > STORAGE_VERSION as u32 {
        bail!(
            "This ledger was written by a newer version of snarkVM (storage schema v{version}; this build understands \
             v{}). Upgrade snarkVM, or resync from genesis.",
            STORAGE_VERSION as u32,
        );
    }
    let version = get_storage_version(database, network_id)?;
    if version == StorageVersion::V0 {
        ensure_no_legacy_mapping_history(database, network_id)?;
        set_history_synced_height(database, network_id, 0)?;
        set_storage_version(database, network_id, StorageVersion::V1)?;
    }
    Ok(())
}

/// Returns an error when a v0 mapping-history prefix still holds keys.
fn ensure_no_legacy_mapping_history(database: &rocksdb::DB, network_id: u16) -> Result<()> {
    for map in LEGACY_MAPPING_HISTORY {
        if !is_prefix_empty(database, &map_prefix(network_id, MapID::Program(map)))? {
            bail!(
                "Refusing to open this ledger: it still has mapping history from storage schema v0, which this build \
                 does not read. Delete the ledger and sync again."
            );
        }
    }
    Ok(())
}

/// Returns whether no key starts with `prefix`.
fn is_prefix_empty(database: &rocksdb::DB, prefix: &[u8; PREFIX_LEN]) -> Result<bool> {
    let mut iterator = database.raw_iterator();
    iterator.seek(prefix);
    match iterator.key() {
        Some(key) => Ok(!key.starts_with(prefix)),
        None => {
            iterator.status()?;
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::rocksdb::{Database, RocksDB};

    use aleo_std::StorageMode;

    const NETWORK_ID: u16 = 0;

    fn raw_key(map: ProgramMap, suffix: u8) -> Vec<u8> {
        let mut key = map_prefix(NETWORK_ID, MapID::Program(map)).to_vec();
        key.push(suffix);
        key
    }

    #[test]
    fn test_fresh_database_is_v1_with_cursor_zero() {
        let db = RocksDB::open(NETWORK_ID, StorageMode::new_test(None)).unwrap();
        assert_eq!(get_storage_version(&db, NETWORK_ID).unwrap(), StorageVersion::V1);
        assert_eq!(read_history_synced_height(&db, NETWORK_ID).unwrap(), 0);
    }

    #[test]
    fn test_newer_version_is_refused() {
        let db = RocksDB::open(NETWORK_ID, StorageMode::new_test(None)).unwrap();
        db.put(metadata_key(NETWORK_ID, MetadataKey::StorageVersion), 2u32.to_le_bytes()).unwrap();
        let error = migrate_storage(&db, NETWORK_ID).unwrap_err().to_string();
        assert!(error.contains("newer version"), "{error}");
        let raw = db.get(metadata_key(NETWORK_ID, MetadataKey::StorageVersion)).unwrap().unwrap();
        assert_eq!(raw, 2u32.to_le_bytes());
    }

    #[test]
    fn test_absent_version_migrates_to_v1() {
        let db = RocksDB::open(NETWORK_ID, StorageMode::new_test(None)).unwrap();
        db.delete(metadata_key(NETWORK_ID, MetadataKey::StorageVersion)).unwrap();
        db.delete(metadata_key(NETWORK_ID, MetadataKey::HistorySyncedHeight)).unwrap();
        assert_eq!(get_storage_version(&db, NETWORK_ID).unwrap(), StorageVersion::V0);
        migrate_storage(&db, NETWORK_ID).unwrap();
        assert_eq!(get_storage_version(&db, NETWORK_ID).unwrap(), StorageVersion::V1);
        assert_eq!(read_history_synced_height(&db, NETWORK_ID).unwrap(), 0);
    }

    #[test]
    fn test_v0_without_mapping_history_migrates_to_v1() {
        let db = RocksDB::open(NETWORK_ID, StorageMode::new_test(None)).unwrap();
        db.put(metadata_key(NETWORK_ID, MetadataKey::StorageVersion), 0u32.to_le_bytes()).unwrap();
        // Staking-reward rows are not the v0 mapping-history layout.
        db.put(raw_key(ProgramMap::StakingRewards, 1), b"reward").unwrap();
        migrate_storage(&db, NETWORK_ID).unwrap();
        assert_eq!(get_storage_version(&db, NETWORK_ID).unwrap(), StorageVersion::V1);
        assert_eq!(read_history_synced_height(&db, NETWORK_ID).unwrap(), 0);
        assert!(db.get(raw_key(ProgramMap::StakingRewards, 1)).unwrap().is_some());
    }

    #[test]
    fn test_v0_with_mapping_history_is_refused() {
        for map in [ProgramMap::MappingUpdate, ProgramMap::MappingUpdateHeights] {
            let db = RocksDB::open(NETWORK_ID, StorageMode::new_test(None)).unwrap();
            db.put(metadata_key(NETWORK_ID, MetadataKey::StorageVersion), 0u32.to_le_bytes()).unwrap();
            set_history_synced_height(&db, NETWORK_ID, 7).unwrap();
            let key = raw_key(map, 1);
            db.put(&key, b"old").unwrap();
            let error = migrate_storage(&db, NETWORK_ID).unwrap_err().to_string();
            assert!(error.contains("storage schema v0"), "{error}");
            assert_eq!(read_storage_version(&db, NETWORK_ID).unwrap(), 0);
            assert_eq!(read_history_synced_height(&db, NETWORK_ID).unwrap(), 7);
            assert_eq!(db.get(&key).unwrap().unwrap(), b"old");
        }
    }

    #[test]
    fn test_v0_ignores_other_networks_and_keeps_an_existing_cursor() {
        let db = RocksDB::open(NETWORK_ID, StorageMode::new_test(None)).unwrap();
        db.put(metadata_key(NETWORK_ID, MetadataKey::StorageVersion), 0u32.to_le_bytes()).unwrap();
        let mut foreign = map_prefix(NETWORK_ID + 1, MapID::Program(ProgramMap::MappingUpdate)).to_vec();
        foreign.push(1);
        db.put(&foreign, b"other-network").unwrap();
        migrate_storage(&db, NETWORK_ID).unwrap();
        assert_eq!(get_storage_version(&db, NETWORK_ID).unwrap(), StorageVersion::V1);
        assert_eq!(read_history_synced_height(&db, NETWORK_ID).unwrap(), 0);
        assert_eq!(db.get(&foreign).unwrap().unwrap(), b"other-network");

        // A second open of a v1 database does not reset the cursor.
        set_history_synced_height(&db, NETWORK_ID, 4).unwrap();
        migrate_storage(&db, NETWORK_ID).unwrap();
        assert_eq!(read_history_synced_height(&db, NETWORK_ID).unwrap(), 4);
    }
}
