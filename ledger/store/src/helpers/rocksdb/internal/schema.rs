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

//! The storage schema version, and the migrations between versions.
//!
//! A database records the schema version it was last written under. `RocksDB::open` reads it
//! before anything else touches the database: a version from a newer build is refused rather than
//! misread, and an older one is brought up to date by running the migrations between it and
//! [`STORAGE_VERSION`], in order.
//!
//! Nothing on disk recorded this before. Three incompatible layouts of one table shipped within
//! three weeks and could only be told apart by inspecting byte patterns, which is how a node came
//! to silently misread its own data. Every change to the on-disk layout from here on bumps the
//! version and ships with a migration.
//!
//! Migrations see the database as raw key ranges under each map's prefix, never through a typed
//! map. That is deliberate: it lets a migration handle a table that the running build has no
//! type for, such as one that a removed feature used to write. For the same reason no table may be
//! conditionally compiled -- a table that exists only under a cargo feature is invisible to a
//! build without it, and a migration's behaviour would then depend on how the tool was compiled.

use super::{MapID, MetadataMap, PREFIX_LEN, RetiredMap};

use anyhow::{Result, anyhow, bail};

/// The storage schema version this build writes and understands.
///
/// Bump this whenever the on-disk layout changes in a way that a build expecting the previous
/// version would read incorrectly, and add the corresponding migration to [`migrations`].
///
/// This only protects forward from the release that introduced the record: builds older than that
/// do not consult it at all.
pub const STORAGE_VERSION: u32 = 1;

/// The well-known keys of the storage metadata map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum MetadataKey {
    /// The storage schema version the database was last written under.
    StorageVersion = 0,
}

/// How a migration run was requested, which decides what it is allowed to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationMode {
    /// Run as a side effect of opening the database. Only migrations that would not touch any
    /// existing data are applied; one that would is refused, and the caller is told to run it
    /// explicitly. This keeps an ordinary node start from silently doing destructive or
    /// long-running work.
    OnOpen,
    /// Run on request, e.g. by a `migrate` command. Every pending migration is applied.
    Explicit,
}

/// A migration from one storage schema version to the next.
struct Migration {
    /// The version this migration starts from; it produces `from + 1`.
    from: u32,
    /// A one-line description, for logs and error messages.
    description: &'static str,
    /// Whether applying this migration would change any existing data in the database.
    ///
    /// Must be cheap: it runs on every open of a database that is behind.
    has_work: fn(&rocksdb::DB, u16) -> Result<bool>,
    /// Applies the migration. Must be safe to repeat: an interrupted run is retried from the
    /// start, as the version only advances once a migration has finished.
    apply: fn(&rocksdb::DB, u16) -> Result<()>,
}

/// Every migration, in order. The `from` versions must be `0, 1, ..., STORAGE_VERSION - 1`.
const fn migrations() -> [Migration; STORAGE_VERSION as usize] {
    [Migration {
        from: 0,
        description: "drop the tables of the removed `history` feature",
        has_work: drop_history_tables::has_work,
        apply: drop_history_tables::apply,
    }]
}

/// Migration v0 -> v1.
///
/// The `history` feature recorded mapping updates in two tables that never recorded deletions and
/// that mixed incompatible height encodings, so their contents cannot be repaired; the feature and
/// its tables were removed. This clears whatever an existing database still holds under their
/// prefixes. A node that never enabled the feature has nothing there, and the migration is then a
/// no-op that runs on open.
mod drop_history_tables {
    use super::*;

    /// The prefixes to clear.
    const RETIRED: [RetiredMap; 2] = [RetiredMap::MappingUpdate, RetiredMap::MappingUpdateHeights];

