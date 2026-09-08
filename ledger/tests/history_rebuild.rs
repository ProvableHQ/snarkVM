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

//! Integration tests for rebuilding the finalize state by replaying blocks.

use snarkvm_ledger::test_helpers::{CurrentConsensusStore, CurrentLedger, CurrentNetwork};

use aleo_std::StorageMode;
use snarkvm_console::{
    account::PrivateKey,
    prelude::*,
    program::{Identifier, Plaintext, ProgramID, Value},
};
use snarkvm_ledger_store::helpers::rocksdb;

/// Opens the ledger's database and restores the schema gate afterwards.
///
/// `open_for_rebuild` latches the bypass on for the whole process. Left set, it disables
/// `check_storage_version` for every later open in this test binary -- which is how two of these
/// tests came to pass without exercising a rebuild at all.
fn open_db(storage_mode: StorageMode) -> rocksdb::RocksDB {
    let database = rocksdb::open_for_rebuild(CurrentNetwork::ID, storage_mode).unwrap();
    rocksdb::disallow_downlevel_open();
    database
}
use snarkvm_synthesizer::vm::VM;

use std::sync::{Mutex, MutexGuard};

/// Serialises the tests in this file.
///
/// The schema-gate bypass these tests rely on is a single process-wide latch over a shared database
/// registry, so two tests running at once decide for each other whether a freshly created ledger
/// gets its schema version stamped. Left parallel, whether a rebuild actually runs depends on
/// thread interleaving.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The number of blocks each test replays.
///
/// Every block ratifies a block reward, which rewrites `credits.aleo/{committee, delegated, bonded}`
/// in full -- so a chain with no transactions at all still exercises the mappings that carry the
/// overwhelming majority of a real archive node's history.
const NUM_BLOCKS: u32 = 8;

/// The `credits.aleo` mappings a rebuild must reproduce, and the history of each.
const REBUILT_MAPPINGS: &[&str] = &["committee", "delegated", "bonded", "account", "withdraw"];

/// The full history of every key of the given mappings: each key, the heights it changed at, and
/// its value at each of those heights.
type HistorySnapshot = Vec<(String, String, Vec<(u32, String)>)>;

/// Reads back the entire history of the `credits.aleo` mappings a rebuild is expected to reproduce.
///
/// Goes through the public read path rather than the raw map, so the comparison is over what a node
/// would actually serve. Rendered as strings so a mismatch names the key and value that differ.
fn snapshot_history(ledger: &CurrentLedger) -> HistorySnapshot {
    let program_id = ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap();
    let store = ledger.vm().finalize_store();

    let mut snapshot = HistorySnapshot::new();
    for name in REBUILT_MAPPINGS {
        let mapping_name = Identifier::<CurrentNetwork>::from_str(name).unwrap();
        let entries = store.get_mapping_confirmed(program_id, mapping_name).unwrap();

        let mut keys = entries.into_iter().map(|(key, _)| key).collect::<Vec<Plaintext<CurrentNetwork>>>();
        keys.sort_by_key(ToString::to_string);

        for key in keys {
            let heights = store
                .get_mapping_update_heights(program_id, mapping_name, key.clone())
                .unwrap()
                .map(|heights| heights.into_owned())
                .unwrap_or_default();

            let values = heights
                .into_iter()
                .map(|height| {
                    let value: Value<CurrentNetwork> = store
                        .get_historical_mapping_value(program_id, mapping_name, key.clone(), height)
                        .unwrap()
                        .unwrap_or_else(|| panic!("{name}/{key} reports an update at {height} but has no value there"))
                        .into_owned();
                    (height, value.to_string())
                })
                .collect();

            snapshot.push((name.to_string(), key.to_string(), values));
        }
    }

    assert!(!snapshot.is_empty(), "the chain wrote no history, so a rebuild of it would prove nothing");
    snapshot
}

/// Returns a ledger of [`NUM_BLOCKS`] beacon blocks, and the key it was built with.
fn sample_ledger(rng: &mut TestRng) -> (CurrentLedger, StorageMode) {
    let storage_mode = StorageMode::new_test(None);

    let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let store = CurrentConsensusStore::open(storage_mode.clone()).unwrap();
    let genesis = VM::from(store).unwrap().genesis_beacon(&private_key, rng).unwrap();

    let ledger = CurrentLedger::load(genesis, storage_mode.clone()).unwrap();
    for _ in 1..=NUM_BLOCKS {
        let block = ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![], rng).unwrap();
        ledger.advance_to_next_block(&block).unwrap();
    }
    assert_eq!(ledger.latest_height(), NUM_BLOCKS);

    // A ledger built here is stamped at the current schema version the moment it is created: it has
    // no history yet, so the startup gate records the version in passing. Left that way, every
    // rebuild below would take the "already rebuilt, nothing to do" path and each assertion would
    // hold trivially over a chain nothing had touched. Winding the version back is what makes these
    // tests exercise a replay at all -- and the sentinel each test plants is what proves they did,
    // rather than this being trusted to stay true.
    let database = open_db(storage_mode.clone());
    rocksdb::set_schema_version(&database, CurrentNetwork::ID, 0).unwrap();

    (ledger, storage_mode)
}

