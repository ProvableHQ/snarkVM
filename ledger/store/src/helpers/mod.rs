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

pub mod memory;
#[cfg(feature = "rocks")]
pub mod rocksdb;

#[cfg(test)]
pub(crate) mod test_helpers;

mod traits;
pub use traits::*;

pub(crate) mod atomic_owner {
    use std::{
        cell::Cell,
        sync::atomic::{AtomicU64, Ordering},
        time::Instant,
    };

    thread_local! {
        static THREAD_KEY: Cell<u64> = const { Cell::new(0) };
    }

    static NEXT_THREAD_KEY: AtomicU64 = AtomicU64::new(1);

    /// Returns a stable nonzero identifier for the current thread.
    pub(crate) fn current_thread_key() -> u64 {
        THREAD_KEY.with(|cell| {
            let mut key = cell.get();
            if key == 0 {
                key = NEXT_THREAD_KEY.fetch_add(1, Ordering::Relaxed);
                // `0` is reserved for "no owner".
                if key == 0 {
                    key = NEXT_THREAD_KEY.fetch_add(1, Ordering::Relaxed);
                }
                cell.set(key);
            }
            key
        })
    }

    /// Records that this thread owns the in-progress atomic batch.
    pub(crate) fn claim(owner: &AtomicU64) {
        owner.store(current_thread_key(), Ordering::Release);
    }

    /// Clears atomic-batch ownership.
    pub(crate) fn release(owner: &AtomicU64) {
        owner.store(0, Ordering::Release);
    }

    /// Returns `true` when this thread started the in-progress atomic batch.
    ///
    /// Off-thread callers observe confirmed state instead of scanning the pending batch.
    pub(crate) fn consults_atomic_batch(batch_in_progress: bool, owner: &AtomicU64) -> bool {
        if !batch_in_progress {
            return false;
        }
        let owner_key = owner.load(Ordering::Acquire);
        let is_owner = owner_key != 0 && owner_key == current_thread_key();
        if !is_owner {
            #[cfg(feature = "metrics")]
            snarkvm_metrics::increment_counter(snarkvm_metrics::store::ATOMIC_BATCH_OFF_THREAD_SPECULATIVE_READ_TOTAL);
        }
        is_owner
    }

    /// Records time spent waiting for the per-map atomic-batch mutex.
    pub(crate) fn record_lock_wait(start: Instant) {
        #[cfg(feature = "metrics")]
        {
            let elapsed = start.elapsed();
            if elapsed.as_millis() >= 1 {
                snarkvm_metrics::histogram(
                    snarkvm_metrics::store::ATOMIC_BATCH_LOCK_WAIT_SECONDS,
                    elapsed.as_secs_f64(),
                );
            }
        }
        let _ = start;
    }
}

/// Coalesced latest-value index for in-progress atomic batches.
///
/// The write log (`atomic_batch` Vec) remains the source of truth for checkpoints, rewind, and
/// nested finish order. This overlay is rebuilt from the log on rewind and updated on insert.
pub(crate) mod pending_overlay {
    use indexmap::{IndexMap, IndexSet};
    use serde::Serialize;
    use std::hash::Hash;

    /// Rebuilds the latest pending log index per key from an ordered write log.
    pub(crate) fn rebuild_flat<K: Clone + Eq + Hash, V>(log: &[(K, Option<V>)]) -> IndexMap<K, usize> {
        let mut pending = IndexMap::with_capacity(log.len());
        for (i, (key, _)) in log.iter().enumerate() {
            if let Some(previous) = pending.insert(key.clone(), i) {
                assert!(previous < i, "Pending overlay index must increase");
            }
        }
        pending
    }

    /// Returns the pending value for `key`, using the latest log index in `pending`.
    pub(crate) fn get_flat<K, V, Q>(log: &[(K, Option<V>)], pending: &IndexMap<K, usize>, key: &Q) -> Option<Option<V>>
    where
        K: Eq + Hash + std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
        V: Clone,
    {
        pending.get(key).and_then(|&i| log.get(i).map(|(_, value)| value.clone()))
    }

