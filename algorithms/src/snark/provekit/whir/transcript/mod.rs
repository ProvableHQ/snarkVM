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

//! Wrapper around Spongefish to add out-of-band hint messages.
//!
//! We need these for the Merkle tree proofs as doing them in-transcript
//! would roughly double the verifier cost.

pub mod codecs;
mod field_elem;
mod mock_sponge;

pub use field_elem::FieldElem;

#[cfg(debug_assertions)]
use std::any::type_name;
use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256, Sha3_512};
use snarkvm_fields::Field;
use snarkvm_utilities::{CanonicalDeserialize, CanonicalSerialize};
use spongefish::StdHash;
pub use spongefish::{
    Codec,
    Decoding,
    DuplexSpongeInterface,
    Encoding,
    NargDeserialize,
    NargSerialize,
    VerificationError,
    VerificationResult,
};

#[cfg(any())]
pub use self::mock_sponge::MockSponge;

macro_rules! verify {
    ($cond:expr) => {
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if !$cond {
            return Err(spongefish::VerificationError);
        };
    };
}

pub(crate) use verify;

/// Marker trait for types that can be used as prover messages.
///
/// Like [`spongefish::Codec`], but without the [`Encoding<T>`] requirement.
pub trait ProverMessage<U = [u8]>: NargDeserialize + NargSerialize + Encoding<U>
where
    U: ?Sized,
{
}

#[derive(Clone, Copy, Debug)]
pub struct DomainSeparator<'a, I> {
    pub protocol_id: [u8; 64],
    pub session_id: [u8; 32],
    instance: &'a I,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Interaction {
    ProverMessage(String),
    VerifierMessage(String),
    Hint(String),
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct Proof {
    pub narg_string: Vec<u8>,
    pub hints: Vec<u8>,

    #[cfg(debug_assertions)]
    pub pattern: Vec<Interaction>,
}

pub struct ProverState<H = StdHash>
where
    H: DuplexSpongeInterface,
{
    inner: spongefish::ProverState<H>,
    hints: Vec<u8>,

    #[cfg(debug_assertions)]
    pattern: Vec<Interaction>,
}

pub struct VerifierState<'a, H = StdHash>
where
    H: DuplexSpongeInterface,
{
    inner: spongefish::VerifierState<'a, H>,
    hints: &'a [u8],

    #[cfg(debug_assertions)]
    pattern: &'a [Interaction],
}

pub trait VerifierMessage {
    type U;

    fn verifier_message<T>(&mut self) -> T
    where
        T: Decoding<[Self::U]>;

    fn verifier_message_vec<T>(&mut self, count: usize) -> Vec<T>
    where
        T: Decoding<[Self::U]>,
    {
        (0..count).map(|_| self.verifier_message()).collect()
    }

    /// Sample a snarkVM prime-field challenge.
    fn verifier_field<F>(&mut self) -> F
    where
        F: Field,
        FieldElem<F>: Decoding<[Self::U]>,
    {
        self.verifier_message::<FieldElem<F>>().0
    }

    /// Sample `count` snarkVM field challenges.
    fn verifier_field_vec<F>(&mut self, count: usize) -> Vec<F>
    where
        F: Field,
        FieldElem<F>: Decoding<[Self::U]>,
    {
        (0..count).map(|_| self.verifier_field()).collect()
    }
}

impl DomainSeparator<'static, ()> {
    pub fn protocol<C: Serialize>(config: &C) -> Self {
        const INSTANCE: &() = &();
        let mut hash = Sha3_512::new();
        ciborium::into_writer(config, &mut hash).expect("Computing protocol hash failed");
        let protocol_id: [u8; 64] = hash.finalize().into();
        Self { protocol_id, session_id: [0; 32], instance: INSTANCE }
    }

    #[must_use]
    pub fn session<S: Serialize>(self, session: &S) -> Self {
        let mut hash = Sha3_256::new();
        ciborium::into_writer(session, &mut hash).expect("Computing session hash failed");
        let session_id: [u8; 32] = hash.finalize().into();
        Self { session_id, ..self }
    }

    pub const fn instance<I>(self, instance: &I) -> DomainSeparator<'_, I> {
        DomainSeparator { protocol_id: self.protocol_id, session_id: self.session_id, instance }
    }
}

impl<T, U> ProverMessage<U> for T
where
    T: NargSerialize + NargDeserialize + Encoding<U>,
    U: ?Sized,
{
}

