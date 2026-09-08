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

#![forbid(unsafe_code)]

const GAUGE_NAMES: &[&str] = &[
    committee::TOTAL_STAKE,
    rocksdb::COMPACTION_PENDING,
    rocksdb::ESTIMATE_PENDING_COMPACTION_BYTES,
    rocksdb::NUM_RUNNING_COMPACTIONS,
    rocksdb::NUM_RUNNING_FLUSHES,
    rocksdb::MEM_TABLE_FLUSH_PENDING,
    rocksdb::TOTAL_SST_FILES_SIZE,
    rocksdb::LIVE_SST_FILES_SIZE,
    rocksdb::ESTIMATE_NUM_KEYS,
    rocksdb::NUM_SNAPSHOTS,
    rocksdb::NUM_FILES_AT_LEVEL[0],
    rocksdb::NUM_FILES_AT_LEVEL[1],
    rocksdb::NUM_FILES_AT_LEVEL[2],
    rocksdb::NUM_FILES_AT_LEVEL[3],
    rocksdb::NUM_FILES_AT_LEVEL[4],
    rocksdb::NUM_FILES_AT_LEVEL[5],
    rocksdb::NUM_FILES_AT_LEVEL[6],
    vm::CHECK_TRANSACTION_IN_FLIGHT,
    vm::PREPARE_FOR_SPECULATE_IN_FLIGHT,
    vm::ATOMIC_SPECULATE_IN_FLIGHT,
    vm::SPECULATE_IN_FLIGHT,
];

pub mod committee {
    pub const TOTAL_STAKE: &str = "snarkvm_ledger_committee_total_stake";
}

/// VM verification and speculation metrics.
///
/// Overlay `CHECK_TRANSACTION_DURATION_SECONDS{cache="hit"}` with host CPU and
/// `PREPARE_FOR_SPECULATE_IN_FLIGHT` to test whether broadcast verification
/// stalls during block construction. Compare `cache="hit"` vs `cache="miss"`
/// to separate duplicate proof work from contention on the cheap path.
pub mod vm {
    use std::{cell::Cell, time::Instant};

    /// Concurrent `VM::check_transaction` calls.
    pub const CHECK_TRANSACTION_IN_FLIGHT: &str = "snarkvm_vm_check_transaction_in_flight";
    /// Wall time of `VM::check_transaction` in seconds, labeled by `cache`.
    ///
    /// Label values: `hit` (proof skipped), `miss` (proof verified), `pre_cache` (failed before the cache lookup).
    pub const CHECK_TRANSACTION_DURATION_SECONDS: &str = "snarkvm_vm_check_transaction_duration_seconds";
    /// `check_transaction` calls that skipped proof verification via the partial-verification cache.
    pub const CHECK_TRANSACTION_CACHE_HIT_TOTAL: &str = "snarkvm_vm_check_transaction_cache_hit_total";
    /// `check_transaction` calls that ran proof verification.
    pub const CHECK_TRANSACTION_CACHE_MISS_TOTAL: &str = "snarkvm_vm_check_transaction_cache_miss_total";
    /// Concurrent `VM::prepare_for_speculate` calls (block-template verification).
    pub const PREPARE_FOR_SPECULATE_IN_FLIGHT: &str = "snarkvm_vm_prepare_for_speculate_in_flight";
    /// Wall time of `VM::prepare_for_speculate` in seconds.
    pub const PREPARE_FOR_SPECULATE_DURATION_SECONDS: &str = "snarkvm_vm_prepare_for_speculate_duration_seconds";
    /// Concurrent `VM::atomic_speculate_inner` calls (finalize dry-run).
    pub const ATOMIC_SPECULATE_IN_FLIGHT: &str = "snarkvm_vm_atomic_speculate_in_flight";
    /// Wall time of `VM::atomic_speculate_inner` in seconds.
    pub const ATOMIC_SPECULATE_DURATION_SECONDS: &str = "snarkvm_vm_atomic_speculate_duration_seconds";
    /// Concurrent `VM::speculate` calls (prepare_for_speculate + atomic_speculate).
    pub const SPECULATE_IN_FLIGHT: &str = "snarkvm_vm_speculate_in_flight";
    /// Wall time of `VM::speculate` in seconds.
    pub const SPECULATE_DURATION_SECONDS: &str = "snarkvm_vm_speculate_duration_seconds";

