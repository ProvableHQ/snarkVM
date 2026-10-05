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

mod json;

use super::*;

use aleo_std::aleo_ledger_dir;
use snarkvm_ledger_store::HistoryScope;
use std::time::Duration;

/// Time between two JSON-import progress logs.
const PROGRESS_INTERVAL: Duration = Duration::from_secs(60);

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Returns the next block height history indexing will process.
    ///
    /// A height `h` can be served when `h` is strictly less than this value.
    pub fn history_synced_height(&self) -> u32 {
        self.vm.finalize_store().history_synced_height()
    }

    /// Records mapping history only within `scope` from now on, and stores the scope with the
    /// history. Staking rewards are recorded for every staker.
    ///
    /// Fails if the history was recorded for a different scope, including a different program
    /// start height. History that an earlier build indexed without a scope covers every mapping.
    /// Call [`Self::reset_history`] to change the scope.
    pub fn configure_history(&self, scope: HistoryScope<N>) -> Result<()> {
        let store = self.vm.finalize_store();
        match store.stored_history_scope()? {
            Some(stored) => ensure!(
                stored == scope,
                "History is recorded for {stored}, not {scope}; clean the history to change what it records"
            ),
            None => {
                let cursor = self.history_synced_height();
                ensure!(
                    cursor == 0,
                    "History below block {cursor} is recorded for every program, not {scope}; clean the history to change what it records"
                );
                store.store_history_scope(&scope)?;
            }
        }
        store.set_history_scope(Some(scope));
        Ok(())
    }

    /// Returns whether this ledger records the history of every mapping of `program_id`.
    pub fn records_history_of(&self, program_id: &ProgramID<N>) -> bool {
        self.vm.finalize_store().records_history_of(program_id)
    }

    /// Returns whether this ledger records the history of `program_id/mapping_name`.
    pub fn records_mapping_history_of(&self, program_id: &ProgramID<N>, mapping_name: &Identifier<N>) -> bool {
        self.vm.finalize_store().records_mapping_history_of(program_id, mapping_name)
    }

    /// Returns the block height from which every mapping of `program_id` is recorded.
    ///
    /// `None` when `program_id` is not recorded as a whole program. A single mapping has no start
    /// height.
    pub fn history_program_start_height(&self, program_id: &ProgramID<N>) -> Option<u32> {
        self.vm.finalize_store().history_scope()?.program_start_height(program_id)
    }

    /// Deletes this ledger's history. The history cursor becomes 0 and the stored scope is
    /// removed.
    ///
    /// A leftover history-replay directory from an earlier build is deleted when it is present.
    pub fn reset_history(&self) -> Result<()> {
        self.vm.finalize_store().reset_history()?;
        let replay_path = leftover_history_replay_path(self.vm.finalize_store().storage_mode(), N::ID);
        if replay_path.exists() {
            std::fs::remove_dir_all(&replay_path).with_context(|| {
                format!("Failed to delete the leftover history replay at {}", replay_path.display())
            })?;
        }
        Ok(())
    }

    /// Enables or disables history recording for blocks committed after this call.
    ///
    /// Recording writes on this ledger. Call [`Self::import_history_json`] first when heights
    /// below the current tip are not indexed yet.
    pub fn set_record_history(&self, enabled: bool) {
        self.record_history.store(enabled, Ordering::SeqCst);
        self.vm.finalize_store().set_record_history(enabled);
    }
}

/// Returns the time to process `remaining` blocks at `rate` blocks per second, or `None` if
/// blocks remain and the rate is zero.
fn time_to_finish(remaining: u32, rate: f64) -> Option<Duration> {
    match (remaining, rate > 0.0) {
        (0, _) => Some(Duration::ZERO),
        (_, true) => Some(Duration::from_secs_f64(f64::from(remaining) / rate)),
        (_, false) => None,
    }
}

