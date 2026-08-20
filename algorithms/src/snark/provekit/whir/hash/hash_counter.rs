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

// Originally derived from WHIR (https://github.com/WizardOfMenlo/whir),
// licensed under Apache-2.0 OR MIT.

use std::sync::atomic::{AtomicUsize, Ordering};

pub static HASH_COUNTER: HashCounter = HashCounter::new();

#[derive(Debug)]
pub struct HashCounter(AtomicUsize);

impl Default for HashCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl HashCounter {
    pub const fn new() -> Self {
        Self(AtomicUsize::new(0))
    }

    pub(crate) fn add(&self, count: usize) {
        self.0.fetch_add(count, Ordering::Relaxed);
    }

    pub fn reset(&self) {
        self.0.store(0, Ordering::SeqCst);
    }

    pub fn get(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}
