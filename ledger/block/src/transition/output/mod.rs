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

mod bytes;
mod serialize;
mod string;

use console::{
    network::prelude::*,
    program::{Ciphertext, Future, Plaintext, Record, TransitionLeaf},
    types::{Field, Group},
};

type Variant = u8;

/// The transition output.
#[derive(Clone, PartialEq, Eq)]
pub enum Output<N: Network> {
    /// The plaintext hash and (optional) plaintext.
    Constant(Field<N>, Option<Plaintext<N>>),
    /// The plaintext hash and (optional) plaintext.
    Public(Field<N>, Option<Plaintext<N>>),
    /// The ciphertext hash and (optional) ciphertext.
    Private(Field<N>, Option<Ciphertext<N>>),
    /// The commitment, checksum, and (optional) record ciphertext.
    Record(Field<N>, Field<N>, Option<Record<N, Ciphertext<N>>>),
    /// The output commitment of the external record. Note: This is **not** the record commitment.
    ExternalRecord(Field<N>),
    /// The future hash and (optional) future.
    Future(Field<N>, Option<Future<N>>),
}

impl<N: Network> Output<N> {
    /// Returns the variant of the output.
    pub const fn variant(&self) -> Variant {
        match self {
            Output::Constant(_, _) => 0,
            Output::Public(_, _) => 1,
            Output::Private(_, _) => 2,
            Output::Record(_, _, _) => 3,
            Output::ExternalRecord(_) => 4,
            Output::Future(_, _) => 5,
        }
    }

    /// Returns the ID of the output.
    pub const fn id(&self) -> &Field<N> {
        match self {
            Output::Constant(id, ..) => id,
            Output::Public(id, ..) => id,
            Output::Private(id, ..) => id,
            Output::Record(commitment, ..) => commitment,
            Output::ExternalRecord(id) => id,
            Output::Future(id, ..) => id,
        }
    }

    /// Returns the output as a transition leaf.
    pub fn to_transition_leaf(&self, index: u8) -> TransitionLeaf<N> {
        TransitionLeaf::new_with_version(index, self.variant(), *self.id())
    }

    /// Returns the commitment and record, if the output is a record.
    #[allow(clippy::type_complexity)]
    pub const fn record(&self) -> Option<(&Field<N>, &Record<N, Ciphertext<N>>)> {
        match self {
            Output::Record(commitment, _, Some(record)) => Some((commitment, record)),
            _ => None,
        }
    }

    /// Consumes `self` and returns the commitment and record, if the output is a record.
    #[allow(clippy::type_complexity)]
    pub fn into_record(self) -> Option<(Field<N>, Record<N, Ciphertext<N>>)> {
        match self {
            Output::Record(commitment, _, Some(record)) => Some((commitment, record)),
            _ => None,
        }
    }

    /// Returns the commitment, if the output is a record.
    pub const fn commitment(&self) -> Option<&Field<N>> {
        match self {
            Output::Record(commitment, ..) => Some(commitment),
            _ => None,
        }
    }

    /// Returns the commitment, if the output is a record, and consumes `self`.
    pub fn into_commitment(self) -> Option<Field<N>> {
        match self {
            Output::Record(commitment, ..) => Some(commitment),
            _ => None,
        }
    }

    /// Returns the nonce, if the output is a record.
    pub const fn nonce(&self) -> Option<&Group<N>> {
        match self {
            Output::Record(_, _, Some(record)) => Some(record.nonce()),
            _ => None,
        }
    }

    /// Returns the nonce, if the output is a record, and consumes `self`.
    pub fn into_nonce(self) -> Option<Group<N>> {
        match self {
            Output::Record(_, _, Some(record)) => Some(record.into_nonce()),
            _ => None,
        }
    }

    /// Returns the checksum, if the output is a record.
    pub const fn checksum(&self) -> Option<&Field<N>> {
        match self {
            Output::Record(_, checksum, ..) => Some(checksum),
            _ => None,
        }
    }

    /// Returns the checksum, if the output is a record, and consumes `self`.
    pub fn into_checksum(self) -> Option<Field<N>> {
        match self {
            Output::Record(_, checksum, ..) => Some(checksum),
            _ => None,
        }
    }

    /// Returns the future, if the output is a future.
    pub const fn future(&self) -> Option<&Future<N>> {
        match self {
            Output::Future(_, Some(future)) => Some(future),
            _ => None,
        }
    }

