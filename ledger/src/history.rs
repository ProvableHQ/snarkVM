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

use super::*;

use aleo_std::aleo_ledger_dir;
use std::time::{Duration, Instant};

/// Time between two backfill progress logs.
const PROGRESS_INTERVAL: Duration = Duration::from_secs(60);

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Returns the next block height history indexing will process.
    ///
    /// A height `h` can be served when `h` is strictly less than this value.
    pub fn history_synced_height(&self) -> u32 {
        self.vm.finalize_store().history_synced_height()
    }

    /// Enables or disables history recording for blocks committed after this call.
    ///
    /// Recording writes on this ledger. The history-replay ledger is updated without recording,
    /// because the same block's history is already stored here. Call [`Self::backfill_history`]
    /// first when heights below the current tip are not indexed yet.
    pub fn set_record_history(&self, enabled: bool) {
        self.record_history.store(enabled, Ordering::SeqCst);
        self.vm.finalize_store().set_record_history(enabled);
    }

    /// Rebuilds history from [`Self::history_synced_height`] through the current tip.
    ///
    /// Blocks already indexed are skipped. The replay store is persistent, so a later call
    /// continues at the stored height.
    pub fn backfill_history(&self) -> Result<()> {
        let replay = self.history_replay()?;
        self.import_recorded_heights(&replay)?;

        let tip = self.latest_height();
        let mut progress = BackfillProgress::new(tip);
        while replay.latest_height()? < tip {
            let next = replay.latest_height()? + 1;
            let record = self.history_synced_height() == next;
            let recording = if record { HistoryRecording::Events } else { HistoryRecording::Off };
            let started = Instant::now();
            let block = self.get_block(next)?;
            let read = started.elapsed();
            replay.finalize(block, recording)?;
            let apply = started.elapsed() - read;
            let events = if record { self.import_recorded_heights(&replay)? } else { 0 };
            progress.record(read, apply, started.elapsed() - read - apply, events);
            progress.log_if_due(next);
        }
        Ok(())
    }

    /// Applies `block` to the history replay without recording it.
    ///
    /// Heights the replay has not applied yet are applied first, also without recording.
    pub(crate) fn sync_history_replay_state(&self, block: &Block<N>) -> Result<()> {
        let replay = self.history_replay()?;
        while replay.latest_height()? < block.height() {
            let next = replay.latest_height()? + 1;
            let next_block = if next == block.height() { block.clone() } else { self.get_block(next)? };
            replay.finalize(next_block, HistoryRecording::Off)?;
        }
        Ok(())
    }

    /// Copies recorded history events from the replay onto this ledger, and returns how many
    /// events were copied.
    ///
    /// Stops at the first height that has no events. That height was applied for state only.
    fn import_recorded_heights(&self, replay: &HistoryReplay<N, C>) -> Result<u64> {
        let mut imported = 0u64;
        loop {
            let cursor = self.history_synced_height();
            if cursor > replay.latest_height()? {
                break;
            }
            let events = replay.vm.finalize_store().history_events(cursor)?;
            if events.is_empty() {
                break;
            }
            imported += events.len() as u64;
            self.vm.finalize_store().import_history_events(cursor, events)?;
            self.vm.finalize_store().set_history_synced_height(cursor + 1)?;
        }
        Ok(imported)
    }

    /// Returns the history replay, opening it on the first call.
    fn history_replay(&self) -> Result<HistoryReplay<N, C>> {
        let mut slot = self.history_replay.lock();
        if let Some(replay) = slot.as_ref() {
            return Ok(replay.clone());
        }
        let replay = HistoryReplay::open(self)?;
        *slot = Some(replay.clone());
        Ok(replay)
    }
}

/// A VM that re-finalizes this ledger's blocks to rebuild mapping and staking history.
///
/// Its store, next to the ledger's directory, holds finalize state and the history event log but
/// no blocks. The last finalized height is the height of its latest committee, which finalize
/// writes in the same batch as the block's other state.
#[derive(Clone)]
pub(crate) struct HistoryReplay<N: Network, C: ConsensusStorage<N>> {
    /// The replay VM.
    vm: VM<N, C>,
}