    /// Write log and latest-index overlay sharing one mutex.
    pub(crate) struct FlatBatch<K, V> {
        pub log: Vec<(K, Option<V>)>,
        pub pending: IndexMap<K, usize>,
    }

    impl<K, V> Default for FlatBatch<K, V> {
        fn default() -> Self {
            Self { log: Vec::new(), pending: IndexMap::new() }
        }
    }

    impl<K: Clone + Eq + Hash, V> FlatBatch<K, V> {
        pub(crate) fn is_empty(&self) -> bool {
            self.log.is_empty()
        }

        pub(crate) fn clear(&mut self) {
            self.log.clear();
            self.pending.clear();
        }

        pub(crate) fn push(&mut self, key: K, value: Option<V>) {
            let index = self.log.len();
            if let Some(previous) = self.pending.insert(key.clone(), index) {
                assert!(previous < index, "Pending overlay index must increase");
            }
            self.log.push((key, value));
        }

        pub(crate) fn rewind(&mut self, checkpoint: usize) {
            self.log.truncate(checkpoint);
            self.pending = rebuild_flat(&self.log);
        }

        pub(crate) fn take_log(&mut self) -> Vec<(K, Option<V>)> {
            self.pending.clear();
            core::mem::take(&mut self.log)
        }

        pub(crate) fn get<Q>(&self, key: &Q) -> Option<Option<V>>
        where
            K: std::borrow::Borrow<Q>,
            Q: Eq + Hash + ?Sized,
            V: Clone,
        {
            get_flat(&self.log, &self.pending, key)
        }
    }

    /// Latest nested pending log indices, plus maps that were fully removed in the log.
    pub(crate) struct NestedPending<M> {
        /// Latest `(map, serialized-key)` write, stored as an index into the write log.
        entries: IndexMap<(M, Vec<u8>), usize>,
        /// Maps whose confirmed contents should be ignored (a map-level delete ran).
        deleted_maps: IndexSet<M>,
    }

    impl<M> Default for NestedPending<M> {
        fn default() -> Self {
            Self { entries: IndexMap::new(), deleted_maps: IndexSet::new() }
        }
    }

    impl<M: Copy + Eq + Hash> NestedPending<M> {
        /// Records one nested log operation at `index`.
        pub(crate) fn apply<K: Serialize>(&mut self, map: M, key: Option<K>, index: usize) {
            match key {
                Some(key) => {
                    // Note: The 'unwrap' is safe here, because the keys are defined by us.
                    let key_bytes = bincode::serialize(&key).unwrap();
                    self.entries.insert((map, key_bytes), index);
                }
                None => {
                    self.entries.retain(|(pending_map, _), _| pending_map != &map);
                    self.deleted_maps.insert(map);
                }
            }
        }

        /// Rebuilds the overlay from an ordered nested write log.
        pub(crate) fn rebuild<K: Clone + Serialize, V>(log: &[(M, Option<K>, Option<V>)]) -> Self {
            let mut pending = Self::default();
            for (i, (map, key, _)) in log.iter().enumerate() {
                pending.apply(*map, key.clone(), i);
            }
            pending
        }

        /// Returns the pending value for `key` in `map`, using the same `Option<Option<V>>` meaning
        /// as a reverse scan of the write log.
        pub(crate) fn get<K: Serialize, V: Clone>(
            &self,
            log: &[(M, Option<K>, Option<V>)],
            map: &M,
            key: &K,
        ) -> Option<Option<V>> {
            // Note: The 'unwrap' is safe here, because the keys are defined by us.
            let key_bytes = bincode::serialize(key).unwrap();
            if let Some(&idx) = self.entries.get(&(*map, key_bytes)) {
                return log.get(idx).map(|(_, _, value)| value.clone());
            }
            if self.deleted_maps.contains(map) {
                return Some(None);
            }
            None
        }

        /// Returns `true` when the overlay has no pending entries or map deletes.
        pub(crate) fn is_empty(&self) -> bool {
            self.entries.is_empty() && self.deleted_maps.is_empty()
        }

