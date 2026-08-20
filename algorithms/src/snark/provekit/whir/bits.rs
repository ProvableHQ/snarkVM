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

use std::{
    cmp::Ordering,
    fmt::{self, Display},
    hash::Hash,
};

use serde::{Deserialize, Serialize};

/// Wrapper for `bits` value types.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bits(f64);

impl Hash for Bits {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl Eq for Bits {}

impl Bits {
    pub fn new(bits: f64) -> Self {
        assert!(bits.is_finite());
        Self(bits)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0.0
    }
}

impl From<f64> for Bits {
    fn from(bits: f64) -> Self {
        Self::new(bits)
    }
}

impl From<Bits> for f64 {
    fn from(bits: Bits) -> Self {
        bits.0
    }
}

impl Display for Bits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <f64 as Display>::fmt(&self.0, f)
    }
}

impl PartialOrd for Bits {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Bits {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap()
    }
}
