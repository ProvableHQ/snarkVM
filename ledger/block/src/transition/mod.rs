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

pub mod input;
pub use input::Input;

pub mod output;
pub use output::Output;

mod bytes;
mod merkle;
mod serialize;
mod string;

use console::{
    network::prelude::*,
    program::{
        compute_function_id,
        Ciphertext,
        Identifier,
        InputID,
        OutputID,
        ProgramID,
        Record,
        Register,
        Request,
        Response,
        TransitionLeaf,
        TransitionPath,
        TransitionTree,
        Value,
        ValueType,
        TRANSITION_DEPTH,
    },
    types::{Field, Group},
};

#[derive(Clone, PartialEq, Eq)]
pub struct Transition<N: Network> {
    /// The transition ID.
    id: N::TransitionID,
    /// The program ID.
    program_id: ProgramID<N>,
    /// The function name.
    function_name: Identifier<N>,
    /// The transition inputs.
    inputs: Vec<Input<N>>,
    /// The transition outputs.
    outputs: Vec<Output<N>>,
    /// The transition public key.
    tpk: Group<N>,
    /// The transition commitment.
    tcm: Field<N>,
    /// The transition signer commitment.
    scm: Field<N>,
}

impl<N: Network> Transition<N> {
    /// Initializes a new transition.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        program_id: ProgramID<N>,
        function_name: Identifier<N>,
        inputs: Vec<Input<N>>,
        outputs: Vec<Output<N>>,
        tpk: Group<N>,
        tcm: Field<N>,
        scm: Field<N>,
    ) -> Result<Self> {
        // Compute the transition ID.
        let function_tree = Self::function_tree(&inputs, &outputs)?;
        let id = N::hash_bhp512(&(*function_tree.root(), tcm).to_bits_le())?;
        // Return the transition.
        Ok(Self { id: id.into(), program_id, function_name, inputs, outputs, tpk, tcm, scm })
    }

    /// Initializes a new transition from a request and response.
    pub fn from(
        request: &Request<N>,
        response: &Response<N>,
        output_types: &[ValueType<N>],
        output_registers: &[Option<Register<N>>],
    ) -> Result<Self> {
        let network_id = *request.network_id();
        let program_id = *request.program_id();
        let function_name = *request.function_name();
        let num_inputs = request.inputs().len();

        // Compute the function ID.
        let function_id = compute_function_id(&network_id, &program_id, &function_name)?;

        let inputs = request
            .input_ids()
            .iter()
            .zip_eq(request.inputs())
            .enumerate()
            .map(|(index, (input_id, input))| {
                // Construct the transition input.
                match (input_id, input) {
                    (InputID::Constant(input_hash), Value::Plaintext(plaintext)) => {
                        // Construct the constant input.
                        let input = Input::Constant(*input_hash, Some(plaintext.clone()));
                        // Ensure the input is valid.
                        match input.verify(function_id, request.tcm(), index) {
                            true => Ok(input),
                            false => bail!("Malformed constant transition input: '{input}'"),
                        }
                    }
                    (InputID::Public(input_hash), Value::Plaintext(plaintext)) => {
                        // Construct the public input.
                        let input = Input::Public(*input_hash, Some(plaintext.clone()));
                        // Ensure the input is valid.
                        match input.verify(function_id, request.tcm(), index) {
                            true => Ok(input),
                            false => bail!("Malformed public transition input: '{input}'"),
                        }
                    }
                    (InputID::Private(input_hash), Value::Plaintext(plaintext)) => {
                        // Construct the (console) input index as a field element.
                        let index = Field::from_u16(index as u16);
                        // Compute the ciphertext, with the input view key as `Hash(function ID || tvk || index)`.
                        let ciphertext =
                            plaintext.encrypt_symmetric(N::hash_psd4(&[function_id, *request.tvk(), index])?)?;
                        // Compute the ciphertext hash.
                        let ciphertext_hash = N::hash_psd8(&ciphertext.to_fields()?)?;
                        // Ensure the ciphertext hash matches.
                        ensure!(*input_hash == ciphertext_hash, "The input ciphertext hash is incorrect");
                        // Return the private input.
                        Ok(Input::Private(*input_hash, Some(ciphertext)))
                    }
                    (InputID::Record(_, _, serial_number, tag), Value::Record(..)) => {
                        // Return the input record.
                        Ok(Input::Record(*serial_number, *tag))
                    }
                    (InputID::ExternalRecord(input_hash), Value::Record(..)) => Ok(Input::ExternalRecord(*input_hash)),
                    _ => bail!("Malformed request input: {:?}, {input}", input_id),
                }
            })
            .collect::<Result<Vec<_>>>()?;

        let outputs = response
            .output_ids()
            .iter()
            .zip_eq(response.outputs())
            .zip_eq(output_types)
            .zip_eq(output_registers)
            .enumerate()
            .map(|(index, (((output_id, output), output_type), output_register))| {
                // Construct the transition output.
                match (output_id, output) {
                    (OutputID::Constant(output_hash), Value::Plaintext(plaintext)) => {
                        // Construct the constant output.
                        let output = Output::Constant(*output_hash, Some(plaintext.clone()));
                        // Ensure the output is valid.
                        match output.verify(function_id, request.tcm(), num_inputs + index) {
                            true => Ok(output),
                            false => bail!("Malformed constant transition output: '{output}'"),
                        }
                    }
                    (OutputID::Public(output_hash), Value::Plaintext(plaintext)) => {
                        // Construct the public output.
                        let output = Output::Public(*output_hash, Some(plaintext.clone()));
                        // Ensure the output is valid.
                        match output.verify(function_id, request.tcm(), num_inputs + index) {
                            true => Ok(output),
                            false => bail!("Malformed public transition output: '{output}'"),
                        }
                    }
                    (OutputID::Private(output_hash), Value::Plaintext(plaintext)) => {
                        // Construct the (console) output index as a field element.
                        let index = Field::from_u16(u16::try_from(num_inputs + index)?);
                        // Compute the ciphertext, with the input view key as `Hash(function ID || tvk || index)`.
                        let ciphertext =
                            plaintext.encrypt_symmetric(N::hash_psd4(&[function_id, *request.tvk(), index])?)?;
                        // Compute the ciphertext hash.
                        let ciphertext_hash = N::hash_psd8(&ciphertext.to_fields()?)?;
                        // Ensure the ciphertext hash matches.
                        ensure!(*output_hash == ciphertext_hash, "The output ciphertext hash is incorrect");
                        // Return the private output.
                        Ok(Output::Private(*output_hash, Some(ciphertext)))
                    }
                    (OutputID::Record(commitment, checksum), Value::Record(record)) => {
                        // Retrieve the record name.
                        let record_name = match output_type {
                            ValueType::Record(record_name) => record_name,
                            // Ensure the input type is a record.
                            _ => bail!("Expected a record type at output {index}"),
                        };

                        // Retrieve the output register.
                        let output_register = match output_register {
                            Some(output_register) => output_register,
                            None => bail!("Expected a register to be paired with a record output"),
                        };

                        // Compute the record commitment.
                        let candidate_cm = record.to_commitment(&program_id, record_name)?;
                        // Ensure the commitment matches.
                        ensure!(*commitment == candidate_cm, "The output record commitment is incorrect");

                        // Construct the (console) output index as a field element.
                        let index = Field::from_u64(output_register.locator());
                        // Compute the encryption randomizer as `HashToScalar(tvk || index)`.
                        let randomizer = N::hash_to_scalar_psd2(&[*request.tvk(), index])?;

                        // Encrypt the record, using the randomizer.
                        let record_ciphertext = record.encrypt(randomizer)?;
                        // Compute the record checksum, as the hash of the encrypted record.
                        let ciphertext_checksum = N::hash_bhp1024(&record_ciphertext.to_bits_le())?;
                        // Ensure the checksum matches.
                        ensure!(*checksum == ciphertext_checksum, "The output record ciphertext checksum is incorrect");

                        // Return the record output.
                        Ok(Output::Record(*commitment, *checksum, Some(record_ciphertext)))
                    }
                    (OutputID::ExternalRecord(hash), Value::Record(record)) => {
                        // Construct the (console) output index as a field element.
                        let index = Field::from_u16(u16::try_from(num_inputs + index)?);
                        // Construct the preimage as `(function ID || output || tvk || index)`.
                        let mut preimage = Vec::new();
                        preimage.push(function_id);
                        preimage.extend(record.to_fields()?);
                        preimage.push(*request.tvk());
                        preimage.push(index);
                        // Hash the output to a field element.
                        let candidate_hash = N::hash_psd8(&preimage)?;
                        // Ensure the hash matches.
                        ensure!(*hash == candidate_hash, "The output external hash is incorrect");
                        // Return the record output.
                        Ok(Output::ExternalRecord(*hash))
                    }
                    (OutputID::Future(output_hash), Value::Future(future)) => {
                        // Construct the future output.
                        let output = Output::Future(*output_hash, Some(future.clone()));
                        // Ensure the output is valid.
                        match output.verify(function_id, request.tcm(), num_inputs + index) {
                            true => Ok(output),
                            false => bail!("Malformed future transition output: '{output}'"),
                        }
                    }
                    _ => bail!("Malformed response output: {output_id:?}, {output}"),
                }
            })
            .collect::<Result<Vec<_>>>()?;

        // Retrieve the `tpk`.
        let tpk = request.to_tpk();
        // Retrieve the `tcm`.
        let tcm = *request.tcm();
        // Retrieve the `scm`.
        let scm = *request.scm();
        // Return the transition.
        Self::new(program_id, function_name, inputs, outputs, tpk, tcm, scm)
    }
}

