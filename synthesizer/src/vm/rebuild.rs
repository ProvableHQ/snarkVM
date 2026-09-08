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

//! Rebuilds the finalize state by replaying the blocks already in storage.
//!
//! Discarding and replaying, rather than repairing the historical mapping entries in place, because
//! the two height encodings that shipped collide: `LE(256)` and `BE(65_536)` are the same four
//! bytes, so a key written at both heights kept only the later value. That loss happened on the
//! running node, and no repair that relocates surviving entries can undo it. Replaying recomputes
//! the values instead of moving them, so it fills those holes rather than inheriting them.
//!
//! See `snarkvm_ledger_store::helpers::rocksdb::internal::history_rebuild` for the storage side.

use super::*;

use snarkvm_ledger_store::helpers::rocksdb::{self, ConsensusDB};

use snarkvm_utilities::defer;

use std::time::{Duration, Instant};

/// How often to report progress.
const REPORT_INTERVAL: Duration = Duration::from_secs(30);

/// Formats a duration as `1h23m`, `23m45s`, or `45s`.
fn human_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    match (seconds / 3600, (seconds % 3600) / 60, seconds % 60) {
        (0, 0, s) => format!("{s}s"),
        (0, m, s) => format!("{m}m{s:02}s"),
        (h, m, _) => format!("{h}h{m:02}m"),
    }
}

/// Progress across a replay, reported on a timer.
struct Progress {
    /// The first height this run replays.
    from: u32,
    /// The last height this run replays.
    to: u32,
    /// When the run began, used for the overall rate.
    started: Instant,
    /// The start of the current reporting window.
    window_started: Instant,
    /// The height the current window started from, one below the first block it covers.
    window_from: u32,
}

impl Progress {
    fn new(from: u32, to: u32) -> Self {
        let now = Instant::now();
        // One below the first block, since `from` is replayed *within* the opening window rather
        // than before it. Counting from `from` itself loses a block, which on an archive replay
        // slow enough to manage one block per window makes the first estimate -- the one an
        // operator reads while deciding on a maintenance window -- print "unknown".
        Self { from, to, started: now, window_started: now, window_from: from.saturating_sub(1) }
    }

    /// Reports progress if the interval has elapsed, and opens a new window if it did.
    ///
    /// The estimate comes from the most recent window rather than the run as a whole. Early blocks
    /// are near-empty and late ones carry the chain's real transaction load, so an average taken
    /// from genesis is optimistic for hours -- which is the opposite of what an operator planning a
    /// maintenance window needs from it.
    fn report(&mut self, height: u32) {
        if self.window_started.elapsed() < REPORT_INTERVAL {
            return;
        }
        let done = height.saturating_sub(self.from) + 1;
        let total = self.to.saturating_sub(self.from) + 1;

        let window_blocks = f64::from(height.saturating_sub(self.window_from));
        let rate = window_blocks / self.window_started.elapsed().as_secs_f64();
        // A window that somehow advanced no blocks would divide by zero on the way to an estimate.
        let remaining = match rate > 0.0 {
            true => human_duration(Duration::from_secs_f64(f64::from(self.to - height) / rate)),
            false => "unknown".to_string(),
        };

        tracing::info!(
            "Rebuilding: block {height}/{} ({:.1}%), {rate:.0} blocks/s, {remaining} remaining, {} elapsed",
            self.to,
            100.0 * f64::from(done) / f64::from(total),
            human_duration(self.started.elapsed()),
        );

        self.window_started = Instant::now();
        self.window_from = height;
    }
}

/// Bound to the RocksDB backend rather than to any `ConsensusStorage`, because the rebuild reaches
/// past the typed store to clear raw key ranges and to record the schema version. On a
/// memory-backed VM those calls would find no such database and open one, then discard the state of
/// a ledger on disk that this VM was never reading.
impl<N: Network> VM<N, ConsensusDB<N>> {
    /// Re-applies a block's finalize operations, without inserting the block.
    ///
    /// Part of the rebuild, and dangerous outside it: applied to a block a running node has already
    /// finalized, it would re-apply that block's mapping updates and append a duplicate historical
    /// entry. Kept on this backend-bound impl rather than the generic one for that reason -- it is
    /// not a general VM operation.
    ///
    /// Inserting the block again is deliberately not done: the blocks are already in storage, and
    /// re-inserting would append to the block Merkle tree a second time.
    ///
    /// # Panics
    /// This function panics if called from an async context.
    #[doc(hidden)]
    #[inline]
    pub fn replay_block(&self, block: &Block<N>) -> Result<()> {
        let sequential_op = SequentialOperation::ReplayBlock(block.clone());
        let Some(SequentialOperationResult::ReplayBlock(ret)) = self.run_sequential_operation(sequential_op) else {
            bail!("Already shutting down");
        };

        ret
    }