impl<H> ProverState<H>
where
    H: DuplexSpongeInterface,
{
    /// Construct a new prover state with a custom duplex hash function.
    ///
    /// **Note.** The `spongefish` API currently does not allow creating an
    /// instance with a non-standard random number generator.
    pub fn new<I>(ds: &DomainSeparator<'_, I>, duplex: H) -> Self
    where
        u8: Encoding<[H::U]>,
        I: Encoding<[H::U]>,
    {
        Self {
            inner: spongefish::DomainSeparator::new(ds.protocol_id)
                .session(ds.session_id)
                .instance(ds.instance)
                .to_prover(duplex),
            hints: Vec::new(),

            #[cfg(debug_assertions)]
            pattern: Vec::new(),
        }
    }
}

impl ProverState<StdHash> {
    /// Construct a new prover state with the standard duplex hash function.
    pub fn new_std<I>(ds: &DomainSeparator<'_, I>) -> Self
    where
        I: Encoding<[u8]>,
    {
        Self::new(ds, StdHash::default())
    }
}

impl<H> ProverState<H>
where
    H: DuplexSpongeInterface,
{
    /// Sample secret field elements from the transcript-bound prover RNG.
    pub fn random_field_vec<F: Field>(&mut self, length: usize) -> Vec<F> {
        use rand_08::RngCore;
        use snarkvm_fields::PrimeField;

        let width = F::BasePrimeField::size_in_bits().div_ceil(8) + 32;
        let mut buf = vec![0u8; width];
        let rng = self.inner.rng();
        (0..length)
            .map(|_| {
                rng.fill_bytes(&mut buf);
                F::from_base_prime_field(F::BasePrimeField::from_bytes_le_mod_order(&buf))
            })
            .collect()
    }

    /// Absorb a snarkVM field prover message.
    #[cfg_attr(test, track_caller)]
    pub fn prover_field<F>(&mut self, message: &F)
    where
        F: Field,
        FieldElem<F>: Encoding<[H::U]> + NargSerialize,
    {
        self.prover_message(&FieldElem(*message));
    }

    /// Absorb snarkVM field prover messages.
    #[cfg_attr(test, track_caller)]
    pub fn prover_fields<F>(&mut self, messages: &[F])
    where
        F: Field,
        FieldElem<F>: Encoding<[H::U]> + NargSerialize,
    {
        for message in messages {
            self.prover_field(message);
        }
    }

    #[cfg_attr(test, track_caller)]
    pub fn prover_message<T>(&mut self, message: &T)
    where
        T: Encoding<[H::U]> + NargSerialize + ?Sized,
    {
        #[cfg(debug_assertions)]
        self.push(Interaction::ProverMessage(type_name::<T>().to_owned()));
        self.inner.prover_message(message);
    }

    #[cfg_attr(test, track_caller)]
    pub fn prover_messages<T>(&mut self, messages: &[T])
    where
        T: Encoding<[H::U]> + NargSerialize,
    {
        for message in messages {
            self.prover_message(message);
        }
    }

    #[cfg_attr(test, track_caller)]
    pub fn prover_hint<T>(&mut self, hint: &T)
    where
        T: NargSerialize,
    {
        #[cfg(debug_assertions)]
        self.push(Interaction::Hint(type_name::<T>().to_owned()));
        hint.serialize_into_narg(&mut self.hints);
    }

    #[cfg_attr(test, track_caller)]
    pub fn prover_hint_ark<T>(&mut self, value: &T)
    where
        T: CanonicalSerialize + ?Sized,
    {
        #[cfg(debug_assertions)]
        self.push(Interaction::Hint(type_name::<T>().to_owned()));
        value.serialize_compressed(&mut self.hints).expect("Failed to serialize hint");
    }

    pub fn proof(self) -> Proof {
        Proof {
            narg_string: self.inner.narg_string().to_owned(),
            hints: self.hints,

            #[cfg(debug_assertions)]
            pattern: self.pattern,
        }
    }

    #[cfg(debug_assertions)]
    fn push(&mut self, interaction: Interaction) {
        self.pattern.push(interaction);
    }
}

impl<H> VerifierMessage for ProverState<H>
where
    H: DuplexSpongeInterface,
{
    type U = H::U;

    #[cfg_attr(test, track_caller)]
    fn verifier_message<T>(&mut self) -> T
    where
        T: Decoding<[H::U]>,
    {
        #[cfg(debug_assertions)]
        self.push(Interaction::VerifierMessage(type_name::<T>().to_owned()));
        self.inner.verifier_message()
    }
}

impl<'a, H> VerifierState<'a, H>
where
    H: DuplexSpongeInterface,
{
    pub fn new<'b, I>(ds: &DomainSeparator<'b, I>, proof: &'a Proof, duplex: H) -> Self
    where
        u8: Encoding<[H::U]>,
        I: Encoding<[H::U]>,
    {
        Self {
            inner: spongefish::DomainSeparator::new(ds.protocol_id)
                .session(ds.session_id)
                .instance(ds.instance)
                .to_verifier(duplex, &proof.narg_string),
            hints: &proof.hints,
            #[cfg(debug_assertions)]
            pattern: &proof.pattern,
        }
    }

    pub const fn as_spongefish(&mut self) -> &mut spongefish::VerifierState<'a, H> {
        &mut self.inner
    }

    #[cfg_attr(debug_assertions, track_caller)]
    pub fn check_eof(self) -> VerificationResult<()> {
        #[cfg(debug_assertions)]
        assert!(self.pattern.is_empty());
        verify!(self.inner.check_eof().is_ok());
        verify!(self.hints.is_empty());
        Ok(())
    }

    #[cfg_attr(test, track_caller)]
    pub fn prover_message<T>(&mut self) -> VerificationResult<T>
    where
        T: Encoding<[H::U]> + NargDeserialize,
    {
        #[cfg(debug_assertions)]
        self.pop_pattern(&Interaction::ProverMessage(type_name::<T>().to_owned()));
        self.inner.prover_message()
    }

    /// Read a snarkVM field prover message.
    #[cfg_attr(test, track_caller)]
    pub fn prover_field<F>(&mut self) -> VerificationResult<F>
    where
        F: Field,
        FieldElem<F>: Encoding<[H::U]> + NargDeserialize,
    {
        Ok(self.prover_message::<FieldElem<F>>()?.0)
    }

    #[cfg_attr(test, track_caller)]
    pub fn prover_messages_vec<T>(&mut self, len: usize) -> VerificationResult<Vec<T>>
    where
        T: Encoding<[H::U]> + NargDeserialize,
    {
        (0..len).map(|_| self.prover_message()).collect()
    }

    /// Read `len` snarkVM field prover messages.
    #[cfg_attr(test, track_caller)]
    pub fn prover_fields_vec<F>(&mut self, len: usize) -> VerificationResult<Vec<F>>
    where
        F: Field,
        FieldElem<F>: Encoding<[H::U]> + NargDeserialize,
    {
        (0..len).map(|_| self.prover_field()).collect()
    }

    #[cfg_attr(test, track_caller)]
    pub fn prover_hint<T>(&mut self) -> VerificationResult<T>
    where
        T: NargDeserialize,
    {
        #[cfg(debug_assertions)]
        self.pop_pattern(&Interaction::Hint(type_name::<T>().to_owned()));
        T::deserialize_from_narg(&mut self.hints)
    }

    #[cfg_attr(test, track_caller)]
    pub fn prover_hint_ark<T>(&mut self) -> VerificationResult<T>
    where
        T: CanonicalDeserialize,
    {
        #[cfg(debug_assertions)]
        self.pop_pattern(&Interaction::Hint(type_name::<T>().to_owned()));
        T::deserialize_compressed(&mut self.hints).map_err(|_| VerificationError)
    }

    #[cfg(debug_assertions)]
    #[track_caller]
    fn pop_pattern(&mut self, interaction: &Interaction) {
        assert!(!self.pattern.is_empty());
        let (expected, tail) = self.pattern.split_first().unwrap();
        assert_eq!(interaction, expected, "Transcript error: Expected interaction {expected:?} got {interaction:?}");
        self.pattern = tail;
    }
}

impl<'a> VerifierState<'a, StdHash> {
    /// Construct a new verifier state with the standard duplex hash function.
    pub fn new_std<'b, I>(ds: &DomainSeparator<'b, I>, proof: &'a Proof) -> Self
    where
        I: Encoding<[u8]>,
    {
        Self::new(ds, proof, StdHash::default())
    }
}

impl<H> VerifierMessage for VerifierState<'_, H>
where
    H: DuplexSpongeInterface,
{
    type U = H::U;

    #[cfg_attr(test, track_caller)]
    fn verifier_message<T>(&mut self) -> T
    where
        T: Decoding<[H::U]>,
    {
        #[cfg(debug_assertions)]
        self.pop_pattern(&Interaction::VerifierMessage(type_name::<T>().to_owned()));
        self.inner.verifier_message()
    }
}
