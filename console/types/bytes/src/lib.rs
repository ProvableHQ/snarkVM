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

#![cfg_attr(test, allow(clippy::assertions_on_result_states))]
#![warn(clippy::cast_possible_truncation)]

mod bitwise;
mod bytes;
mod parse;
mod random;
mod serialize;

pub use snarkvm_console_network_environment::prelude::*;
pub use snarkvm_console_types_boolean::Boolean;

use core::marker::PhantomData;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BytesType<E: Environment> {
    /// The underlying bytes.
    bytes: Vec<u8>,
    /// PhantomData
    _phantom: PhantomData<E>,
}

impl<E: Environment> BytesTrait for BytesType<E> {}

impl<E: Environment> BytesType<E> {
    /// Initializes a new instance.
    pub fn new(bytes: Vec<u8>) -> Self {
        // Ensure the bytes are within the allowed capacity.
        let num_bytes = bytes.len();
        match num_bytes <= E::MAX_DECODED_BYTES as usize {
            true => Self { bytes: bytes.to_owned(), _phantom: PhantomData },
            false => E::halt(format!("Attempted to allocate bytes of size {num_bytes}")),
        }
    }
}

impl<E: Environment> TypeName for BytesType<E> {
    /// Returns the type name as a string.
    #[inline]
    fn type_name() -> &'static str {
        "bytes"
    }
}

impl<E: Environment> Deref for BytesType<E> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.bytes.as_slice()
    }
}