impl<N: Network, C: ConsensusStorage<N>> HistoryReplay<N, C> {
    /// Opens the replay for `ledger`, finalizing the genesis block when the replay is new.
    ///
    /// Programs deployed up to the replay's height are loaded from `ledger`.
    fn open(ledger: &Ledger<N, C>) -> Result<Self> {
        let storage = history_replay_storage(ledger.vm.finalize_store().storage_mode(), N::ID);
        let store = ConsensusStore::<N, C>::open(storage)?;
        let height = store.finalize_store().committee_store().current_height().ok();
        let replay = Self { vm: VM::from_history_replay(store, &ledger.vm, height)? };
        if height.is_none() {
            let recording =
                if ledger.history_synced_height() == 0 { HistoryRecording::Events } else { HistoryRecording::Off };
            replay.finalize(ledger.genesis_block.clone(), recording)?;
        }
        Ok(replay)
    }

    /// Returns the height of the last block the replay finalized.
    fn latest_height(&self) -> Result<u32> {
        self.vm.finalize_store().committee_store().current_height()
    }

    /// Finalizes the next block, recording its history as `recording` selects.
    fn finalize(&self, block: Block<N>, recording: HistoryRecording) -> Result<()> {
        let height = block.height();
        let expected = match self.latest_height() {
            Ok(latest) => latest + 1,
            Err(_) => 0,
        };
        ensure!(height == expected, "The history replay expected block {expected}, found block {height}");

        self.vm.finalize_store().set_history_recording(recording);
        let result = self.vm.replay_finalize(block);
        self.vm.finalize_store().set_history_recording(HistoryRecording::Off);
        result.with_context(|| format!("Failed to replay block {height} for history"))?;

        ensure!(self.latest_height()? == height, "The history replay did not store the committee for block {height}");
        Ok(())
    }
}

/// Throughput and per-phase time of a history backfill since the last progress log.
struct BackfillProgress {
    /// Height the backfill stops at.
    tip: u32,
    /// Start of the current window.
    window_start: Instant,
    /// Blocks applied in the current window.
    blocks: u32,
    /// History events imported in the current window.
    events: u64,
    /// Time spent reading blocks from the primary ledger in the current window.
    read: Duration,
    /// Time spent applying blocks to the replay in the current window.
    apply: Duration,
    /// Time spent importing history onto the primary ledger in the current window.
    import: Duration,
}

impl BackfillProgress {
    /// Starts a progress window for a backfill that stops at `tip`.
    fn new(tip: u32) -> Self {
        Self {
            tip,
            window_start: Instant::now(),
            blocks: 0,
            events: 0,
            read: Duration::ZERO,
            apply: Duration::ZERO,
            import: Duration::ZERO,
        }
    }

    /// Adds one applied block to the current window.
    fn record(&mut self, read: Duration, apply: Duration, import: Duration, events: u64) {
        self.blocks += 1;
        self.events += events;
        self.read += read;
        self.apply += apply;
        self.import += import;
    }

    /// Logs throughput and per-block phase times once [`PROGRESS_INTERVAL`] has passed, then
    /// starts a new window.
    fn log_if_due(&mut self, height: u32) {
        let elapsed = self.window_start.elapsed();
        if elapsed < PROGRESS_INTERVAL || self.blocks == 0 {
            return;
        }
        let blocks = f64::from(self.blocks);
        let rate = blocks / elapsed.as_secs_f64();
        let eta = Duration::from_secs_f64(f64::from(self.tip.saturating_sub(height)) / rate);
        let per_block_ms = |total: Duration| total.as_secs_f64() * 1000.0 / blocks;
        info!(
            "Backfilled history to block {height}/{} ({rate:.1} blocks/s, ETA {}); per block: read {:.1} ms, apply {:.1} ms, import {:.1} ms, {:.0} events",
            self.tip,
            format_eta(eta),
            per_block_ms(self.read),
            per_block_ms(self.apply),
            per_block_ms(self.import),
            self.events as f64 / blocks,
        );
        *self = Self::new(self.tip);
    }
}

/// Formats a remaining duration as hours and minutes.
fn format_eta(eta: Duration) -> String {
    let minutes = eta.as_secs() / 60;
    format!("{}h{:02}m", minutes / 60, minutes % 60)
}