/// Leaves a mapping behind that no replay would recreate, so a caller can tell whether the finalize
/// state was actually discarded.
fn plant_sentinel(ledger: &CurrentLedger) -> (ProgramID<CurrentNetwork>, Identifier<CurrentNetwork>) {
    let program = ProgramID::<CurrentNetwork>::from_str("not_on_chain.aleo").unwrap();
    let mapping = Identifier::<CurrentNetwork>::from_str("sentinel").unwrap();
    ledger.vm().finalize_store().initialize_mapping(program, mapping).unwrap();
    (program, mapping)
}

/// Returns whether the sentinel planted by [`plant_sentinel`] is still present.
fn sentinel_survives(
    ledger: &CurrentLedger,
    program: ProgramID<CurrentNetwork>,
    mapping: Identifier<CurrentNetwork>,
) -> bool {
    ledger
        .vm()
        .finalize_store()
        .get_mapping_names_confirmed(&program)
        .unwrap()
        .is_some_and(|names| names.contains(&mapping))
}

/// Re-initializes the `credits.aleo` mappings a clear removes.
///
/// Mirrors what the rebuild does before its first block; a test that hand-replays has to do it too,
/// because genesis ratification replaces those mappings wholesale and refuses one that is absent.
fn initialize_credits_mappings(ledger: &CurrentLedger) {
    let credits = snarkvm_synthesizer::program::Program::<CurrentNetwork>::credits().unwrap();
    ledger.vm().finalize_store().initialize_credits_mappings(&credits).unwrap();
}

/// A rebuild must reproduce the history the chain originally wrote, entry for entry.
#[test]
fn test_rebuild_reproduces_history() {
    let _guard = serial();
    let rng = &mut TestRng::default();
    let (ledger, storage_mode) = sample_ledger(rng);

    let expected = snapshot_history(&ledger);
    let (program, mapping) = plant_sentinel(&ledger);

    ledger.vm().rebuild_finalize_state().unwrap();

    // Without this the comparison below would pass on a rebuild that never ran.
    assert!(!sentinel_survives(&ledger, program, mapping), "the rebuild did not discard the finalize state");

    assert_eq!(snapshot_history(&ledger), expected, "the rebuilt history differs from the one the chain wrote");
    assert_eq!(ledger.vm().block_store().current_block_height(), NUM_BLOCKS, "the rebuild changed the block store");

    // A completed rebuild stamps the schema version and clears the in-progress flag, which is what
    // lets a node start again.
    let database = open_db(storage_mode);
    assert_eq!(rocksdb::schema_version(&database, CurrentNetwork::ID).unwrap(), rocksdb::STORAGE_VERSION);
    assert!(!rocksdb::is_rebuilding(&database, CurrentNetwork::ID).unwrap());
}

/// A rebuild interrupted partway must resume where it stopped, not start over or skip ahead.
#[test]
fn test_rebuild_resumes_after_interruption() {
    let _guard = serial();
    let rng = &mut TestRng::default();
    let (ledger, storage_mode) = sample_ledger(rng);

    let expected = snapshot_history(&ledger);

    // Interrupt a rebuild by hand: discard the state, then replay only part of the chain.
    let database = open_db(storage_mode);
    rocksdb::clear_rebuilt_state(&database, CurrentNetwork::ID).unwrap();
    initialize_credits_mappings(&ledger);

    let stopped_at = NUM_BLOCKS / 2;
    for height in 0..=stopped_at {
        let block = ledger.get_block(height).unwrap();
        ledger.vm().replay_block(block).unwrap();
    }
    assert_eq!(ledger.vm().finalize_store().committee_store().current_height().unwrap(), stopped_at);

    // Resuming must not re-clear: the committee store is the resume point, and clearing it would
    // silently throw away the blocks already replayed rather than continuing from them.
    ledger.vm().rebuild_finalize_state().unwrap();

    assert_eq!(snapshot_history(&ledger), expected, "the resumed rebuild differs from the history the chain wrote");
    assert_eq!(rocksdb::schema_version(&database, CurrentNetwork::ID).unwrap(), rocksdb::STORAGE_VERSION);
    assert!(!rocksdb::is_rebuilding(&database, CurrentNetwork::ID).unwrap());
}

/// A rebuild that has already finished must be a no-op, not a second pass over the chain.
///
/// Comparing the history across the two runs would not show the difference -- a second full pass
/// reproduces it exactly. So this leaves a mapping behind that only a clear would remove and that
/// no replay would recreate: if it survives, the second call returned without touching anything.
#[test]
fn test_rebuild_is_idempotent() {
    let _guard = serial();
    let rng = &mut TestRng::default();
    let (ledger, _storage_mode) = sample_ledger(rng);

    // The first call must genuinely rebuild, or the second one proves nothing.
    let (program, mapping) = plant_sentinel(&ledger);
    ledger.vm().rebuild_finalize_state().unwrap();
    assert!(!sentinel_survives(&ledger, program, mapping), "the first rebuild did not discard the finalize state");
    let once = snapshot_history(&ledger);

    let (program, mapping) = plant_sentinel(&ledger);
    ledger.vm().rebuild_finalize_state().unwrap();

    assert!(
        sentinel_survives(&ledger, program, mapping),
        "the second rebuild discarded the finalize state instead of returning early"
    );
    assert_eq!(snapshot_history(&ledger), once);
}