    pub(super) fn has_work(database: &rocksdb::DB, network_id: u16) -> Result<bool> {
        for map in RETIRED {
            if !is_prefix_empty(database, &map_prefix(network_id, MapID::Retired(map)))? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn apply(database: &rocksdb::DB, network_id: u16) -> Result<()> {
        let mut batch = rocksdb::WriteBatch::default();
        for map in RETIRED {
            let (from, to) = prefix_range(&map_prefix(network_id, MapID::Retired(map)));
            batch.delete_range(from, to);
        }
        database.write(batch)?;
        Ok(())
    }
}

/// Returns the raw key prefix under which every entry of the given map is stored.
pub(crate) fn map_prefix(network_id: u16, map_id: MapID) -> [u8; PREFIX_LEN] {
    let mut prefix = [0u8; PREFIX_LEN];
    prefix[..2].copy_from_slice(&network_id.to_le_bytes());
    prefix[2..].copy_from_slice(&u16::from(map_id).to_le_bytes());
    prefix
}

/// Returns the half-open raw key range `[from, to)` that contains exactly the keys starting with
/// the given prefix.
///
/// `to` is the byte-wise successor of the prefix, which is the right bound regardless of how the
/// prefix's fields are encoded (a `u16` map ID is little-endian, so `id + 1` would not do).
fn prefix_range(prefix: &[u8; PREFIX_LEN]) -> (Vec<u8>, Vec<u8>) {
    let mut to = prefix.to_vec();
    // Increment with carry. A prefix of all 0xFF bytes has no successor, but that would need
    // both a network ID and a map ID of `u16::MAX`, neither of which exists.
    for byte in to.iter_mut().rev() {
        if *byte == u8::MAX {
            *byte = 0;
        } else {
            *byte += 1;
            break;
        }
    }
    (prefix.to_vec(), to)
}

/// Returns whether no key starts with the given prefix.
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

/// Returns the raw database key for a metadata entry.
fn metadata_key(network_id: u16, key: MetadataKey) -> Vec<u8> {
    let mut raw = map_prefix(network_id, MapID::Metadata(MetadataMap::Metadata)).to_vec();
    raw.push(key as u8);
    raw
}

/// Reads the storage schema version, treating an absent record as zero.
///
/// Zero is the right default: every database written before this record existed is, by
/// definition, at the version that preceded it.
pub(crate) fn get_storage_version(database: &rocksdb::DB, network_id: u16) -> Result<u32> {
    match database.get(metadata_key(network_id, MetadataKey::StorageVersion))? {
        Some(bytes) => {
            let bytes: [u8; 4] =
                bytes.as_slice().try_into().map_err(|_| anyhow!("Malformed storage version record"))?;
            Ok(u32::from_le_bytes(bytes))
        }
        None => Ok(0),
    }
}

/// Writes the storage schema version.
pub(crate) fn set_storage_version(database: &rocksdb::DB, network_id: u16, version: u32) -> Result<()> {
    Ok(database.put(metadata_key(network_id, MetadataKey::StorageVersion), version.to_le_bytes())?)
}

/// Brings the database up to [`STORAGE_VERSION`], refusing one written by a newer build.
///
/// Migrations are applied one at a time and in order, and the version is only advanced once a
/// migration has finished, so an interrupted run repeats the migration it was in the middle of.
///
/// In [`MigrationMode::OnOpen`], a migration that would change existing data is not applied;
/// this returns an error naming it and asking for an explicit run. Migrations with nothing to do
/// are applied in passing, so a database that a newer build merely needs to stamp is stamped.
pub(crate) fn migrate_storage(database: &rocksdb::DB, network_id: u16, mode: MigrationMode) -> Result<()> {
    let mut version = get_storage_version(database, network_id)?;

    // A database from the future cannot be read safely, and the failure would otherwise be
    // silent and data-dependent rather than immediate.
    if version > STORAGE_VERSION {
        bail!(
            "This ledger was written by a newer version of snarkVM (storage schema v{version}; this build \
             understands v{STORAGE_VERSION}). Upgrade snarkVM, or resync from genesis."
        );
    }

    for migration in migrations() {
        if migration.from < version {
            continue;
        }
        debug_assert_eq!(migration.from, version, "storage migrations must be contiguous");

        let has_work = (migration.has_work)(database, network_id)?;
        if has_work && mode == MigrationMode::OnOpen {
            bail!(
                "This ledger has storage schema v{version} and must be migrated to v{} before it can be opened: \
                 {}. Run the storage migration explicitly (`RocksDB::migrate`, which snarkOS exposes as \
                 `snarkos db migrate`).",
                migration.from + 1,
                migration.description,
            );
        }
        if has_work {
            tracing::info!(
                "Migrating the storage schema from v{version} to v{}: {}",
                version + 1,
                migration.description
            );
            (migration.apply)(database, network_id)?;
        }
        version += 1;
        set_storage_version(database, network_id, version)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::rocksdb::{Database, ProgramMap, RocksDB, TestMap};

    use aleo_std::StorageMode;

    const NETWORK_ID: u16 = 0;

    /// A raw key under the given map's prefix.
    fn raw_key(map_id: MapID, suffix: &[u8]) -> Vec<u8> {
        let mut key = map_prefix(NETWORK_ID, map_id).to_vec();
        key.extend_from_slice(suffix);
        key
    }

    /// Writes a raw entry under the given map's prefix.
    fn put_raw(database: &rocksdb::DB, map_id: MapID, suffix: &[u8]) {
        database.put(raw_key(map_id, suffix), b"value").unwrap();
    }

    fn contains_raw(database: &rocksdb::DB, map_id: MapID, suffix: &[u8]) -> bool {
        database.get(raw_key(map_id, suffix)).unwrap().is_some()
    }

    #[test]
    fn test_prefix_range() {
        let (from, to) = prefix_range(&[0, 0, 5, 0]);
        assert_eq!(from, vec![0, 0, 5, 0]);
        assert_eq!(to, vec![0, 0, 5, 1]);

        // The successor carries across bytes.
        let (_, to) = prefix_range(&[0, 0, 0xFF, 0xFF]);
        assert_eq!(to, vec![0, 1, 0, 0]);

        // A map ID whose little-endian bytes end in 0xFF is bounded correctly, where `id + 1`
        // would not be.
        let (from, to) = prefix_range(&[1, 0, 0xFF, 0]);
        assert_eq!(from, vec![1, 0, 0xFF, 0]);
        assert_eq!(to, vec![1, 0, 0xFF, 1]);
    }

    #[test]
    fn test_fresh_database_is_stamped_on_open() {
        let db = RocksDB::open(NETWORK_ID, StorageMode::new_test(None)).unwrap();
        assert_eq!(get_storage_version(&db, NETWORK_ID).unwrap(), STORAGE_VERSION);
    }

    #[test]
    fn test_newer_version_is_refused() {
        let db = RocksDB::open(NETWORK_ID, StorageMode::new_test(None)).unwrap();
        set_storage_version(&db, NETWORK_ID, STORAGE_VERSION + 1).unwrap();
        for mode in [MigrationMode::OnOpen, MigrationMode::Explicit] {
            let error = migrate_storage(&db, NETWORK_ID, mode).unwrap_err().to_string();
            assert!(error.contains("newer version"), "{error}");
        }
        // The record is left alone.
        assert_eq!(get_storage_version(&db, NETWORK_ID).unwrap(), STORAGE_VERSION + 1);
    }

    #[test]
    fn test_v0_without_history_is_stamped_on_open() {
        let db = RocksDB::open(NETWORK_ID, StorageMode::new_test(None)).unwrap();
        // Roll the database back to an unstamped state, with data in an ordinary map.
        database_delete_version(&db);
        put_raw(&db, MapID::Test(TestMap::Test), b"ordinary");

        migrate_storage(&db, NETWORK_ID, MigrationMode::OnOpen).unwrap();
        assert_eq!(get_storage_version(&db, NETWORK_ID).unwrap(), STORAGE_VERSION);
        assert!(contains_raw(&db, MapID::Test(TestMap::Test), b"ordinary"));
    }

    #[test]
    fn test_v0_with_history_needs_an_explicit_migration() {
        let db = RocksDB::open(NETWORK_ID, StorageMode::new_test(None)).unwrap();
        database_delete_version(&db);
        put_raw(&db, MapID::Test(TestMap::Test), b"ordinary");
        put_raw(&db, MapID::Retired(RetiredMap::MappingUpdate), b"update");
        put_raw(&db, MapID::Retired(RetiredMap::MappingUpdateHeights), b"heights");

        // Opening refuses, and changes nothing.
        let error = migrate_storage(&db, NETWORK_ID, MigrationMode::OnOpen).unwrap_err().to_string();
        assert!(error.contains("must be migrated to v1"), "{error}");
        assert_eq!(get_storage_version(&db, NETWORK_ID).unwrap(), 0);
        assert!(contains_raw(&db, MapID::Retired(RetiredMap::MappingUpdate), b"update"));

        // An explicit run drops the retired tables, keeps everything else, and stamps.
        migrate_storage(&db, NETWORK_ID, MigrationMode::Explicit).unwrap();
        assert_eq!(get_storage_version(&db, NETWORK_ID).unwrap(), STORAGE_VERSION);
        assert!(!contains_raw(&db, MapID::Retired(RetiredMap::MappingUpdate), b"update"));
        assert!(!contains_raw(&db, MapID::Retired(RetiredMap::MappingUpdateHeights), b"heights"));
        assert!(contains_raw(&db, MapID::Test(TestMap::Test), b"ordinary"));

        // Once migrated, opening is fine again and repeating is a no-op.
        migrate_storage(&db, NETWORK_ID, MigrationMode::OnOpen).unwrap();
        assert_eq!(get_storage_version(&db, NETWORK_ID).unwrap(), STORAGE_VERSION);
    }

    #[test]
    fn test_drop_history_tables_only_touches_the_retired_prefixes() {
        let db = RocksDB::open(NETWORK_ID, StorageMode::new_test(None)).unwrap();
        // Neighbouring prefixes: the map IDs just below and above each retired one, and the
        // same map ID on another network.
        let below = MapID::Program(ProgramMap::KeyValueID);
        let above = MapID::Program(ProgramMap::StakingRewards);
        put_raw(&db, below, b"below");
        put_raw(&db, above, b"above");
        let other_network = {
            let mut key = map_prefix(NETWORK_ID + 1, MapID::Retired(RetiredMap::MappingUpdate)).to_vec();
            key.extend_from_slice(b"other");
            key
        };
        db.put(&other_network, b"value").unwrap();
        // Retired entries with an empty suffix and with a long one.
        put_raw(&db, MapID::Retired(RetiredMap::MappingUpdate), b"");
        put_raw(&db, MapID::Retired(RetiredMap::MappingUpdate), &[0xFF; 64]);
        put_raw(&db, MapID::Retired(RetiredMap::MappingUpdateHeights), b"h");

        assert!(drop_history_tables::has_work(&db, NETWORK_ID).unwrap());
        drop_history_tables::apply(&db, NETWORK_ID).unwrap();
        assert!(!drop_history_tables::has_work(&db, NETWORK_ID).unwrap());

        assert!(!contains_raw(&db, MapID::Retired(RetiredMap::MappingUpdate), b""));
        assert!(!contains_raw(&db, MapID::Retired(RetiredMap::MappingUpdate), &[0xFF; 64]));
        assert!(!contains_raw(&db, MapID::Retired(RetiredMap::MappingUpdateHeights), b"h"));
        assert!(contains_raw(&db, below, b"below"));
        assert!(contains_raw(&db, above, b"above"));
        assert!(db.get(&other_network).unwrap().is_some());
    }

    /// Removes the version record, as a database written before it existed would look.
    fn database_delete_version(database: &rocksdb::DB) {
        database.delete(metadata_key(NETWORK_ID, MetadataKey::StorageVersion)).unwrap();
        assert_eq!(get_storage_version(database, NETWORK_ID).unwrap(), 0);
    }
}
