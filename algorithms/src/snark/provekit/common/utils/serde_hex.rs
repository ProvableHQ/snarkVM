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

//! Serde workaround to encode `Vec<u8>` as base64 strings in
//! human-readable formats.
//!
//! Uses standard base64 encoding (33% overhead) instead of hexadecimal
//! (100% overhead), cutting human-readable proof size by ~25%.
//! Deserialization auto-detects hex for backwards compatibility.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

pub fn serialize<S>(obj: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if serializer.is_human_readable() {
        let b64 = STANDARD.encode(obj);
        serializer.serialize_str(&b64)
    } else {
        serializer.serialize_bytes(obj)
    }
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        let encoded: String = <String>::deserialize(deserializer)?;
        if encoded.len() % 2 == 0 && encoded.bytes().all(|b| b.is_ascii_hexdigit()) {
            hex::decode(&encoded).map_err(D::Error::custom)
        } else {
            STANDARD.decode(&encoded).map_err(D::Error::custom)
        }
    } else {
        <Vec<u8>>::deserialize(deserializer)
    }
}