    /// Increments an in-flight gauge until dropped, then records elapsed seconds.
    pub struct TimedInFlight {
        gauge_name: &'static str,
        histogram_name: &'static str,
        start: Instant,
    }

    impl TimedInFlight {
        /// Starts an in-flight measurement for the given gauge and histogram.
        #[must_use]
        pub fn enter(gauge_name: &'static str, histogram_name: &'static str) -> Self {
            super::increment_gauge(gauge_name, 1.0);
            Self { gauge_name, histogram_name, start: Instant::now() }
        }
    }

    impl Drop for TimedInFlight {
        fn drop(&mut self) {
            super::decrement_gauge(self.gauge_name, 1.0);
            super::histogram(self.histogram_name, self.start.elapsed().as_secs_f64());
        }
    }

    /// Tracks `check_transaction` in-flight count, duration, and cache hit/miss.
    pub struct TimedCheckTransaction {
        start: Instant,
        cache: Cell<&'static str>,
    }

    impl TimedCheckTransaction {
        /// Starts a `check_transaction` measurement.
        #[must_use]
        pub fn enter() -> Self {
            super::increment_gauge(CHECK_TRANSACTION_IN_FLIGHT, 1.0);
            Self { start: Instant::now(), cache: Cell::new("pre_cache") }
        }

        /// Records whether this call skipped proof verification.
        pub fn set_cache_hit(&self, hit: bool) {
            self.cache.set(if hit { "hit" } else { "miss" });
        }
    }

    impl Drop for TimedCheckTransaction {
        fn drop(&mut self) {
            super::decrement_gauge(CHECK_TRANSACTION_IN_FLIGHT, 1.0);
            let label = self.cache.get();
            super::histogram_label(
                CHECK_TRANSACTION_DURATION_SECONDS,
                "cache",
                label.to_string(),
                self.start.elapsed().as_secs_f64(),
            );
            match label {
                "hit" => super::increment_counter(CHECK_TRANSACTION_CACHE_HIT_TOTAL),
                "miss" => super::increment_counter(CHECK_TRANSACTION_CACHE_MISS_TOTAL),
                _ => {}
            }
        }
    }
}

/// RocksDB internal database metrics.
///
/// Polled and published by calling `BlockStore::export_rocksdb_metrics()` from an existing
/// background loop (e.g. the auto-checkpoint task in snarkOS). All sizes are in bytes;
/// counts are dimensionless. Requires the `rocks` and `metrics` features on `snarkvm-ledger-store`.
pub mod rocksdb {
    /// 1 if a compaction is pending (background compaction requested but not yet running), else 0.
    pub const COMPACTION_PENDING: &str = "snarkvm_rocksdb_compaction_pending";
    /// Estimated total bytes of data to be compacted. A sustained non-zero value signals backpressure.
    pub const ESTIMATE_PENDING_COMPACTION_BYTES: &str = "snarkvm_rocksdb_estimate_pending_compaction_bytes";
    /// Number of compactions currently running in the background.
    pub const NUM_RUNNING_COMPACTIONS: &str = "snarkvm_rocksdb_num_running_compactions";
    /// Number of memtable flushes currently running.
    pub const NUM_RUNNING_FLUSHES: &str = "snarkvm_rocksdb_num_running_flushes";
    /// 1 if a memtable flush is pending (memtable full but flush not yet started), else 0.
    pub const MEM_TABLE_FLUSH_PENDING: &str = "snarkvm_rocksdb_mem_table_flush_pending";
    /// Total size of all SST files on disk (includes files pending deletion).
    pub const TOTAL_SST_FILES_SIZE: &str = "snarkvm_rocksdb_total_sst_files_size_bytes";
    /// Size of live (referenced) SST files only.
    pub const LIVE_SST_FILES_SIZE: &str = "snarkvm_rocksdb_live_sst_files_size_bytes";
    /// Estimated number of keys in the database.
    pub const ESTIMATE_NUM_KEYS: &str = "snarkvm_rocksdb_estimate_num_keys";
    /// Number of snapshots currently held (non-zero blocks deletion of old SST files).
    pub const NUM_SNAPSHOTS: &str = "snarkvm_rocksdb_num_snapshots";
    /// Number of SST files per LSM level (levels 0–6).
    pub const NUM_FILES_AT_LEVEL: [&str; 7] = [
        "snarkvm_rocksdb_num_files_at_level0",
        "snarkvm_rocksdb_num_files_at_level1",
        "snarkvm_rocksdb_num_files_at_level2",
        "snarkvm_rocksdb_num_files_at_level3",
        "snarkvm_rocksdb_num_files_at_level4",
        "snarkvm_rocksdb_num_files_at_level5",
        "snarkvm_rocksdb_num_files_at_level6",
    ];
}