impl<N: Network> Transition<N> {
    /// Returns the transition ID.
    pub const fn id(&self) -> &N::TransitionID {
        &self.id
    }

    /// Returns the program ID.
    pub const fn program_id(&self) -> &ProgramID<N> {
        &self.program_id
    }

    /// Returns the function name.
    pub const fn function_name(&self) -> &Identifier<N> {
        &self.function_name
    }

    /// Returns the inputs.
    pub fn inputs(&self) -> &[Input<N>] {
        &self.inputs
    }

    /// Return the outputs.
    pub fn outputs(&self) -> &[Output<N>] {
        &self.outputs
    }

    /// Returns the transition public key.
    pub const fn tpk(&self) -> &Group<N> {
        &self.tpk
    }

    /// Returns the transition commitment.
    pub const fn tcm(&self) -> &Field<N> {
        &self.tcm
    }

    /// Returns the signer commitment.
    pub const fn scm(&self) -> &Field<N> {
        &self.scm
    }
}

impl<N: Network> Transition<N> {
    /// Returns `true` if this is a `bond_public` transition.
    #[inline]
    pub fn is_bond_public(&self) -> bool {
        self.inputs.len() == 3
            && self.outputs.len() == 1
            && self.program_id.to_string() == "credits.aleo"
            && self.function_name.to_string() == "bond_public"
    }

