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

use crate::{boxed::Box, vec::Vec};

#[cfg(not(feature = "serial"))]
use rayon::slice::ParallelSliceMut;

pub struct ExecutionPool<'a, T> {
    jobs: Vec<Box<dyn 'a + FnOnce() -> T + Send>>,
}

impl<'a, T> ExecutionPool<'a, T> {
    pub fn new() -> Self {
        Self { jobs: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self { jobs: Vec::with_capacity(cap) }
    }

    pub fn add_job<F: 'a + FnOnce() -> T + Send>(&mut self, f: F) {
        self.jobs.push(Box::new(f));
    }

    pub fn execute_all(self) -> Vec<T>
    where
        T: Send + Sync,
    {
        #[cfg(not(feature = "serial"))]
        {
            use rayon::prelude::*;
            execute_with_max_available_threads(|| self.jobs.into_par_iter().map(|f| f()).collect())
        }
        #[cfg(feature = "serial")]
        {
            self.jobs.into_iter().map(|f| f()).collect()
        }
    }
}

impl<T> Default for ExecutionPool<'_, T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "serial"))]
pub fn max_available_threads() -> usize {
    use aleo_std::Cpu;
    let rayon_threads = rayon::current_num_threads();

    match aleo_std::get_cpu() {
        Cpu::Intel => num_cpus::get_physical().min(rayon_threads),
        Cpu::AMD | Cpu::Unknown => rayon_threads,
    }
}

#[cfg(not(any(feature = "serial", feature = "wasm")))]
#[inline(always)]
pub fn execute_with_max_available_threads<T: Sync + Send>(f: impl FnOnce() -> T + Send) -> T {
    execute_with_threads(f, max_available_threads())
}

#[cfg(any(feature = "serial", feature = "wasm"))]
#[inline(always)]
pub fn execute_with_max_available_threads<T>(f: impl FnOnce() -> T + Send) -> T {
    f()
}

#[cfg(not(any(feature = "serial", feature = "wasm")))]
#[inline(always)]
fn execute_with_threads<T: Sync + Send>(f: impl FnOnce() -> T + Send, num_threads: usize) -> T {
    if rayon::current_thread_index().is_none() {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();
        pool.install(f)
    } else {
        f()
    }
}

/// Creates an iterator from a collection. The iterator is serial if the `serial` feature is enabled,
/// otherwise it will be parallel using rayon.
///
/// # Usage
/// This function works for any struct that implements `IntoIterator`.
///
/// If you want to iterate over an object without consuming it (similar to how `iter()` behaves), use a reference.
/// ```rust
/// let my_data = vec![1, 2, 3,];
///
/// // This will not consume the vector and the iterator will return references to each entry.
/// let _it = cfg_iter(&my_data);
///
/// // This will consume the vector and the iterator returns the element directly (to avoid copying).
/// let _it = cfg_iter(my_data);
/// ```
#[cfg(feature = "serial")]
#[inline(always)]
pub fn cfg_iter<C: IntoIterator>(collection: C) -> C::IntoIter {
    collection.into_iter()
}
#[cfg(not(feature = "serial"))]
#[inline(always)]
pub fn cfg_iter<C: rayon::iter::IntoParallelIterator>(collection: C) -> C::Iter {
    collection.into_par_iter()
}

/// Returns an iterator over `chunk_size` elements of the slice at a
/// time.
#[macro_export]
macro_rules! cfg_chunks {
    ($e: expr, $size: expr) => {{
        #[cfg(not(feature = "serial"))]
        let result = $e.par_chunks($size);

        #[cfg(feature = "serial")]
        let result = $e.chunks($size);

        result
    }};
}

/// Returns an iterator over `chunk_size` elements of the slice at a time.
#[macro_export]
macro_rules! cfg_chunks_mut {
    ($e: expr, $size: expr) => {{
        #[cfg(not(feature = "serial"))]
        let result = $e.par_chunks_mut($size);

        #[cfg(feature = "serial")]
        let result = $e.chunks_mut($size);

        result
    }};
}

/// Creates parallel iterator from iterator if `parallel` feature is enabled.
#[macro_export]
macro_rules! cfg_par_bridge {
    ($e: expr) => {{
        #[cfg(not(feature = "serial"))]
        let result = $e.par_bridge();

        #[cfg(feature = "serial")]
        let result = $e;

        result
    }};
}

/// Applies the reduce operation over an iterator.
#[macro_export]
macro_rules! cfg_reduce {
    ($e: expr, $default: expr, $op: expr) => {{
        #[cfg(not(feature = "serial"))]
        let result = $e.reduce($default, $op);

        #[cfg(feature = "serial")]
        let result = $e.fold($default(), $op);

        result
    }};
}

/// Applies `reduce_with` or `reduce` depending on the `serial` feature.
#[macro_export]
macro_rules! cfg_reduce_with {
    ($e: expr, $op: expr) => {{
        #[cfg(not(feature = "serial"))]
        let result = $e.reduce_with($op);

        #[cfg(feature = "serial")]
        let result = $e.reduce($op);

        result
    }};
}

#[cfg(feature = "indexmap")]
pub mod indexmap {
    use indexmap::{IndexMap, map};

