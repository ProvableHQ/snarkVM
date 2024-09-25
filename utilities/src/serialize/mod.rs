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

pub mod error;
pub use error::*;

mod helpers;
pub use helpers::*;

mod impls;
#[allow(unused_imports)]
pub use impls::*;

mod flags;
pub use flags::*;

mod traits;
pub use traits::*;

#[cfg(feature = "derive")]
pub use snarkvm_utilities_derives::*;

/// Return the number of (byte-aligned) bits and bytes required to represent the given number of bits.
///
/// Examples:
/// - Given `64 bits`, returns `(64, 8)`.
/// - Given `251 bits`, returns `(256, 32)`.
/// - Given `999 bits`, returns `(1000, 125)`.
#[inline]
pub const fn number_of_bits_and_bytes(num_bits: usize) -> (usize, usize) {
    let byte_size = number_of_bits_to_number_of_bytes(num_bits);
    ((byte_size * 8), byte_size)
}

/// Return the number of bytes required to represent the given number of bits.
#[inline]
pub const fn number_of_bits_to_number_of_bytes(num_bits: usize) -> usize {
    (num_bits + 7) / 8
}

#[test]
fn test_number_of_bits_and_bytes() {
    assert_eq!((64, 8), number_of_bits_and_bytes(64));
    assert_eq!((256, 32), number_of_bits_and_bytes(251));
    assert_eq!((1000, 125), number_of_bits_and_bytes(999));
}
