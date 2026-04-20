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

use snarkvm_console_types::{Field, prelude::*};

use core::ops::{Index, IndexMut, Range};

#[derive(Copy, Clone, Debug)]
pub struct State<E: Environment, const RATE: usize, const CAPACITY_PLUS_RATE: usize> {
    state: [Field<E>; CAPACITY_PLUS_RATE],
}

impl<E: Environment, const RATE: usize, const CAPACITY_PLUS_RATE: usize> Default
    for State<E, RATE, CAPACITY_PLUS_RATE>
{
    fn default() -> Self {
        Self { state: [Field::<E>::zero(); CAPACITY_PLUS_RATE] }
    }
}

impl<E: Environment, const RATE: usize, const CAPACITY_PLUS_RATE: usize> State<E, RATE, CAPACITY_PLUS_RATE> {
    /// Returns a reference to a range of the rate state.
    pub(super) fn rate_state(&self, range: Range<usize>) -> &[Field<E>] {
        let offset = CAPACITY_PLUS_RATE - RATE;
        &self.state[(offset + range.start)..(offset + range.end)]
    }

    /// Returns a mutable slice over the rate portion of the state.
    pub(super) fn rate_state_mut(&mut self) -> &mut [Field<E>] {
        let offset = CAPACITY_PLUS_RATE - RATE;
        &mut self.state[offset..]
    }
}

impl<E: Environment, const RATE: usize, const CAPACITY_PLUS_RATE: usize> State<E, RATE, CAPACITY_PLUS_RATE> {
    /// Returns an immutable iterator over the state.
    pub fn iter(&self) -> impl Iterator<Item = &Field<E>> + Clone {
        self.state.iter()
    }

    /// Returns a mutable iterator over the state.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Field<E>> {
        self.state.iter_mut()
    }
}

impl<E: Environment, const RATE: usize, const CAPACITY_PLUS_RATE: usize> Index<usize>
    for State<E, RATE, CAPACITY_PLUS_RATE>
{
    type Output = Field<E>;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < CAPACITY_PLUS_RATE, "Index out of bounds: index is {index} but length is {CAPACITY_PLUS_RATE}");
        &self.state[index]
    }
}

impl<E: Environment, const RATE: usize, const CAPACITY_PLUS_RATE: usize> IndexMut<usize>
    for State<E, RATE, CAPACITY_PLUS_RATE>
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < CAPACITY_PLUS_RATE, "Index out of bounds: index is {index} but length is {CAPACITY_PLUS_RATE}");
        &mut self.state[index]
    }
}
