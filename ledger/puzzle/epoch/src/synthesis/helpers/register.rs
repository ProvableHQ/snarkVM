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

use std::fmt::Display;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PuzzleRegister {
    Locator(u64),
}

impl From<u64> for PuzzleRegister {
    /// Initializes a new register locator.
    fn from(locator: u64) -> Self {
        Self::new_locator(locator)
    }
}

impl PuzzleRegister {
    /// Initializes a new register locator.
    pub const fn new_locator(locator: u64) -> Self {
        Self::Locator(locator)
    }
}

impl Display for PuzzleRegister {
    /// Returns the display string for the register.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Locator(locator) => write!(f, "r{locator}"),
        }
    }
}
