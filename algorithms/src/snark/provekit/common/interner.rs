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

// Originally derived from ProveKit, Copyright 2026 World Foundation (MIT).

use crate::snark::provekit::common::utils::serde_ark;
use ark_ff::Field;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Interner<F: Field> {
    #[serde(with = "serde_ark")]
    values: Vec<F>,
    /// Reverse index: `F` → its position in `values`. Kept in sync
    /// by `intern`. Not serialized (the on-disk format remains a single
    /// `values` field for backward compatibility); rebuilt lazily on first
    /// `intern` after deserialize.
    #[serde(skip)]
    index: HashMap<F, usize>,
}

/// `index` is `#[serde(skip)]` and rebuilt lazily, so a roundtripped
/// `Interner` has an empty map until the next `intern` call. Compare only
/// the canonical `values` field — handles based on `values` indices are
/// stable across map state, and the map is just an O(1) lookup cache.
impl<F: Field> PartialEq for Interner<F> {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}

impl<F: Field> Default for Interner<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Field> Interner<F> {
    pub fn new() -> Self {
        Self { values: Vec::new(), index: HashMap::new() }
    }

    /// Intern a field element, returning a stable handle.
    ///
    /// O(1) amortized — uses a `HashMap` to deduplicate. The map is populated
    /// lazily on first call after deserialize / `from_values`.
    pub fn intern(&mut self, value: F) -> InternedFieldElement {
        // After deserialize (or `from_values` without index population), the
        // map may be empty while `values` is not. Rebuild once.
        if self.index.len() < self.values.len() {
            self.rebuild_index();
        }

        if let Some(&idx) = self.index.get(&value) {
            return InternedFieldElement(idx);
        }

        let idx = self.values.len();
        self.values.push(value);
        self.index.insert(value, idx);
        InternedFieldElement(idx)
    }

    pub fn get(&self, el: InternedFieldElement) -> Option<F> {
        self.values.get(el.0).copied()
    }

    /// Borrow the deduplicated values array. Used by mmap-format writers
    /// that need the raw bytes.
    pub fn values_raw(&self) -> &[F] {
        &self.values
    }

    /// Construct an `Interner` from a pre-built values vector. Bypasses the
    /// dedup work in `intern()` — used by mmap-format readers that have
    /// already loaded a deduplicated set of values from disk.
    ///
    /// The reverse index is built eagerly so subsequent `intern` calls are
    /// O(1) without paying for a lazy rebuild on first use.
    ///
    /// # Precondition
    ///
    /// `values` must contain no duplicate field elements. Violating this
    /// leaves the interner in a non-canonical state where two distinct
    /// `InternedFieldElement` handles can refer to the same field element,
    /// breaking the "equal values produce equal handles" invariant relied on
    /// by handle-equality consumers (cache keys, dedup checks, etc.).
    /// Debug builds assert this; release builds trust the caller.
    pub fn from_values(values: Vec<F>) -> Self {
        let mut index = HashMap::with_capacity(values.len());
        for (i, v) in values.iter().enumerate() {
            index.insert(*v, i);
        }
        debug_assert_eq!(
            index.len(),
            values.len(),
            "Interner::from_values called with duplicate field elements — the caller's \
             deduplication contract has been violated"
        );
        Self { values, index }
    }

    fn rebuild_index(&mut self) {
        self.index.clear();
        self.index.reserve(self.values.len());
        for (i, v) in self.values.iter().enumerate() {
            self.index.insert(*v, i);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternedFieldElement(usize);

impl InternedFieldElement {
    /// Construct an InternedFieldElement from a raw index. Used by
    /// mmap-format readers that load the index Vec from raw bytes.
    pub const fn new(idx: usize) -> Self {
        Self(idx)
    }

    /// Inner index value.
    pub const fn index(&self) -> usize {
        self.0
    }
}