        /// Clears the overlay.
        pub(crate) fn clear(&mut self) {
            self.entries.clear();
            self.deleted_maps.clear();
        }
    }

    /// Nested write log and overlay sharing one mutex.
    pub(crate) struct NestedBatch<M, K, V> {
        pub log: Vec<(M, Option<K>, Option<V>)>,
        pub pending: NestedPending<M>,
    }

    impl<M, K, V> Default for NestedBatch<M, K, V> {
        fn default() -> Self {
            Self { log: Vec::new(), pending: NestedPending::default() }
        }
    }

    impl<M: Copy + Eq + Hash, K: Clone + Serialize, V: Clone> NestedBatch<M, K, V> {
        pub(crate) fn is_empty(&self) -> bool {
            self.log.is_empty() && self.pending.is_empty()
        }

        pub(crate) fn clear(&mut self) {
            self.log.clear();
            self.pending.clear();
        }

        pub(crate) fn push(&mut self, map: M, key: Option<K>, value: Option<V>) {
            if key.is_none() && value.is_some() {
                unreachable!("Cannot remove a key-value pair from a map without a key.");
            }
            self.pending.apply(map, key.clone(), self.log.len());
            self.log.push((map, key, value));
        }

        pub(crate) fn rewind(&mut self, checkpoint: usize) {
            self.log.truncate(checkpoint);
            self.pending = NestedPending::rebuild(&self.log);
        }

        pub(crate) fn take_log(&mut self) -> Vec<(M, Option<K>, Option<V>)> {
            self.pending.clear();
            core::mem::take(&mut self.log)
        }

        pub(crate) fn get(&self, map: &M, key: &K) -> Option<Option<V>> {
            self.pending.get(&self.log, map, key)
        }
    }

    /// Applies an ordered nested write log to confirmed `key_values` for `map`.
    pub(crate) fn apply_nested_log<M: Copy + PartialEq, K: Clone + PartialEq, V: Clone>(
        key_values: &mut Vec<(K, V)>,
        log: &[(M, Option<K>, Option<V>)],
        map: &M,
    ) {
        for (pending_map, key, value) in log {
            if pending_map != map {
                continue;
            }
            match (key, value) {
                (Some(k), Some(v)) => match key_values.iter_mut().find(|(existing, _)| existing == k) {
                    Some((_, existing)) => *existing = v.clone(),
                    None => key_values.push((k.clone(), v.clone())),
                },
                (Some(k), None) => key_values.retain(|(existing, _)| existing != k),
                (None, None) => key_values.clear(),
                (None, Some(_)) => unreachable!("Cannot remove a key-value pair from a map without a key."),
            }
        }
    }

    #[cfg(test)]
    mod pending_overlay_tests {
        use super::{FlatBatch, NestedBatch, NestedPending, get_flat, rebuild_flat};

        #[test]
        fn rebuild_flat_keeps_latest_value() {
            let log = vec![(1u32, Some("a")), (1u32, Some("b")), (2u32, None)];
            let pending = rebuild_flat(&log);
            assert_eq!(get_flat(&log, &pending, &1), Some(Some("b")));
            assert_eq!(get_flat(&log, &pending, &2), Some(None));
        }

        #[test]
        fn flat_push_replaces_with_a_later_index() {
            let mut batch = FlatBatch::default();
            batch.push(1u32, Some("a"));
            batch.push(1u32, Some("b"));
            assert_eq!(batch.get(&1), Some(Some("b")));
        }

        #[test]
        fn nested_delete_then_reinsert_ignores_confirmed_keys() {
            let mut batch = NestedBatch::default();
            batch.push(0u8, Some(1u8), Some(10u8));
            batch.push(0u8, None, None);
            batch.push(0u8, Some(2u8), Some(20u8));
            assert_eq!(batch.get(&0, &2), Some(Some(20u8)));
            assert_eq!(batch.get(&0, &1), Some(None));
            assert_eq!(batch.get(&0, &99), Some(None));
        }

