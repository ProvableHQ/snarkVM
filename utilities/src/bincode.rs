// Copyright (c) 2019-2025 Provable Inc.
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

pub use bincode::error::{DecodeError, EncodeError};
use serde::{Serialize, de};

use std::{cell::Cell, io::Write};

thread_local! {
    pub(crate) static UNCHECKED_DESERIALIZE: Cell<bool> = const { Cell::new(false) };
}

pub(crate) use bincode::config::legacy as config;

/// Wrapper around bincode's deserialization that ensures the correct format is used.
pub fn serialize<T: Serialize + ?Sized>(object: &T) -> Result<Vec<u8>, EncodeError> {
    bincode::serde::encode_to_vec(object, config())
}

/// Wrapper around bincode's deserialization that ensures the correct format is used.
pub fn serialize_into_write<T: Serialize + ?Sized, W: Write>(write: &mut W, object: &T) -> Result<usize, EncodeError> {
    bincode::serde::encode_into_std_write(object, write, config())
}

/// Wrapper around bincode's deserialization that ensures the correct format is used.
pub fn deserialize<T: de::DeserializeOwned>(data: &[u8]) -> Result<T, DecodeError> {
    bincode::serde::decode_from_slice(data, config()).map(|(data, _)| data)
}

/// Performs a bincode deserialization without any checks of the data.
///
/// Important: This should only be used when deserializing from local storage.
#[inline(always)]
pub fn unchecked_deserialize<T: de::DeserializeOwned>(data: &[u8]) -> Result<T, DecodeError> {
    UNCHECKED_DESERIALIZE.set(true);
    let result = deserialize(data);
    UNCHECKED_DESERIALIZE.set(false);
    result
}
