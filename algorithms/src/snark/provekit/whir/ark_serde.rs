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

//! Serde helpers for types that serialize through canonical encodings.

use serde::{Deserialize as _, Deserializer, Serializer, de::Error as _, ser::Error as _};

/// Serialize using snarkVM canonical encodings.
pub mod canonical {
    use snarkvm_utilities::{CanonicalDeserialize, CanonicalSerialize};

    use super::*;

    pub fn serialize<T, S>(obj: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: CanonicalSerialize,
        S: Serializer,
    {
        let mut buf = Vec::with_capacity(obj.compressed_size());
        obj.serialize_compressed(&mut buf).map_err(|e| S::Error::custom(format!("Failed to serialize: {e}")))?;
        super::bytes::serialize(&buf, serializer)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: CanonicalDeserialize,
        D: Deserializer<'de>,
    {
        let bytes = super::bytes::deserialize(deserializer)?;
        let mut reader = &*bytes;
        let obj = T::deserialize_compressed(&mut reader)
            .map_err(|e| D::Error::custom(format!("while deserializing: {e}")))?;
        if !reader.is_empty() {
            return Err(D::Error::custom("while deserializing: trailing bytes"));
        }
        Ok(obj)
    }
}

pub mod bytes {
    use super::*;

    pub fn serialize<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let hex = hex::encode(value);
            serializer.serialize_str(&hex)
        } else {
            serializer.serialize_bytes(value)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let hex = String::deserialize(deserializer)?;
            hex::decode(hex).map_err(|e| D::Error::custom(format!("while deserializing bytes: {e}")))
        } else {
            <Vec<u8>>::deserialize(deserializer)
        }
    }
}
