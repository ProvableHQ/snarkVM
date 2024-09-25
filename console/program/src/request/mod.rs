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

mod input_id;
pub use input_id::InputID;

mod bytes;
mod serialize;
mod sign;
mod string;
mod verify;

use crate::{compute_function_id, Identifier, Plaintext, ProgramID, Record, Value, ValueType};
use snarkvm_console_account::{Address, ComputeKey, GraphKey, PrivateKey, Signature, ViewKey};
use snarkvm_console_network::Network;
use snarkvm_console_types::prelude::*;

#[derive(Clone, PartialEq, Eq)]
pub struct Request<N: Network> {
    /// The request signer.
    signer: Address<N>,
    /// The network ID.
    network_id: U16<N>,
    /// The program ID.
    program_id: ProgramID<N>,
    /// The function name.
    function_name: Identifier<N>,
    /// The input ID for the transition.
    input_ids: Vec<InputID<N>>,
    /// The function inputs.
    inputs: Vec<Value<N>>,
    /// The signature for the transition.
    signature: Signature<N>,
    /// The tag secret key.
    sk_tag: Field<N>,
    /// The transition view key.
    tvk: Field<N>,
    /// The transition commitment.
    tcm: Field<N>,
    /// The signer commitment.
    scm: Field<N>,
}

impl<N: Network>
    From<(
        Address<N>,
        U16<N>,
        ProgramID<N>,
        Identifier<N>,
        Vec<InputID<N>>,
        Vec<Value<N>>,
        Signature<N>,
        Field<N>,
        Field<N>,
        Field<N>,
        Field<N>,
    )> for Request<N>
{
    /// Note: See `Request::sign` to create the request. This method is used to eject from a circuit.
    fn from(
        (signer, network_id, program_id, function_name, input_ids, inputs, signature, sk_tag, tvk, tcm, scm): (
            Address<N>,
            U16<N>,
            ProgramID<N>,
            Identifier<N>,
            Vec<InputID<N>>,
            Vec<Value<N>>,
            Signature<N>,
            Field<N>,
            Field<N>,
            Field<N>,
            Field<N>,
        ),
    ) -> Self {
        // Ensure the network ID is correct.
        if *network_id != N::ID {
            N::halt(format!("Invalid network ID. Expected {}, found {}", N::ID, *network_id))
        } else {
            Self { signer, network_id, program_id, function_name, input_ids, inputs, signature, sk_tag, tvk, tcm, scm }
        }
    }
}

impl<N: Network> Request<N> {
    /// Returns the request signer.
    pub const fn signer(&self) -> &Address<N> {
        &self.signer
    }

    /// Returns the network ID.
    pub const fn network_id(&self) -> &U16<N> {
        &self.network_id
    }

    /// Returns the program ID.
    pub const fn program_id(&self) -> &ProgramID<N> {
        &self.program_id
    }

    /// Returns the function name.
    pub const fn function_name(&self) -> &Identifier<N> {
        &self.function_name
    }

    /// Returns the input ID for the transition.
    pub fn input_ids(&self) -> &[InputID<N>] {
        &self.input_ids
    }

    /// Returns the function inputs.
    pub fn inputs(&self) -> &[Value<N>] {
        &self.inputs
    }

    /// Returns the signature for the transition.
    pub const fn signature(&self) -> &Signature<N> {
        &self.signature
    }

    /// Returns the tag secret key `sk_tag`.
    pub const fn sk_tag(&self) -> &Field<N> {
        &self.sk_tag
    }

    /// Returns the transition view key `tvk`.
    pub const fn tvk(&self) -> &Field<N> {
        &self.tvk
    }

    /// Returns the transition public key `tpk`.
    pub fn to_tpk(&self) -> Group<N> {
        // Retrieve the challenge from the signature.
        let challenge = self.signature.challenge();
        // Retrieve the response from the signature.
        let response = self.signature.response();
        // Retrieve `pk_sig` from the signature.
        let pk_sig = self.signature.compute_key().pk_sig();
        // Compute `tpk` as `(challenge * pk_sig) + (response * G)`, equivalent to `r * G`.
        (pk_sig * challenge) + N::g_scalar_multiply(&response)
    }

    /// Returns the transition commitment `tcm`.
    pub const fn tcm(&self) -> &Field<N> {
        &self.tcm
    }

    /// Returns the signer commitment `scm`.
    pub const fn scm(&self) -> &Field<N> {
        &self.scm
    }
}

#[cfg(test)]
mod test_helpers {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 1000;

    pub(super) fn sample_requests(rng: &mut TestRng) -> Vec<Request<CurrentNetwork>> {
        (0..ITERATIONS)
            .map(|i| {
                // Sample a random private key and address.
                let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
                let address = Address::try_from(&private_key).unwrap();

                // Construct a program ID and function name.
                let program_id = ProgramID::from_str("token.aleo").unwrap();
                let function_name = Identifier::from_str("transfer").unwrap();

                // Prepare a record belonging to the address.
                let record_string =
                    format!("{{ owner: {address}.private, token_amount: {i}u64.private, _nonce: 2293253577170800572742339369209137467208538700597121244293392265726446806023group.public }}");

                // Construct four inputs.
                let input_constant = Value::from_str(&format!("{{ token_amount: {i}u128 }}")).unwrap();
                let input_public = Value::from_str(&format!("{{ token_amount: {i}u128 }}")).unwrap();
                let input_private = Value::from_str(&format!("{{ token_amount: {i}u128 }}")).unwrap();
                let input_record = Value::from_str(&record_string).unwrap();
                let input_external_record = Value::from_str(&record_string).unwrap();
                let inputs = vec![input_constant, input_public, input_private, input_record, input_external_record];

                // Construct 'is_root'.
                let is_root = false;

                // Construct the input types.
                let input_types = [
                    ValueType::from_str("amount.constant").unwrap(),
                    ValueType::from_str("amount.public").unwrap(),
                    ValueType::from_str("amount.private").unwrap(),
                    ValueType::from_str("token.record").unwrap(),
                    ValueType::from_str("token.aleo/token.record").unwrap(),
                ];

                // Sample root_tvk.
                let root_tvk = None;

                // Compute the signed request.
                let request =
                    Request::sign(&private_key, program_id, function_name, inputs.into_iter(), &input_types, root_tvk, is_root, rng).unwrap();
                assert!(request.verify(&input_types, is_root));
                request
            })
            .collect()
    }
}