/// Returns storage for the history-replay ledger beside `mode`'s ledger directory.
fn history_replay_storage(mode: &StorageMode, network_id: u16) -> StorageMode {
    let path = aleo_ledger_dir(network_id, mode);
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("ledger").to_string();
    let mut replay_path = path;
    replay_path.set_file_name(format!("{name}-history-replay"));
    StorageMode::Custom(replay_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{CurrentLedger, CurrentNetwork, sample_ledger};
    use console::prelude::TestRng;
    use snarkvm_ledger_store::helpers::MapRead;

    /// A program whose finalize writes a mapping and rejects a zero value.
    const PARITY_PROGRAM: &str = r"
program history_parity.aleo;

mapping counter:
    key as u8.public;
    value as u64.public;

function set:
    input r0 as u8.public;
    input r1 as u64.public;
    async set r0 r1 into r2;
    output r2 as history_parity.aleo/set.future;

finalize set:
    input r0 as u8.public;
    input r1 as u64.public;
    assert.neq r1 0u64;
    set r1 into counter[r0];
";

    /// Returns every recorded update of a key that is currently mapped, and every staking reward.
    fn recorded_history(ledger: &CurrentLedger) -> (Vec<String>, Vec<String>) {
        let store = ledger.vm.finalize_store();
        let mut mappings = Vec::new();
        for program_id in ["credits.aleo", "history_parity.aleo"] {
            let program_id = ProgramID::<CurrentNetwork>::from_str(program_id).unwrap();
            for mapping_name in store.get_mapping_names_confirmed(&program_id).unwrap().unwrap() {
                for (key, _) in store.get_mapping_confirmed(program_id, mapping_name).unwrap() {
                    let heights = store.get_mapping_update_heights(program_id, mapping_name, key.clone()).unwrap();
                    for height in heights.map(|heights| heights.into_owned()).unwrap_or_default() {
                        let value = store
                            .get_historical_mapping_value(program_id, mapping_name, key.clone(), height)
                            .unwrap()
                            .map(|value| value.into_owned());
                        mappings.push(format!("{program_id}/{mapping_name}[{key}] at {height}: {value:?}"));
                    }
                }
            }
        }
        let mut rewards = store
            .staking_rewards_map()
            .iter_confirmed()
            .map(|(key, value)| format!("{key:?}: {value:?}"))
            .collect_vec();
        rewards.sort();
        (mappings, rewards)
    }

    #[test]
    fn test_backfill_matches_live_recording() {
        let rng = &mut TestRng::default();
        let private_key = console::account::PrivateKey::new(rng).unwrap();
        // `live` records every block when it is added. `backfilled` records genesis only.
        let live = sample_ledger(private_key, rng);
        let backfilled = CurrentLedger::load(live.get_block(0).unwrap(), StorageMode::new_test(None)).unwrap();
        backfilled.set_record_history(false);

        let advance = |transactions: Vec<Transaction<CurrentNetwork>>, rng: &mut TestRng| {
            let block =
                live.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], transactions, rng).unwrap();
            live.advance_to_next_block(&block).unwrap();
            backfilled.advance_to_next_block(&block).unwrap();
            block
        };
        let execute = |key: &str, value: &str, rng: &mut TestRng| {
            let inputs = [Value::<CurrentNetwork>::from_str(key).unwrap(), Value::from_str(value).unwrap()];
            live.vm().execute(&private_key, ("history_parity.aleo", "set"), inputs.iter(), None, 0, None, rng).unwrap()
        };

        let program = Program::<CurrentNetwork>::from_str(PARITY_PROGRAM).unwrap();
        let deployment = live.vm().deploy(&private_key, &program, None, 0, None, rng).unwrap();
        advance(vec![deployment], rng);
        let accepted = execute("1u8", "5u64", rng);
        let rejected = execute("2u8", "0u64", rng);
        let block = advance(vec![accepted, rejected], rng);
        assert_eq!(block.transactions().iter().filter(|transaction| transaction.is_rejected()).count(), 1);
        let update = execute("1u8", "6u64", rng);
        advance(vec![update], rng);

        assert_eq!(backfilled.history_synced_height(), 1);
        backfilled.backfill_history().unwrap();
        assert_eq!(backfilled.history_synced_height(), 4);
        assert_eq!(live.history_synced_height(), 4);

        // The replay's finalize state matches the ledger it replayed.
        let replay = backfilled.history_replay().unwrap();
        assert_eq!(replay.latest_height().unwrap(), 3);
        assert_eq!(
            replay.vm.finalize_store().get_checksum_confirmed().unwrap(),
            backfilled.vm.finalize_store().get_checksum_confirmed().unwrap()
        );
        assert_eq!(
            replay.vm.finalize_store().committee_store().current_committee().unwrap(),
            backfilled.latest_committee().unwrap()
        );

        // Backfilled history matches history recorded while the blocks were added.
        let (mappings, rewards) = recorded_history(&backfilled);
        assert!(mappings.iter().any(|update| update.contains("history_parity.aleo/counter[1u8] at 3")));
        assert!(!rewards.is_empty());
        assert_eq!((mappings, rewards), recorded_history(&live));

        // The replay only accepts the block after its latest height.
        let error = replay.finalize(live.get_block(2).unwrap(), HistoryRecording::Off).unwrap_err().to_string();
        assert!(error.contains("expected block 4, found block 2"), "{error}");

        // A replay VM loads the programs deployed up to its height.
        let program_id = ProgramID::from_str("history_parity.aleo").unwrap();
        let open_at = |height: Option<u32>| {
            let store = ConsensusStore::open(StorageMode::new_test(None)).unwrap();
            VM::from_history_replay(store, &backfilled.vm, height).unwrap().contains_program(&program_id)
        };
        assert!(!open_at(None));
        assert!(!open_at(Some(0)));
        assert!(open_at(Some(1)));

        // A reopened replay resumes, then finalizes a later call into the deployed program.
        drop(replay);
        *backfilled.history_replay.lock() = None;
        backfilled.set_record_history(true);
        let update = execute("2u8", "7u64", rng);
        advance(vec![update], rng);
        let replay = backfilled.history_replay().unwrap();
        assert_eq!(replay.latest_height().unwrap(), 4);
        assert_eq!(
            replay.vm.finalize_store().get_checksum_confirmed().unwrap(),
            backfilled.vm.finalize_store().get_checksum_confirmed().unwrap()
        );
        assert_eq!(backfilled.history_synced_height(), 5);
        assert_eq!(recorded_history(&backfilled), recorded_history(&live));
    }

    #[test]
    fn test_backfill_resumes_after_unrecorded_blocks() {
        let rng = &mut TestRng::default();
        let private_key = console::account::PrivateKey::new(rng).unwrap();
        let ledger = sample_ledger(private_key, rng);
        // Genesis was recorded. The next block is applied with recording off.
        assert_eq!(ledger.history_synced_height(), 1);
        ledger.vm.finalize_store().set_record_history(false);

        let block = ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![], rng).unwrap();
        ledger.advance_to_next_block(&block).unwrap();
        assert_eq!(ledger.latest_height(), 1);
        assert_eq!(ledger.history_synced_height(), 1);
        let program_id = ProgramID::<crate::test_helpers::CurrentNetwork>::from_str("credits.aleo").unwrap();
        let mapping_name = Identifier::from_str("metadata").unwrap();
        let key = Plaintext::from_str("aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc").unwrap();
        assert!(
            ledger
                .vm
                .finalize_store()
                .get_historical_mapping_value(program_id, mapping_name, key.clone(), 1)
                .unwrap_err()
                .to_string()
                .contains("not in the history index")
        );

        ledger.backfill_history().unwrap();
        assert_eq!(ledger.history_synced_height(), 2);
        let value =
            ledger.vm.finalize_store().get_historical_mapping_value(program_id, mapping_name, key, 1).unwrap().unwrap();
        assert_eq!(&*value, &Value::try_from("4u32").unwrap());

        // A second call does not move the cursor.
        ledger.backfill_history().unwrap();
        assert_eq!(ledger.history_synced_height(), 2);
    }

    #[test]
    fn test_format_eta() {
        assert_eq!(format_eta(Duration::from_secs(59)), "0h00m");
        assert_eq!(format_eta(Duration::from_secs(3 * 3600 + 7 * 60 + 5)), "3h07m");
        assert_eq!(format_eta(Duration::from_secs(1190 * 3600)), "1190h00m");
    }
}