/// Formats a remaining duration as hours and minutes, or `unknown`.
fn format_eta(eta: Option<Duration>) -> String {
    match eta {
        Some(eta) => {
            let minutes = eta.as_secs() / 60;
            format!("{}h{:02}m", minutes / 60, minutes % 60)
        }
        None => "unknown".to_string(),
    }
}

/// Returns the history-replay directory an earlier build kept beside `mode`'s ledger directory.
fn leftover_history_replay_path(mode: &StorageMode, network_id: u16) -> std::path::PathBuf {
    let path = aleo_ledger_dir(network_id, mode);
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("ledger").to_string();
    let mut replay_path = path;
    replay_path.set_file_name(format!("{name}-history-replay"));
    replay_path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{CurrentNetwork, sample_ledger};
    use console::prelude::TestRng;
    use indexmap::IndexMap;
    use snarkvm_ledger_store::helpers::MapRead;

    #[test]
    fn test_configure_history_requires_a_reset_to_change_scope() {
        let rng = &mut TestRng::default();
        let private_key = console::account::PrivateKey::new(rng).unwrap();
        let ledger = sample_ledger(private_key, rng);
        let credits = ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap();
        let other = ProgramID::<CurrentNetwork>::from_str("other.aleo").unwrap();

        let scope = |pairs: &[(&str, u32)]| {
            HistoryScope::programs(
                pairs
                    .iter()
                    .map(|(id, height)| (ProgramID::from_str(id).unwrap(), *height))
                    .collect::<IndexMap<_, _>>(),
            )
        };

        // Genesis history was recorded for every program, without a stored list.
        assert_eq!(ledger.history_synced_height(), 1);
        let error = ledger.configure_history(scope(&[("credits.aleo", 0)])).unwrap_err().to_string();
        assert!(error.contains("recorded for every program"), "{error}");

        ledger.reset_history().unwrap();
        assert_eq!(ledger.history_synced_height(), 0);
        ledger.configure_history(scope(&[("credits.aleo", 0)])).unwrap();
        ledger.configure_history(scope(&[("credits.aleo", 0)])).unwrap();
        assert!(ledger.records_history_of(&credits));
        assert!(!ledger.records_history_of(&other));
        assert_eq!(ledger.history_program_start_height(&credits), Some(0));
        let error = ledger.configure_history(scope(&[("credits.aleo", 5)])).unwrap_err().to_string();
        assert!(error.contains("recorded for [(credits.aleo, 0)], not [(credits.aleo, 5)]"), "{error}");
        assert!(error.contains("clean the history"), "{error}");
        let error = ledger.configure_history(scope(&[("credits.aleo", 0), ("other.aleo", 0)])).unwrap_err().to_string();
        assert!(error.contains("not [(credits.aleo, 0), (other.aleo, 0)]"), "{error}");

        // Genesis, then a live block, records staking rewards for the listed programs.
        ledger.import_genesis_history().unwrap();
        ledger.set_record_history(true);
        let block = ledger.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], vec![], rng).unwrap();
        ledger.advance_to_next_block(&block).unwrap();
        assert_eq!(ledger.history_synced_height(), 2);
        assert!(ledger.vm.finalize_store().staking_rewards_map().iter_confirmed().next().is_some());

        ledger.reset_history().unwrap();
        assert_eq!(ledger.history_synced_height(), 0);
    }

    #[test]
    fn test_format_eta() {
        assert_eq!(format_eta(Some(Duration::from_secs(59))), "0h00m");
        assert_eq!(format_eta(Some(Duration::from_secs(3 * 3600 + 7 * 60 + 5))), "3h07m");
        assert_eq!(format_eta(Some(Duration::from_secs(1190 * 3600))), "1190h00m");
        assert_eq!(format_eta(None), "unknown");
    }

    #[test]
    fn test_time_to_finish() {
        assert_eq!(time_to_finish(0, 0.0), Some(Duration::ZERO));
        assert_eq!(time_to_finish(100, 50.0), Some(Duration::from_secs(2)));
        assert_eq!(time_to_finish(100, 0.0), None);
    }
}