    cfg_if::cfg_if! {
        if #[cfg(feature="serial")] {
            pub type IndexmapIntoIter<K, V> = map::IntoIter<K, V>;
            pub type IndexmapKeys<'a, K, V> = map::Keys<'a, K, V>;
            pub type IndexmapValues<'a, K, V> = map::Values<'a, K, V>;
        } else {
            use rayon::iter::ParallelIterator;

            pub type IndexmapIntoIter<K, V> = map::rayon::IntoParIter<K, V>;
            pub type IndexmapKeys<'a, K, V> = map::rayon::ParKeys<'a, K, V>;
            pub type IndexmapValues<'a, K, V> = map::rayon::ParValues<'a, K, V>;
        }
    }

    /// Returns an iterator over all keys in an `IndexMap`.
    #[inline(always)]
    pub fn cfg_keys<K: Sync, V: Sync>(imap: &IndexMap<K, V>) -> IndexmapKeys<K, V> {
        #[cfg(feature = "serial")]
        {
            imap.keys()
        }
        #[cfg(not(feature = "serial"))]
        {
            imap.par_keys()
        }
    }

    /// Returns an iterator over all values in an `IndexMap`.
    #[inline(always)]
    pub fn cfg_values<K: Sync, V: Sync>(imap: &IndexMap<K, V>) -> IndexmapValues<K, V> {
        #[cfg(feature = "serial")]
        {
            imap.values()
        }
        #[cfg(not(feature = "serial"))]
        {
            imap.par_values()
        }
    }

    /// Find a value `v` in an indexmap where `lambda(v)` evalutes to true (if any).
    ///
    /// # Notes
    /// - This returns at most one entry that satisfies the given condition, not necessarily the first one.
    /// - `closure` must be a lambda function returning a boolean, e.g., `|e| e > 0`.
    #[inline(always)]
    pub fn cfg_find_value<K, V, F>(imap: &IndexMap<K, V>, closure: F) -> Option<&V>
    where
        K: Sync,
        V: Sync,
        F: Sync + Fn(&V) -> bool,
    {
        #[cfg(feature = "serial")]
        {
            imap.values().find(|v| closure(*v))
        }

        #[cfg(not(feature = "serial"))]
        {
            imap.par_values().find_any(|v| closure(*v))
        }
    }

    /// Returns `v'=lambda(v)` for a value `v` in the map where `v'` is not None (if any).
    ///
    /// # Notes
    /// - This returns at most one `v'` not necessarily the first one.
    /// - `closure` must be a lambda function returning Option, e.g., `|v| Some(v)`.
    #[inline(always)]
    pub fn cfg_find_value_map<'a, K, V, V2, F>(imap: &'a IndexMap<K, V>, closure: F) -> Option<&'a V2>
    where
        K: Sync,
        V: Sync,
        V2: Sync + Send,
        F: Sync + Send + Fn(&'a V) -> Option<&'a V2>,
    {
        #[cfg(feature = "serial")]
        let result = imap.values().find_map(closure);
        #[cfg(not(feature = "serial"))]
        let result = imap.par_values().filter_map(closure).find_any(|_| true);

        result
    }

    /// Returns a sorted, by-value, iterator for the given IndexMap/IndexSet
    #[inline(always)]
    pub fn cfg_sorted_by<K, V, F>(imap: IndexMap<K, V>, closure: F) -> IndexmapIntoIter<K, V>
    where
        K: Sync + Send,
        V: Sync + Send,
        F: Sync + Send + Fn(&K, &V, &K, &V) -> std::cmp::Ordering,
    {
        #[cfg(feature = "serial")]
        {
            imap.sorted_by(closure)
        }

        #[cfg(not(feature = "serial"))]
        {
            imap.par_sorted_by(closure)
        }
    }
}

/// Applies fold to the iterator
#[macro_export]
macro_rules! cfg_zip_fold {
    ($self: expr, $other: expr, $init: expr, $op: expr, $type: ty) => {{
        let default = $init;

        #[cfg(feature = "serial")]
        let default = $init();
        let result = $self.zip_eq($other).fold(default, $op);

        #[cfg(not(feature = "serial"))]
        let result = result.sum::<$type>();

        result
    }};
}

/// Performs an unstable sort
#[inline(always)]
pub fn cfg_sort_unstable_by<T, F>(slice: &mut [T], sort_fn: F)
where
    F: Fn(&T, &T) -> std::cmp::Ordering + Sync,
    T: Send,
{
    #[cfg(feature = "serial")]
    slice.sort_unstable_by(sort_fn);

    #[cfg(not(feature = "serial"))]
    slice.par_sort_unstable_by(sort_fn);
}

/// Performs a sort that caches the extracted keys.
#[inline(always)]
pub fn cfg_sort_by_cached_key<T, F, K>(slice: &mut [T], key_fn: F)
where
    F: Fn(&T) -> K + Sync,
    K: Ord + Send,
    T: Send,
{
    #[cfg(feature = "serial")]
    slice.sort_by_cached_key(key_fn);

    #[cfg(not(feature = "serial"))]
    slice.par_sort_by_cached_key(key_fn);
}
