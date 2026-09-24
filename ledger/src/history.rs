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
    /// Blocks already indexed are skipped. The replay ledger is persistent, so a later call
    /// continues at the stored height.
    pub fn backfill_history(&self) -> Result<()> {
        let replay = self.history_replay_ledger()?;
        self.import_recorded_heights(&replay)?;

        let tip = self.latest_height();
        let mut progress = BackfillProgress::new(tip);
        while replay.latest_height() < tip {
            let next = replay.latest_height() + 1;
            let record = self.history_synced_height() == next;
            replay.vm.finalize_store().set_record_history(record);
            let started = Instant::now();
            let block = self.get_block(next)?;
            let read = started.elapsed();
            replay.advance_to_next_block(&block)?;
            let apply = started.elapsed() - read;
            replay.vm.finalize_store().set_record_history(false);
            let events = if record { self.import_recorded_heights(&replay)? } else { 0 };
            progress.record(read, apply, started.elapsed() - read - apply, events);
            progress.log_if_due(next);
        }
        Ok(())
    }

    /// Applies `block` to the history-replay ledger without recording it.
    ///
    /// Heights the replay has not applied yet are applied first, also without recording.
    pub(crate) fn sync_history_replay_state(&self, block: &Block<N>) -> Result<()> {
        let replay = self.history_replay_ledger()?;
        while replay.latest_height() < block.height() {
            let next = replay.latest_height() + 1;
            replay.vm.finalize_store().set_record_history(false);
            let next_block = if next == block.height() { block.clone() } else { self.get_block(next)? };
            replay.advance_to_next_block(&next_block)?;
        }
        Ok(())
    }

    /// Copies recorded history events from the replay ledger onto this ledger, and returns how many
    /// events were copied.
    ///
    /// Stops at the first height that has no events. That height was applied for state only.
    fn import_recorded_heights(&self, replay: &Ledger<N, C>) -> Result<u64> {
        let mut imported = 0u64;
        loop {
            let cursor = self.history_synced_height();
            if cursor > replay.latest_height() {
                break;
            }
            let events = replay.vm.finalize_store().history_events(cursor)?;
            if events.is_empty() {
                break;
            }
            self.vm.finalize_store().import_history_events(cursor, &events)?;
            self.vm.finalize_store().set_history_synced_height(cursor + 1)?;
            imported += events.len() as u64;
        }
        Ok(imported)
    }

    /// Returns the history-replay ledger, opening it on the first call.
    fn history_replay_ledger(&self) -> Result<Ledger<N, C>> {
        let mut slot = self.history_replay.lock();
        if let Some(replay) = slot.as_ref() {
            return Ok(replay.clone());
        }
        let replay = self.open_history_replay()?;
        *slot = Some(replay.clone());
        Ok(replay)
    }

    /// Opens the history-replay ledger next to this ledger's directory.
    fn open_history_replay(&self) -> Result<Ledger<N, C>> {
        let storage = history_replay_storage(self.vm.finalize_store().storage_mode(), N::ID);
        let record_genesis = self.history_synced_height() == 0;
        #[cfg(feature = "dev-committee")]
        let replay = Ledger::load_unchecked_inner(self.genesis_block.clone(), storage, None, record_genesis)?;
        #[cfg(not(feature = "dev-committee"))]
        let replay = Ledger::load_unchecked_inner(self.genesis_block.clone(), storage, record_genesis)?;
        // Later blocks are recorded one at a time by [`Self::backfill_history`].
        replay.vm.finalize_store().set_record_history(false);
        Ok(replay)
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
    use crate::test_helpers::sample_ledger;
    use console::prelude::TestRng;

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
