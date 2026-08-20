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

//! Transcript codec for snarkVM fields.
//!
//! spongefish's `Encoding`/`Decoding` traits cannot be implemented directly on
//! [`snarkvm_curves::bls12_377::Fr`] (orphan rule), so WHIR absorbs and samples
//! field elements through this local wrapper.

use snarkvm_fields::{Field, PrimeField};
use spongefish::{Decoding, Encoding, NargDeserialize, VerificationError, VerificationResult};
use std::marker::PhantomData;

/// Byte length of a canonical little-endian prime-field encoding, plus 32 extra
/// bytes of Fiat-Shamir randomness used when sampling verifier messages.
fn decoding_buffer_size<F: Field>() -> usize {
    F::BasePrimeField::size_in_bits().div_ceil(8) + 32
}

fn encoding_size<F: Field>() -> usize {
    F::BasePrimeField::size_in_bits().div_ceil(8)
}

/// Local wrapper so snarkVM field elements can be Fiat-Shamir messages.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct FieldElem<F>(pub F);

impl<F: Field> Encoding<[u8]> for FieldElem<F> {
    fn encode(&self) -> impl AsRef<[u8]> {
        let mut bytes = vec![0u8; encoding_size::<F>()];
        self.0.write_le(&mut bytes[..]).expect("field encoding fits canonical width");
        bytes
    }
}

/// Buffer holding enough bytes to sample a uniform field element.
pub struct DecodingFieldBuffer<F: Field> {
    buf: Vec<u8>,
    _field: PhantomData<F>,
}

impl<F: Field> Default for DecodingFieldBuffer<F> {
    fn default() -> Self {
        Self { buf: vec![0u8; decoding_buffer_size::<F>()], _field: PhantomData }
    }
}

impl<F: Field> AsMut<[u8]> for DecodingFieldBuffer<F> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }
}

impl<F: Field> Decoding<[u8]> for FieldElem<F> {
    type Repr = DecodingFieldBuffer<F>;

    fn decode(repr: Self::Repr) -> Self {
        debug_assert_eq!(repr.buf.len(), decoding_buffer_size::<F>());
        let base = F::BasePrimeField::from_bytes_le_mod_order(&repr.buf);
        Self(F::from_base_prime_field(base))
    }
}

impl<F: Field> NargDeserialize for FieldElem<F> {
    fn deserialize_from_narg(buf: &mut &[u8]) -> VerificationResult<Self> {
        let n = encoding_size::<F>();
        if buf.len() < n {
            return Err(VerificationError);
        }
        let (head, tail) = buf.split_at(n);
        *buf = tail;
        let base = F::BasePrimeField::from_bytes_le_mod_order(head);
        Ok(Self(F::from_base_prime_field(base)))
    }
}
