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

use core::num::NonZeroU32;
use rand_core::RngCore;

/// `OptionalRng` is a hack that is necessary because `Option<&mut R>` is not implicitly reborrowed
/// like `&mut R` is. This causes problems when a variable of type `Option<&mut R>`
/// is moved (eg, in a loop).
///
/// To overcome this, we define the wrapper `OptionalRng` here that can be borrowed
/// mutably, without fear of being moved.
pub struct OptionalRng<R>(pub Option<R>);

impl<R: RngCore> RngCore for OptionalRng<R> {
    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.0.as_mut().map(|r| r.next_u32()).expect("Rng was invoked in a non-hiding context")
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.0.as_mut().map(|r| r.next_u64()).expect("Rng was invoked in a non-hiding context")
    }

    #[inline]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.as_mut().map(|r| r.fill_bytes(dest)).expect("Rng was invoked in a non-hiding context")
    }

    #[inline]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        match &mut self.0 {
            Some(r) => r.try_fill_bytes(dest),
            None => Err(NonZeroU32::new(rand_core::Error::CUSTOM_START).unwrap().into()),
        }
    }
}

impl<R: RngCore> From<R> for OptionalRng<R> {
    fn from(other: R) -> Self {
        Self(Some(other))
    }
}
