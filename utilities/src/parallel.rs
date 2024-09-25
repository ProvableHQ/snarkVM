// Copyright 2024 Aleo Network Foundation
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

impl<'a, T> Default for ExecutionPool<'a, T> {
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

#[inline(always)]
#[cfg(not(any(feature = "serial", feature = "wasm")))]
pub fn execute_with_max_available_threads<T: Sync + Send>(f: impl FnOnce() -> T + Send) -> T {
    execute_with_threads(f, max_available_threads())
}

#[inline(always)]
#[cfg(any(feature = "serial", feature = "wasm"))]
pub fn execute_with_max_available_threads<T>(f: impl FnOnce() -> T + Send) -> T {
    f()
}

#[cfg(not(any(feature = "serial", feature = "wasm")))]
#[inline(always)]
fn execute_with_threads<T: Sync + Send>(f: impl FnOnce() -> T + Send, num_threads: usize) -> T {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();
    pool.install(f)
}

/// Creates parallel iterator over refs if `parallel` feature is enabled.
#[macro_export]
macro_rules! cfg_iter {
    ($e: expr) => {{
        #[cfg(not(feature = "serial"))]
        let result = $e.par_iter();

        #[cfg(feature = "serial")]
        let result = $e.iter();

        result
    }};
}

/// Creates parallel iterator over mut refs if `parallel` feature is enabled.
#[macro_export]
macro_rules! cfg_iter_mut {
    ($e: expr) => {{
        #[cfg(not(feature = "serial"))]
        let result = $e.par_iter_mut();

        #[cfg(feature = "serial")]
        let result = $e.iter_mut();

        result
    }};
}

/// Creates parallel iterator if `parallel` feature is enabled.
#[macro_export]
macro_rules! cfg_into_iter {
    ($e: expr) => {{
        #[cfg(not(feature = "serial"))]
        let result = $e.into_par_iter();

        #[cfg(feature = "serial")]
        let result = $e.into_iter();

        result
    }};
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

/// Turns a collection into an iterator.
#[macro_export]
macro_rules! cfg_keys {
    ($e: expr) => {{
        #[cfg(not(feature = "serial"))]
        let result = $e.par_keys();

        #[cfg(feature = "serial")]
        let result = $e.keys();

        result
    }};
}

/// Turns a collection into an iterator.
#[macro_export]
macro_rules! cfg_values {
    ($e: expr) => {{
        #[cfg(not(feature = "serial"))]
        let result = $e.par_values();

        #[cfg(feature = "serial")]
        let result = $e.values();

        result
    }};
}

/// Finds the first element that satisfies the predicate function
#[macro_export]
macro_rules! cfg_find {
    ($self:expr, $object:expr, $func:ident) => {{
        #[cfg(not(feature = "serial"))]
        let result = $self.par_values().find_any(|tx| tx.$func($object));

        #[cfg(feature = "serial")]
        let result = $self.values().find(|tx| tx.$func($object));

        result
    }};
}

/// Applies a function and returns the first value that is not None
#[macro_export]
macro_rules! cfg_find_map {
    ($self:expr, $object:expr, $func:ident) => {{
        #[cfg(not(feature = "serial"))]
        let result = $self.par_values().filter_map(|tx| tx.$func($object)).find_any(|_| true);

        #[cfg(feature = "serial")]
        let result = $self.values().find_map(|tx| tx.$func($object));

        result
    }};
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
#[macro_export]
macro_rules! cfg_sort_unstable_by {
    ($self: expr, $closure: expr) => {{
        #[cfg(feature = "serial")]
        $self.sort_unstable_by($closure);

        #[cfg(not(feature = "serial"))]
        $self.par_sort_unstable_by($closure);
    }};
}

/// Performs a sort that caches the extracted keys
#[macro_export]
macro_rules! cfg_sort_by_cached_key {
    ($self: expr, $closure: expr) => {{
        #[cfg(feature = "serial")]
        $self.sort_by_cached_key($closure);

        #[cfg(not(feature = "serial"))]
        $self.par_sort_by_cached_key($closure);
    }};
}