        #[test]
        fn nested_rebuild_matches_apply() {
            let log = vec![(0u8, Some(1u8), Some(10u8)), (0u8, None, None), (0u8, Some(1u8), Some(11u8))];
            let pending = NestedPending::rebuild(&log);
            assert_eq!(pending.get(&log, &0, &1), Some(Some(11u8)));
            assert_eq!(pending.get(&log, &0, &99), Some(None));
        }
    }
}

/// This macro executes the given block of operations as a new atomic write batch IFF there is no
/// atomic write batch in progress yet. This ensures that complex atomic operations consisting of
/// multiple lower-level operations - which might also need to be atomic if executed individually -
/// are executed as a single large atomic operation regardless.
#[macro_export]
macro_rules! atomic_batch_scope {
    // Untyped variant: delegates to the typed variant with `anyhow::Error`.
    // `From<anyhow::Error> for anyhow::Error` is the identity, so behaviour is unchanged.
    ($self:expr, $ops:block) => {{ $crate::atomic_batch_scope!($self, ::anyhow::Error, $ops) }};
    // Typed variant: callers specify the error type explicitly.
    // `$err` must implement `From<anyhow::Error>` so that `finish_atomic` errors
    // can be converted without ambiguity.
    ($self:expr, $err:ty, $ops:block) => {{
        // Check if an atomic batch write is already in progress. If there isn't one, this means
        // this operation is a "top-level" one and is the one to start and finalize the batch.
        let is_atomic_in_progress = $self.is_atomic_in_progress();

        // Start an atomic batch write operation IFF it's not already part of one.
        match is_atomic_in_progress {
            true => $self.atomic_checkpoint(),
            false => $self.start_atomic(),
        }

        // Wrap the operations that should be batched in a closure to be able to rewind the batch on error.
        // The closure is typed with the caller-provided error type so that `?` inside the block
        // preserves the full structured error rather than erasing it through `anyhow::Error`.
        let run_atomic_ops = || -> ::core::result::Result<_, $err> { $ops };

        // Run the atomic operations.
        match run_atomic_ops() {
            // Save this atomic batch scope and return.
            Ok(result) => match is_atomic_in_progress {
                // A 'true' implies this is a nested atomic batch scope.
                true => {
                    // Once a nested batch scope is completed, clear its checkpoint.
                    // Until a new checkpoint is established,
                    // we can now only rewind to a previous (higher-level) checkpoint.
                    $self.clear_latest_checkpoint();
                    Ok(result)
                }
                // A 'false' implies this is the top-level calling scope.
                // Commit the atomic batch IFF it's the top-level calling scope.
                false => $self.finish_atomic().map_err(<$err as ::core::convert::From<_>>::from).map(|_| result),
            },
            // Rewind this atomic batch scope.
            Err(err) => {
                if is_atomic_in_progress {
                    $self.atomic_rewind();
                } else {
                    $self.abort_atomic();
                }
                Err(err)
            }
        }
    }};
}

/// A top-level helper macro to perform the finalize operation on a list of transactions.
/// The batch is committed when the operations succeed and aborted when they fail.
#[macro_export]
macro_rules! atomic_finalize {
    ($self:expr, $ops:block) => {{
        // Ensure that there is no atomic batch write in progress.
        if $self.is_atomic_in_progress() {
            // We intentionally 'bail!' here instead of passing an Err() to the caller because
            // this is a top-level operation and the caller must fix the issue.
            bail!("Cannot start an atomic batch write operation while another one is already in progress.")
        }

        // Start the atomic batch.
        $self.start_atomic();

        // Run the atomic operations.
        //
        // Wrap the operations that should be batched in a closure to be able to abort the entire
        // write batch if any of them fails.
        #[allow(clippy::redundant_closure_call)]
        match (|| -> Result<_, String> { $ops }()) {
            // Commit the atomic batch.
            Ok(result) => {
                $self.finish_atomic()?;
                Ok(result)
            }
            // Abort the atomic batch.
            Err(error_msg) => {
                $self.abort_atomic();
                Err(anyhow!("Failed to finalize transactions - {error_msg}"))
            }
        }
    }};
}
