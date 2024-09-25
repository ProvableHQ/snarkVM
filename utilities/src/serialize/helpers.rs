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

pub use crate::{
    io::{self, Read, Write},
    FromBytes,
    ToBytes,
    Vec,
};
use crate::{serialize::traits::*, SerializationError};

/// Serialize a Vector's elements without serializing the Vector's length
/// If you want to serialize the full Vector, use `CanonicalSerialize for Vec<T>`
pub fn serialize_vec_without_len<'a>(
    src: impl Iterator<Item = &'a (impl CanonicalSerialize + 'a)>,
    mut writer: impl Write,
    compress: Compress,
) -> Result<(), SerializationError> {
    for elem in src {
        CanonicalSerialize::serialize_with_mode(elem, &mut writer, compress)?;
    }
    Ok(())
}

/// Serialize a Vector's element sizes without serializing the Vector's length
/// If you want to serialize the full Vector, use `CanonicalSerialize for Vec<T>`
pub fn serialized_vec_size_without_len(src: &[impl CanonicalSerialize], compress: Compress) -> usize {
    if src.is_empty() { 0 } else { src.len() * CanonicalSerialize::serialized_size(&src[0], compress) }
}

/// Deserialize a Vector's elements without deserializing the Vector's length
/// If you want to deserialize the full Vector, use `CanonicalDeserialize for Vec<T>`
pub fn deserialize_vec_without_len<T: CanonicalDeserialize>(
    mut reader: impl Read,
    compress: Compress,
    validate: Validate,
    len: usize,
) -> Result<Vec<T>, SerializationError> {
    (0..len).map(|_| CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)).collect()
}