    /// Returns `true` if this is a `bond_validator` transition.
    #[inline]
    pub fn is_bond_validator(&self) -> bool {
        self.inputs.len() == 3
            && self.outputs.len() == 1
            && self.program_id.to_string() == "credits.aleo"
            && self.function_name.to_string() == "bond_validator"
    }

    /// Returns `true` if this is an `unbond_public` transition.
    #[inline]
    pub fn is_unbond_public(&self) -> bool {
        self.inputs.len() == 2
            && self.outputs.len() == 1
            && self.program_id.to_string() == "credits.aleo"
            && self.function_name.to_string() == "unbond_public"
    }

    /// Returns `true` if this is a `fee_private` transition.
    #[inline]
    pub fn is_fee_private(&self) -> bool {
        self.inputs.len() == 4
            && self.outputs.len() == 1
            && self.program_id.to_string() == "credits.aleo"
            && self.function_name.to_string() == "fee_private"
    }

    /// Returns `true` if this is a `fee_public` transition.
    #[inline]
    pub fn is_fee_public(&self) -> bool {
        self.inputs.len() == 3
            && self.outputs.len() == 1
            && self.program_id.to_string() == "credits.aleo"
            && self.function_name.to_string() == "fee_public"
    }

    /// Returns `true` if this is a `split` transition.
    #[inline]
    pub fn is_split(&self) -> bool {
        self.inputs.len() == 2
            && self.outputs.len() == 2
            && self.program_id.to_string() == "credits.aleo"
            && self.function_name.to_string() == "split"
    }
}

impl<N: Network> Transition<N> {
    /// Returns `true` if the transition contains the given serial number.
    pub fn contains_serial_number(&self, serial_number: &Field<N>) -> bool {
        self.inputs.iter().any(|input| match input {
            Input::Constant(_, _) => false,
            Input::Public(_, _) => false,
            Input::Private(_, _) => false,
            Input::Record(input_sn, _) => input_sn == serial_number,
            Input::ExternalRecord(_) => false,
        })
    }

    /// Returns `true` if the transition contains the given commitment.
    pub fn contains_commitment(&self, commitment: &Field<N>) -> bool {
        self.outputs.iter().any(|output| match output {
            Output::Constant(_, _) => false,
            Output::Public(_, _) => false,
            Output::Private(_, _) => false,
            Output::Record(output_cm, _, _) => output_cm == commitment,
            Output::ExternalRecord(_) => false,
            Output::Future(_, _) => false,
        })
    }
}

