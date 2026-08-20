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

use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use serde::{
    Deserializer,
    Serializer,
    de::{Error as _, SeqAccess, Visitor},
    ser::{Error as _, SerializeSeq},
};
use std::{fmt, marker::PhantomData};

pub fn serialize<T, S>(vec: &Vec<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    T: CanonicalSerialize,
    S: Serializer,
{
    let is_human_readable = serializer.is_human_readable();
    let mut seq = serializer.serialize_seq(Some(vec.len()))?;
    for element in vec {
        let mut buf = Vec::with_capacity(element.compressed_size());
        element.serialize_compressed(&mut buf).map_err(|e| S::Error::custom(format!("Failed to serialize: {e}")))?;

        // Write bytes
        if is_human_readable {
            // ark_serialize doesn't have human-readable serialization. And Serde
            // doesn't have good defaults for [u8]. So we implement hexadecimal
            // serialization.
            let hex = hex::encode(buf);
            seq.serialize_element(&hex)?;
        } else {
            seq.serialize_element(&buf)?;
        }
    }
    seq.end()
}

pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: CanonicalDeserialize,
    D: Deserializer<'de>,
{
    struct VecVisitor<T> {
        is_human_readable: bool,
        _marker: PhantomData<T>,
    }

    impl<'de, T: CanonicalDeserialize> Visitor<'de> for VecVisitor<T> {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a sequence of field elements")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            if self.is_human_readable {
                while let Some(hex) = seq.next_element::<String>()? {
                    let bytes = hex::decode(hex).map_err(|e| A::Error::custom(format!("invalid hex: {e}")))?;
                    let mut reader = &*bytes;
                    let element = T::deserialize_compressed(&mut reader)
                        .map_err(|e| A::Error::custom(format!("deserialize failed: {e}")))?;
                    if !reader.is_empty() {
                        return Err(A::Error::custom("while deserializing: trailing bytes"));
                    }
                    vec.push(element);
                }
            } else {
                while let Some(bytes) = seq.next_element::<Vec<u8>>()? {
                    let mut reader = &*bytes;
                    let element = T::deserialize_compressed(&mut reader)
                        .map_err(|e| A::Error::custom(format!("deserialize failed: {e}")))?;
                    if !reader.is_empty() {
                        return Err(A::Error::custom("while deserializing: trailing bytes"));
                    }
                    vec.push(element);
                }
            }
            Ok(vec)
        }
    }

    let is_human_readable = deserializer.is_human_readable();
    deserializer.deserialize_seq(VecVisitor { is_human_readable, _marker: PhantomData })
}