/// Registers all snarkVM metrics.
pub fn register_metrics() {
    for name in GAUGE_NAMES {
        register_gauge(name);
    }
}

/******** Counter ********/

/// Registers a counter with the given name.
pub fn register_counter(name: &'static str) {
    let _counter = ::metrics::counter!(name);
}

/// Updates a counter with the given name to the given value.
///
/// Counters represent a single monotonic value, which means the value can only be incremented,
/// not decremented, and always starts out with an initial value of zero.
pub fn counter<V: Into<u64>>(name: &'static str, value: V) {
    let counter = ::metrics::counter!(name);
    counter.absolute(value.into());
}

/// Increments a counter with the given name by one.
///
/// Counters represent a single monotonic value, which means the value can only be incremented,
/// not decremented, and always starts out with an initial value of zero.
pub fn increment_counter(name: &'static str) {
    let counter = ::metrics::counter!(name);
    counter.increment(1);
}

/******** Gauge ********/

/// Registers a gauge with the given name.
pub fn register_gauge(name: &'static str) {
    let _gauge = ::metrics::gauge!(name);
}

/// Updates a gauge with the given name to the given value.
///
/// Gauges represent a single value that can go up or down over time,
/// and always starts out with an initial value of zero.
pub fn gauge<V: Into<f64>>(name: &'static str, value: V) {
    let gauge = ::metrics::gauge!(name);
    gauge.set(value.into());
}

/// Increments a gauge with the given name by the given value.
///
/// Gauges represent a single value that can go up or down over time,
/// and always starts out with an initial value of zero.
pub fn increment_gauge<V: Into<f64>>(name: &'static str, value: V) {
    let gauge = ::metrics::gauge!(name);
    gauge.increment(value.into());
}

/// Decrements a gauge with the given name by the given value.
///
/// Gauges represent a single value that can go up or down over time,
/// and always starts out with an initial value of zero.
pub fn decrement_gauge<V: Into<f64>>(name: &'static str, value: V) {
    let gauge = ::metrics::gauge!(name);
    gauge.decrement(value.into());
}

/******** Histogram ********/

/// Registers a histogram with the given name.
pub fn register_histogram(name: &'static str) {
    let _histogram = ::metrics::histogram!(name);
}

/// Updates a histogram with the given name to the given value.
pub fn histogram<V: Into<f64>>(name: &'static str, value: V) {
    let histogram = ::metrics::histogram!(name);
    histogram.record(value.into());
}

pub fn histogram_label<V: Into<f64>>(name: &'static str, label_key: &'static str, label_value: String, value: V) {
    ::metrics::histogram!(name, label_key => label_value).record(value.into());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_in_flight_drop_does_not_panic() {
        register_metrics();
        let _guard = vm::TimedInFlight::enter(vm::SPECULATE_IN_FLIGHT, vm::SPECULATE_DURATION_SECONDS);
    }

    #[test]
    fn timed_check_transaction_records_hit_and_miss() {
        register_metrics();
        {
            let miss = vm::TimedCheckTransaction::enter();
            miss.set_cache_hit(false);
        }
        {
            let hit = vm::TimedCheckTransaction::enter();
            hit.set_cache_hit(true);
        }
        let _pre_cache = vm::TimedCheckTransaction::enter();
    }
}