    /// Returns the public verifier inputs for the proof.
    pub fn verifier_inputs(&self) -> impl '_ + Iterator<Item = N::Field> {
        // Append the output ID.
        [**self.id()].into_iter()
            // Append the checksum if it exists.
            .chain([self.checksum().map(|sum| **sum)].into_iter().flatten())
    }

    /// Returns `true` if the output is well-formed.
    /// If the optional value exists, this method checks that it hashes to the output ID.
    pub fn verify(&self, function_id: Field<N>, tcm: &Field<N>, index: usize) -> bool {
        // Ensure the hash of the value (if the value exists) is correct.
        let result = || match self {
            Output::Constant(hash, Some(output)) => {
                match output.to_fields() {
                    Ok(fields) => {
                        // Construct the (console) output index as a field element.
                        let index = Field::from_u16(index as u16);
                        // Construct the preimage as `(function ID || output || tcm || index)`.
                        let mut preimage = Vec::new();
                        preimage.push(function_id);
                        preimage.extend(fields);
                        preimage.push(*tcm);
                        preimage.push(index);
                        // Ensure the hash matches.
                        match N::hash_psd8(&preimage) {
                            Ok(candidate_hash) => Ok(hash == &candidate_hash),
                            Err(error) => Err(error),
                        }
                    }
                    Err(error) => Err(error),
                }
            }
            Output::Public(hash, Some(output)) => {
                match output.to_fields() {
                    Ok(fields) => {
                        // Construct the (console) output index as a field element.
                        let index = Field::from_u16(index as u16);
                        // Construct the preimage as `(function ID || output || tcm || index)`.
                        let mut preimage = Vec::new();
                        preimage.push(function_id);
                        preimage.extend(fields);
                        preimage.push(*tcm);
                        preimage.push(index);
                        // Ensure the hash matches.
                        match N::hash_psd8(&preimage) {
                            Ok(candidate_hash) => Ok(hash == &candidate_hash),
                            Err(error) => Err(error),
                        }
                    }
                    Err(error) => Err(error),
                }
            }
            Output::Private(hash, Some(value)) => {
                match value.to_fields() {
                    // Ensure the hash matches.
                    Ok(fields) => match N::hash_psd8(&fields) {
                        Ok(candidate_hash) => Ok(hash == &candidate_hash),
                        Err(error) => Err(error),
                    },
                    Err(error) => Err(error),
                }
            }
            Output::Record(_, checksum, Some(value)) => match N::hash_bhp1024(&value.to_bits_le()) {
                Ok(candidate_hash) => Ok(checksum == &candidate_hash),
                Err(error) => Err(error),
            },
            Output::Future(hash, Some(output)) => {
                match output.to_fields() {
                    Ok(fields) => {
                        // Construct the (future) output index as a field element.
                        let index = Field::from_u16(index as u16);
                        // Construct the preimage as `(function ID || output || tcm || index)`.
                        let mut preimage = Vec::new();
                        preimage.push(function_id);
                        preimage.extend(fields);
                        preimage.push(*tcm);
                        preimage.push(index);
                        // Ensure the hash matches.
                        match N::hash_psd8(&preimage) {
                            Ok(candidate_hash) => Ok(hash == &candidate_hash),
                            Err(error) => Err(error),
                        }
                    }
                    Err(error) => Err(error),
                }
            }
            Output::Constant(_, None)
            | Output::Public(_, None)
            | Output::Private(_, None)
            | Output::Record(_, _, None)
            | Output::Future(_, None) => {
                // This enforces that the transition *must* contain the value for this transition output.
                // A similar rule is enforced for the transition input.
                bail!("A transition output value is missing")
            }
            Output::ExternalRecord(_) => Ok(true),
        };

        match result() {
            Ok(is_hash_valid) => is_hash_valid,
            Err(error) => {
                eprintln!("{error}");
                false
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use console::{network::MainnetV0, program::Literal};

    type CurrentNetwork = MainnetV0;

    /// Sample the transition outputs.
    pub(crate) fn sample_outputs() -> Vec<(<CurrentNetwork as Network>::TransitionID, Output<CurrentNetwork>)> {
        let rng = &mut TestRng::default();

        // Sample a transition.
        let transaction = crate::transaction::test_helpers::sample_execution_transaction_with_fee(true, rng);
        let transition = transaction.transitions().next().unwrap();

        // Retrieve the transition ID and input.
        let transition_id = *transition.id();
        let input = transition.outputs().iter().next().unwrap().clone();

        // Sample a random plaintext.
        let plaintext = Plaintext::Literal(Literal::Field(Uniform::rand(rng)), Default::default());
        let plaintext_hash = CurrentNetwork::hash_bhp1024(&plaintext.to_bits_le()).unwrap();
        // Sample a random ciphertext.
        let fields: Vec<_> = (0..10).map(|_| Uniform::rand(rng)).collect();
        let ciphertext = Ciphertext::from_fields(&fields).unwrap();
        let ciphertext_hash = CurrentNetwork::hash_bhp1024(&ciphertext.to_bits_le()).unwrap();
        // Sample a random record.
        let randomizer = Uniform::rand(rng);
        let nonce = CurrentNetwork::g_scalar_multiply(&randomizer);
        let record = Record::<CurrentNetwork, Plaintext<CurrentNetwork>>::from_str(
            &format!("{{ owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah.private, token_amount: 100u64.private, _nonce: {nonce}.public }}"),
        ).unwrap();
        let record_ciphertext = record.encrypt(randomizer).unwrap();
        let record_checksum = CurrentNetwork::hash_bhp1024(&record_ciphertext.to_bits_le()).unwrap();

        vec![
            (transition_id, input),
            (Uniform::rand(rng), Output::Constant(Uniform::rand(rng), None)),
            (Uniform::rand(rng), Output::Constant(plaintext_hash, Some(plaintext.clone()))),
            (Uniform::rand(rng), Output::Public(Uniform::rand(rng), None)),
            (Uniform::rand(rng), Output::Public(plaintext_hash, Some(plaintext))),
            (Uniform::rand(rng), Output::Private(Uniform::rand(rng), None)),
            (Uniform::rand(rng), Output::Private(ciphertext_hash, Some(ciphertext))),
            (Uniform::rand(rng), Output::Record(Uniform::rand(rng), Uniform::rand(rng), None)),
            (Uniform::rand(rng), Output::Record(Uniform::rand(rng), record_checksum, Some(record_ciphertext))),
            (Uniform::rand(rng), Output::ExternalRecord(Uniform::rand(rng))),
        ]
    }
}
