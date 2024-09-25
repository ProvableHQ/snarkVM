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

use crate::{polycommit::sonic_pc, snark::varuna::ahp::indexer::*};
use snarkvm_curves::PairingEngine;
use snarkvm_utilities::{
    error,
    io::{self, Read, Write},
    serialize::*,
    string::String,
    FromBytes,
    FromBytesDeserializer,
    ToBytes,
    ToBytesSerializer,
};

use anyhow::Result;
use core::{fmt, str::FromStr};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;

/// Verification key for a specific index (i.e., R1CS matrices).
#[derive(Debug, Clone, PartialEq, Eq, CanonicalSerialize, CanonicalDeserialize)]
pub struct CircuitVerifyingKey<E: PairingEngine> {
    /// Stores information about the size of the circuit, as well as its defined field.
    pub circuit_info: CircuitInfo,
    /// Commitments to the indexed polynomials.
    pub circuit_commitments: Vec<sonic_pc::Commitment<E>>,
    pub id: CircuitId,
}

impl<E: PairingEngine> FromBytes for CircuitVerifyingKey<E> {
    fn read_le<R: Read>(r: R) -> io::Result<Self> {
        Self::deserialize_compressed(r).map_err(|_| error("could not deserialize CircuitVerifyingKey"))
    }
}

impl<E: PairingEngine> ToBytes for CircuitVerifyingKey<E> {
    fn write_le<W: Write>(&self, w: W) -> io::Result<()> {
        self.serialize_compressed(w).map_err(|_| error("could not serialize CircuitVerifyingKey"))
    }
}

impl<E: PairingEngine> CircuitVerifyingKey<E> {
    /// Iterate over the commitments to indexed polynomials in `self`.
    pub fn iter(&self) -> impl Iterator<Item = &sonic_pc::Commitment<E>> {
        self.circuit_commitments.iter()
    }
}

impl<E: PairingEngine> FromStr for CircuitVerifyingKey<E> {
    type Err = anyhow::Error;

    #[inline]
    fn from_str(vk_hex: &str) -> Result<Self, Self::Err> {
        Self::from_bytes_le(&hex::decode(vk_hex)?)
    }
}

impl<E: PairingEngine> fmt::Display for CircuitVerifyingKey<E> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let vk_hex = hex::encode(self.to_bytes_le().expect("Failed to convert verifying key to bytes"));
        write!(f, "{vk_hex}")
    }
}

impl<E: PairingEngine> Serialize for CircuitVerifyingKey<E> {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => serializer.collect_str(self),
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, E: PairingEngine> Deserialize<'de> for CircuitVerifyingKey<E> {
    #[inline]
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                let s: String = Deserialize::deserialize(deserializer)?;
                FromStr::from_str(&s).map_err(de::Error::custom)
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "verifying key"),
        }
    }
}

impl<E: PairingEngine> Ord for CircuitVerifyingKey<E> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl<E: PairingEngine> PartialOrd for CircuitVerifyingKey<E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
