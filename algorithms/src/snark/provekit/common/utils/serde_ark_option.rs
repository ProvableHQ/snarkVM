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

// Originally derived from ProveKit, Copyright 2026 World Foundation (MIT).

use serde::{Deserialize as _, Deserializer, Serializer, de::Error as _, ser::Error as _};
use snarkvm_utilities::{CanonicalDeserialize, CanonicalSerialize};

pub fn serialize<T, S>(obj: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    T: CanonicalSerialize,
    S: Serializer,
{
    match obj {
        Some(value) => {
            let mut buf = Vec::with_capacity(value.compressed_size());
            value.serialize_compressed(&mut buf).map_err(|e| S::Error::custom(format!("Failed to serialize: {e}")))?;

            // Write bytes
            if serializer.is_human_readable() {
                // ark_serialize doesn't have human-readable serialization. And Serde
                // doesn't have good defaults for [u8]. So we implement hexadecimal
                // serialization.
                let hex = hex::encode(buf);
                serializer.serialize_some(&hex)
            } else {
                serializer.serialize_some(&buf)
            }
        }
        None => serializer.serialize_none(),
    }
}

pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: CanonicalDeserialize,
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        let maybe_hex: Option<String> = Option::deserialize(deserializer)?;
        match maybe_hex {
            Some(hex) => {
                let bytes = hex::decode(&hex).map_err(|e| D::Error::custom(format!("invalid hex: {e}")))?;
                let mut reader = &*bytes;
                let field = T::deserialize_compressed(&mut reader)
                    .map_err(|e| D::Error::custom(format!("deserialize failed: {e}")))?;
                Ok(Some(field))
            }
            None => Ok(None),
        }
    } else {
        let maybe_bytes: Option<Vec<u8>> = Option::deserialize(deserializer)?;
        match maybe_bytes {
            Some(bytes) => {
                let mut reader = &*bytes;
                let field = T::deserialize_compressed(&mut reader)
                    .map_err(|e| D::Error::custom(format!("deserialize failed: {e}")))?;
                Ok(Some(field))
            }
            None => Ok(None),
        }
    }
}
