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

use crate::serialize::Flags;

/// Flags to be encoded into the serialization.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct EmptyFlags;

impl Flags for EmptyFlags {
    const BIT_SIZE: usize = 0;

    #[inline]
    fn u8_bitmask(&self) -> u8 {
        0
    }

    #[inline]
    fn from_u8(value: u8) -> Option<Self> {
        if (value >> 7) == 0 { Some(EmptyFlags) } else { None }
    }
}

/// Flags to be encoded into the serialization.
/// The default flags (empty) should not change the binary representation.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum SWFlags {
    Infinity,
    PositiveY,
    #[default]
    NegativeY,
}

impl SWFlags {
    #[inline]
    pub fn infinity() -> Self {
        SWFlags::Infinity
    }

    #[inline]
    pub fn from_y_sign(is_positive: bool) -> Self {
        if is_positive { SWFlags::PositiveY } else { SWFlags::NegativeY }
    }

    #[inline]
    pub fn is_infinity(&self) -> bool {
        matches!(self, SWFlags::Infinity)
    }

    #[inline]
    pub fn is_positive(&self) -> Option<bool> {
        match self {
            SWFlags::Infinity => None,
            SWFlags::PositiveY => Some(true),
            SWFlags::NegativeY => Some(false),
        }
    }
}

impl Flags for SWFlags {
    const BIT_SIZE: usize = 2;

    #[inline]
    fn u8_bitmask(&self) -> u8 {
        let mut mask = 0;
        match self {
            SWFlags::Infinity => mask |= 1 << 6,
            SWFlags::PositiveY => mask |= 1 << 7,
            _ => (),
        }
        mask
    }

    #[inline]
    fn from_u8(value: u8) -> Option<Self> {
        let x_sign = (value >> 7) & 1 == 1;
        let is_infinity = (value >> 6) & 1 == 1;
        match (x_sign, is_infinity) {
            // This is invalid because we only want *one* way to serialize
            // the point at infinity.
            (true, true) => None,
            (false, true) => Some(SWFlags::Infinity),
            (true, false) => Some(SWFlags::PositiveY),
            (false, false) => Some(SWFlags::NegativeY),
        }
    }
}

/// Flags to be encoded into the serialization.
/// The default flags (empty) should not change the binary representation.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum EdwardsFlags {
    PositiveY,
    #[default]
    NegativeY,
}

impl EdwardsFlags {
    #[inline]
    pub fn from_y_sign(is_positive: bool) -> Self {
        if is_positive { EdwardsFlags::PositiveY } else { EdwardsFlags::NegativeY }
    }

    #[inline]
    pub fn is_positive(&self) -> bool {
        match self {
            EdwardsFlags::PositiveY => true,
            EdwardsFlags::NegativeY => false,
        }
    }
}

impl Flags for EdwardsFlags {
    const BIT_SIZE: usize = 1;

    #[inline]
    fn u8_bitmask(&self) -> u8 {
        let mut mask = 0;
        if let Self::PositiveY = self {
            mask |= 1 << 7;
        }
        mask
    }

    #[inline]
    fn from_u8(value: u8) -> Option<Self> {
        let x_sign = (value >> 7) & 1 == 1;
        if x_sign { Some(Self::PositiveY) } else { Some(Self::NegativeY) }
    }
}