impl<N: Network> Transition<N> {
    /// Returns the record with the corresponding commitment, if it exists.
    pub fn find_record(&self, commitment: &Field<N>) -> Option<&Record<N, Ciphertext<N>>> {
        self.outputs.iter().find_map(|output| match output {
            Output::Constant(_, _) => None,
            Output::Public(_, _) => None,
            Output::Private(_, _) => None,
            Output::Record(output_cm, _, Some(record)) if output_cm == commitment => Some(record),
            Output::Record(_, _, _) => None,
            Output::ExternalRecord(_) => None,
            Output::Future(_, _) => None,
        })
    }
}

impl<N: Network> Transition<N> {
    /* Input */

    /// Returns the input IDs.
    pub fn input_ids(&self) -> impl '_ + ExactSizeIterator<Item = &Field<N>> {
        self.inputs.iter().map(Input::id)
    }

    /// Returns an iterator over the serial numbers, for inputs that are records.
    pub fn serial_numbers(&self) -> impl '_ + Iterator<Item = &Field<N>> {
        self.inputs.iter().flat_map(Input::serial_number)
    }

    /// Returns an iterator over the tags, for inputs that are records.
    pub fn tags(&self) -> impl '_ + Iterator<Item = &Field<N>> {
        self.inputs.iter().flat_map(Input::tag)
    }

    /* Output */

    /// Returns the output IDs.
    pub fn output_ids(&self) -> impl '_ + ExactSizeIterator<Item = &Field<N>> {
        self.outputs.iter().map(Output::id)
    }

    /// Returns an iterator over the commitments, for outputs that are records.
    pub fn commitments(&self) -> impl '_ + Iterator<Item = &Field<N>> {
        self.outputs.iter().flat_map(Output::commitment)
    }

    /// Returns an iterator over the nonces, for outputs that are records.
    pub fn nonces(&self) -> impl '_ + Iterator<Item = &Group<N>> {
        self.outputs.iter().flat_map(Output::nonce)
    }

    /// Returns an iterator over the output records, as a tuple of `(commitment, record)`.
    pub fn records(&self) -> impl '_ + Iterator<Item = (&Field<N>, &Record<N, Ciphertext<N>>)> {
        self.outputs.iter().flat_map(Output::record)
    }
}

impl<N: Network> Transition<N> {
    /// Returns the transition ID, and consumes `self`.
    pub fn into_id(self) -> N::TransitionID {
        self.id
    }

    /* Input */

    /// Returns a consuming iterator over the serial numbers, for inputs that are records.
    pub fn into_serial_numbers(self) -> impl Iterator<Item = Field<N>> {
        self.inputs.into_iter().flat_map(Input::into_serial_number)
    }

    /// Returns a consuming iterator over the tags, for inputs that are records.
    pub fn into_tags(self) -> impl Iterator<Item = Field<N>> {
        self.inputs.into_iter().flat_map(Input::into_tag)
    }

    /* Output */

    /// Returns a consuming iterator over the commitments, for outputs that are records.
    pub fn into_commitments(self) -> impl Iterator<Item = Field<N>> {
        self.outputs.into_iter().flat_map(Output::into_commitment)
    }

    /// Returns a consuming iterator over the nonces, for outputs that are records.
    pub fn into_nonces(self) -> impl Iterator<Item = Group<N>> {
        self.outputs.into_iter().flat_map(Output::into_nonce)
    }

    /// Returns a consuming iterator over the output records, as a tuple of `(commitment, record)`.
    pub fn into_records(self) -> impl Iterator<Item = (Field<N>, Record<N, Ciphertext<N>>)> {
        self.outputs.into_iter().flat_map(Output::into_record)
    }

    /// Returns the transition public key, and consumes `self`.
    pub fn into_tpk(self) -> Group<N> {
        self.tpk
    }
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use crate::Transaction;

    type CurrentNetwork = console::network::MainnetV0;

    /// Samples a random transition.
    pub(crate) fn sample_transition(rng: &mut TestRng) -> Transition<CurrentNetwork> {
        if let Transaction::Execute(_, execution, _) =
            crate::transaction::test_helpers::sample_execution_transaction_with_fee(true, rng)
        {
            execution.into_transitions().next().unwrap()
        } else {
            unreachable!()
        }
    }
}
