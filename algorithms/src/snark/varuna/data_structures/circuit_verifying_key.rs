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

use crate::{AlgebraicSponge, polycommit::sonic_pc, snark::varuna::ahp::indexer::*};
use snarkvm_curves::PairingEngine;
use snarkvm_utilities::{FromBytes, FromBytesDeserializer, ToBytes, ToBytesSerializer, into_io_error, serialize::*};

use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{
    cmp::Ordering,
    fmt,
    io::{self, Read, Write},
    str::FromStr,
    string::String,
    sync::OnceLock,
};

/// Verification key for a specific index (i.e., R1CS matrices).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitVerifyingKey<E: PairingEngine> {
    /// Stores information about the size of the circuit, as well as its defined
    /// field.
    pub circuit_info: CircuitInfo,
    /// Commitments to the indexed polynomials.
    pub circuit_commitments: Vec<sonic_pc::Commitment<E>>,
    pub id: CircuitId,
    pub circuit_commitments_hash: OnceLock<E::Fq>,
}

impl<E: PairingEngine> CircuitVerifyingKey<E> {
    pub fn get_or_calculate_circuit_commitments_hash<FS: AlgebraicSponge<E::Fq, 2>>(
        &self,
        fs_parameters: &FS::Parameters,
    ) -> &E::Fq {
        self.circuit_commitments_hash.get_or_init(|| {
            let mut sponge = FS::new_with_parameters(fs_parameters);
            sponge.absorb_native_field_elements(&self.circuit_commitments);

            sponge.squeeze_native_field_elements(1)[0]
        })
    }
}

impl<E: PairingEngine> FromBytes for CircuitVerifyingKey<E> {
    fn read_le<R: Read>(r: R) -> io::Result<Self> {
        Self::deserialize_compressed(r)
            .map_err(|err| into_io_error(anyhow::Error::from(err).context("could not deserialize CircuitVerifyingKey")))
    }
}

impl<E: PairingEngine> ToBytes for CircuitVerifyingKey<E> {
    fn write_le<W: Write>(&self, w: W) -> io::Result<()> {
        self.serialize_compressed(w)
            .map_err(|err| into_io_error(anyhow::Error::from(err).context("could not serialize CircuitVerifyingKey")))
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

impl<E: PairingEngine> CanonicalSerialize for CircuitVerifyingKey<E> {
    fn serialize_with_mode<W: Write>(&self, mut writer: W, compress: Compress) -> Result<(), SerializationError> {
        self.circuit_info.serialize_with_mode(&mut writer, compress)?;
        self.circuit_commitments.serialize_with_mode(&mut writer, compress)?;
        self.id.serialize_with_mode(&mut writer, compress)?;
        // The hash is omitted.
        Ok(())
    }

    fn serialized_size(&self, compress: Compress) -> usize {
        self.circuit_info.serialized_size(compress)
            + self.circuit_commitments.serialized_size(compress)
            + self.id.serialized_size(compress)
        // The hash is omitted.
    }
}

impl<E: PairingEngine> CanonicalDeserialize for CircuitVerifyingKey<E> {
    fn deserialize_with_mode<R: Read>(
        mut reader: R,
        compress: Compress,
        validate: Validate,
    ) -> Result<Self, SerializationError> {
        let circuit_info = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let circuit_commitments = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let id = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        Ok(Self { circuit_info, circuit_commitments, id, circuit_commitments_hash: Default::default() })
    }
}

impl<E: PairingEngine> Valid for CircuitVerifyingKey<E> {
    fn check(&self) -> Result<(), SerializationError> {
        Valid::check(&self.circuit_info)?;
        Valid::check(&self.circuit_commitments)?;
        Valid::check(&self.id)?;
        // The hash is omitted.
        Ok(())
    }

    fn batch_check<'a>(batch: impl Iterator<Item = &'a Self> + Send) -> Result<(), SerializationError>
    where
        Self: 'a,
    {
        let batch: Vec<_> = batch.collect();
        Valid::batch_check(batch.iter().map(|v| &v.circuit_info))?;
        Valid::batch_check(batch.iter().map(|v| &v.circuit_commitments))?;
        Valid::batch_check(batch.iter().map(|v| &v.id))?;
        // The hash is omitted.
        Ok(())
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