    /// Discards the finalize state and rebuilds it by replaying every block in storage.
    ///
    /// Resumable: interrupt it and call it again. The resume point is the committee store's current
    /// height, which ratification writes inside the same atomic batch as the block's mapping
    /// updates. That makes it the state's own account of how far it has got, rather than a cursor
    /// recorded alongside the data it describes and able to disagree with it. `CommitteeStorage`
    /// additionally requires each height to follow the last, so a replay resuming in the wrong
    /// place is refused by the store rather than silently writing over itself.
    ///
    /// The node must not be running: this rewrites the state underneath it, and RocksDB permits a
    /// single writer in any case.
    pub fn rebuild_finalize_state(&self) -> Result<()> {
        let network_id = N::ID;
        let storage_mode = self.finalize_store().storage_mode().clone();
        let database = rocksdb::open_for_rebuild(network_id, storage_mode)?;

        // Restore the schema gate however this returns. The bypass has to outlast the store's own
        // open, which happens before this is called, but it must not outlast the rebuild: a run
        // that fails partway leaves a discarded finalize state behind, and the gate is what stops
        // anything else in this process from opening it.
        defer! {
            rocksdb::disallow_downlevel_open();
        }

        let resuming = rocksdb::is_rebuilding(&database, network_id)?;

        // A ledger already at this schema version has nothing to rebuild, and rebuilding it anyway
        // would discard a healthy finalize state and replay the whole chain to reproduce it. That
        // matters because this is an operator command: running it twice, or wiring it into a
        // startup script, must not take the node offline for a second full pass -- and an
        // interruption during that pass would leave a half-empty store where a complete one stood.
        if !resuming && rocksdb::schema_version(&database, network_id)? >= rocksdb::STORAGE_VERSION {
            tracing::info!(
                "This ledger is already at storage schema v{}; nothing to rebuild",
                rocksdb::STORAGE_VERSION
            );
            return Ok(());
        }

        // The blocks are the source this reads from, so the tip is the extent of the work.
        let tip = self.block_store().current_block_height();

        // Establish that the source is complete before discarding the state built from it.
        //
        // Deliberately a pre-flight rather than left to the replay to discover. A ledger missing a
        // block cannot be rebuilt at all, and finding that out at height 12,000,000 -- with the
        // finalize state already gone -- turns a ledger that merely reads history wrongly into one
        // that cannot serve anything.
        //
        // The tip comes from the block Merkle tree's leaf count and this from the height index, so
        // the two disagreeing is itself the gap. Counted rather than probed height by height: one
        // pass over the smallest map in the store, against `tip` point lookups across the largest.
        let (count, highest) = self
            .block_store()
            .heights()
            .fold((0u32, 0u32), |(count, highest), height| (count.saturating_add(1), highest.max(*height)));
        ensure!(
            count == tip + 1 && highest == tip,
            "This ledger is missing blocks: it holds {count} of the {} up to its tip of {tip}, the \
             highest being {highest}. The finalize state is rebuilt by replaying blocks, so a \
             ledger with gaps in them must be resynced from genesis instead.",
            tip + 1
        );

        // Refuse to discard data this build has no way to write back.
        //
        // The clear works on raw prefixes, so it removes history whether or not this build was
        // compiled to keep any -- but only a `history` build repopulates it. Without this the
        // failure is silent and total: the history is cleared, the replay writes none, and the
        // schema version is stamped over the result, so the next history-enabled node finds an
        // empty map and believes it.
        //
        // Read from the record once a rebuild is under way, because by then the data it describes
        // is gone and an emptied history map is indistinguishable from one that never existed. A
        // run begun by a `history` build must not be finishable by one without it.
        let (needs_history, needs_rewards) = match resuming {
            true => rocksdb::required_features(&database, network_id)?,
            false => {
                (rocksdb::has_history(&database, network_id)?, rocksdb::has_staking_rewards(&database, network_id)?)
            }
        };
        ensure!(
            cfg!(feature = "history") || !needs_history,
            "This ledger holds mapping history, but this build was compiled without the `history` \
             feature and so would discard it without writing any back. Rebuild it with a build that \
             has the features the node runs with."
        );
        ensure!(
            cfg!(feature = "history-staking-rewards") || !needs_rewards,
            "This ledger holds staking rewards history, but this build was compiled without the \
             `history-staking-rewards` feature and so would discard it without writing any back. \
             Rebuild it with a build that has the features the node runs with."
        );

        // Refuse a ledger carrying an upgraded program, before discarding anything.
        //
        // `VM::from` preloads the latest edition of every program, so replaying an *earlier*
        // deployment of an upgraded one hands `Process::finalize_deployment` the wrong stack: for a
        // non-zero edition it diffs the block's mappings against the latest program's, yielding
        // fewer `InitializeMapping` operations than the block records, and for a program with a
        // constructor it runs the upgrade check in reverse. Either way the replay aborts.
        //
        // A pre-flight rather than a mid-run failure. Aborting after the clear would leave the
        // finalize state discarded and the rebuild flagged in progress -- a ledger that serves
        // nothing, from one that only read history wrongly.
        //
        // The fix is to build the process as the replay goes rather than preloading it, which is a
        // change to how a VM is constructed and is deliberately not made here.
        let upgraded = self
            .transaction_store()
            .deployment_store()
            .program_ids_and_latest_editions()
            .find(|(_, edition)| **edition > 0)
            .map(|(program_id, edition)| (program_id.into_owned(), edition.into_owned()));
        ensure!(
            upgraded.is_none(),
            "This ledger carries an upgraded program ({} is at edition {}), which the replay cannot \
             reproduce: the process is loaded with the latest edition of every program, so \
             replaying an earlier deployment of one would be checked against the wrong program. \
             Rebuilding such a ledger needs the process to be built as the replay proceeds.",
            upgraded.as_ref().map(|(id, _)| id.to_string()).unwrap_or_default(),
            upgraded.as_ref().map(|(_, edition)| *edition).unwrap_or_default()
        );

        // Discard the state, unless a previous run already did and was interrupted before finishing.
        // Re-clearing would be harmless but would throw away every block already replayed.
        if !resuming {
            tracing::info!("Discarding the finalize state of {} blocks", tip + 1);
            rocksdb::clear_rebuilt_state(&database, network_id)?;
        }

        // Re-initialize the mappings of 'credits.aleo', which the clear removed. Genesis ratification
        // replaces their contents wholesale and refuses a mapping that does not yet exist, so this
        // has to happen before the first block rather than as part of it.
        let credits = Program::<N>::credits()?;
        for mapping in credits.mappings().values() {
            if !self.finalize_store().contains_mapping_confirmed(credits.id(), mapping.name())? {
                self.finalize_store().initialize_mapping(*credits.id(), *mapping.name())?;
            }
        }

        // Resume after the last height the committee store recorded, or start at genesis.
        let next = match self.finalize_store().committee_store().current_height() {
            Ok(height) => height + 1,
            Err(..) => 0,
        };
        if next > 0 {
            tracing::info!("Resuming an interrupted rebuild at block {next}");
        }

        let mut progress = Progress::new(next, tip);
        for height in next..=tip {
            let Some(hash) = self.block_store().get_block_hash(height)? else {
                bail!(
                    "Block {height} is missing from storage, so the state after it cannot be \
                     reproduced. A ledger with gaps in its blocks must be resynced from genesis."
                );
            };
            let Some(block) = self.block_store().get_block(&hash)? else {
                bail!("Block {height} ({hash}) is missing from storage; the ledger must be resynced from genesis")
            };
            self.replay_block(&block)?;
            progress.report(height);
        }

        // The committee store is written by every block's ratification, so its height disagreeing
        // with the tip means blocks were skipped -- and the rebuild would otherwise go on to stamp
        // the schema version over an incomplete state.
        let rebuilt = self.finalize_store().committee_store().current_height()?;
        ensure!(rebuilt == tip, "Rebuilt the finalize state only to block {rebuilt}, but the ledger's tip is {tip}");

        // Stamped before the flag is cleared. A crash between the two leaves the ledger looking
        // mid-rebuild, so the next run resumes, finds nothing left to replay, and clears the flag;
        // the other order would leave a ledger that reports itself unrebuilt and be cleared and
        // replayed from genesis all over again.
        rocksdb::set_schema_version(&database, network_id, rocksdb::STORAGE_VERSION)?;
        rocksdb::set_rebuilding(&database, network_id, false)?;

        tracing::info!(
            "Rebuilt the finalize state through block {tip} in {}",
            human_duration(progress.started.elapsed())
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_human_duration() {
        assert_eq!(human_duration(Duration::from_secs(0)), "0s");
        assert_eq!(human_duration(Duration::from_secs(45)), "45s");
        assert_eq!(human_duration(Duration::from_secs(60)), "1m00s");
        assert_eq!(human_duration(Duration::from_secs(1425)), "23m45s");
        assert_eq!(human_duration(Duration::from_secs(3600)), "1h00m");
        assert_eq!(human_duration(Duration::from_secs(5000)), "1h23m");
    }
}
