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

use super::*;

impl<A: Aleo> Response<A> {
    /// Returns the injected circuit outputs, given the number of inputs, tvk, tcm, outputs, and output types.
    pub fn process_outputs_from_callback(
        network_id: &U16<A>,
        program_id: &ProgramID<A>,
        function_name: &Identifier<A>,
        num_inputs: usize,
        tvk: &Field<A>,
        tcm: &Field<A>,
        outputs: Vec<console::Value<A::Network>>,        // Note: Console type
        output_types: &[console::ValueType<A::Network>], // Note: Console type
    ) -> Vec<Value<A>> {
        // Compute the function ID.
        let function_id = compute_function_id(network_id, program_id, function_name);

        match outputs
            .iter()
            .zip_eq(output_types)
            .enumerate()
            .map(|(index, (output, output_types))| {
                match output_types {
                    // For a constant output, compute the hash (using `tcm`) of the output.
                    console::ValueType::Constant(..) => {
                        // Inject the output as `Mode::Constant`.
                        let output = Value::new(Mode::Constant, output.clone());
                        // Ensure the output is a plaintext.
                        ensure!(matches!(output, Value::Plaintext(..)), "Expected a plaintext output");

                        // Prepare the index as a constant field element.
                        let output_index = Field::constant(console::Field::from_u16((num_inputs + index) as u16));
                        // Construct the preimage as `(function ID || output || tcm || index)`.
                        let mut preimage = Vec::new();
                        preimage.push(function_id.clone());
                        preimage.extend(output.to_fields());
                        preimage.push(tcm.clone());
                        preimage.push(output_index);

                        // Hash the output to a field element.
                        match &output {
                            // Return the output ID.
                            Value::Plaintext(..) => Ok((OutputID::constant(A::hash_psd8(&preimage)), output)),
                            // Ensure the output is a plaintext.
                            Value::Record(..) => A::halt("Expected a plaintext output, found a record output"),
                            Value::Future(..) => A::halt("Expected a plaintext output, found a future output"),
                        }
                    }
                    // For a public output, compute the hash (using `tcm`) of the output.
                    console::ValueType::Public(..) => {
                        // Inject the output as `Mode::Private`.
                        let output = Value::new(Mode::Private, output.clone());
                        // Ensure the output is a plaintext.
                        ensure!(matches!(output, Value::Plaintext(..)), "Expected a plaintext output");

                        // Prepare the index as a constant field element.
                        let output_index = Field::constant(console::Field::from_u16((num_inputs + index) as u16));
                        // Construct the preimage as `(function ID || output || tcm || index)`.
                        let mut preimage = Vec::new();
                        preimage.push(function_id.clone());
                        preimage.extend(output.to_fields());
                        preimage.push(tcm.clone());
                        preimage.push(output_index);

                        // Hash the output to a field element.
                        match &output {
                            // Return the output ID.
                            Value::Plaintext(..) => Ok((OutputID::public(A::hash_psd8(&preimage)), output)),
                            // Ensure the output is a plaintext.
                            Value::Record(..) => A::halt("Expected a plaintext output, found a record output"),
                            Value::Future(..) => A::halt("Expected a plaintext output, found a future output"),
                        }
                    }
                    // For a private output, compute the ciphertext (using `tvk`) and hash the ciphertext.
                    console::ValueType::Private(..) => {
                        // Inject the output as `Mode::Private`.
                        let output = Value::new(Mode::Private, output.clone());
                        // Ensure the output is a plaintext.
                        ensure!(matches!(output, Value::Plaintext(..)), "Expected a plaintext output");

                        // Prepare the index as a constant field element.
                        let output_index = Field::constant(console::Field::from_u16((num_inputs + index) as u16));
                        // Compute the output view key as `Hash(function ID || tvk || index)`.
                        let output_view_key = A::hash_psd4(&[function_id.clone(), tvk.clone(), output_index]);
                        // Compute the ciphertext.
                        let ciphertext = match &output {
                            Value::Plaintext(plaintext) => plaintext.encrypt_symmetric(output_view_key),
                            // Ensure the output is a plaintext.
                            Value::Record(..) => A::halt("Expected a plaintext output, found a record output"),
                            Value::Future(..) => A::halt("Expected a plaintext output, found a future output"),
                        };
                        // Return the output ID.
                        Ok((OutputID::private(A::hash_psd8(&ciphertext.to_fields())), output))
                    }
                    // For a record output, compute the record commitment.
                    console::ValueType::Record(record_name) => {
                        // Inject the output as `Mode::Private`.
                        let output = Value::new(Mode::Private, output.clone());

                        // Retrieve the record.
                        let record = match &output {
                            Value::Record(record) => record,
                            // Ensure the output is a record.
                            Value::Plaintext(..) => A::halt("Expected a record output, found a plaintext output"),
                            Value::Future(..) => A::halt("Expected a record output, found a future output"),
                        };
                        // Compute the record commitment.
                        let commitment = record.to_commitment(program_id, &Identifier::constant(*record_name));

                        // Return the output ID.
                        // Note: Because this is a callback, the output ID is an **external record** ID.
                        Ok((OutputID::external_record(commitment), output))
                    }
                    // For an external record output, compute the hash (using `tvk`) of the output.
                    console::ValueType::ExternalRecord(..) => {
                        // Inject the output as `Mode::Private`.
                        let output = Value::new(Mode::Private, output.clone());
                        // Ensure the output is a record.
                        ensure!(matches!(output, Value::Record(..)), "Expected a record output");

                        // Prepare the index as a constant field element.
                        let output_index = Field::constant(console::Field::from_u16((num_inputs + index) as u16));
                        // Construct the preimage as `(function ID || output || tvk || index)`.
                        let mut preimage = Vec::new();
                        preimage.push(function_id.clone());
                        preimage.extend(output.to_fields());
                        preimage.push(tvk.clone());
                        preimage.push(output_index);

                        // Return the output ID.
                        match &output {
                            Value::Record(..) => Ok((OutputID::external_record(A::hash_psd8(&preimage)), output)),
                            // Ensure the output is a record.
                            Value::Plaintext(..) => A::halt("Expected a record output, found a plaintext output"),
                            Value::Future(..) => A::halt("Expected a record output, found a future output"),
                        }
                    }
                    // For a future output, compute the hash (using `tcm`) of the output.
                    console::ValueType::Future(..) => {
                        // Inject the output as `Mode::Private`.
                        let output = Value::new(Mode::Private, output.clone());
                        // Ensure the output is a future.
                        ensure!(matches!(output, Value::Future(..)), "Expected a future output");

                        // Prepare the index as a constant field element.
                        let output_index = Field::constant(console::Field::from_u16((num_inputs + index) as u16));
                        // Construct the preimage as `(function ID || output || tcm || index)`.
                        let mut preimage = Vec::new();
                        preimage.push(function_id.clone());
                        preimage.extend(output.to_fields());
                        preimage.push(tcm.clone());
                        preimage.push(output_index);

                        // Hash the output to a field element.
                        match &output {
                            // Return the output ID.
                            Value::Future(..) => Ok((OutputID::future(A::hash_psd8(&preimage)), output)),
                            // Ensure the output is a future.
                            Value::Plaintext(..) => A::halt("Expected a future output, found a plaintext output"),
                            Value::Record(..) => A::halt("Expected a future output, found a record output"),
                        }
                    }
                }
            })
            .collect::<Result<Vec<_>>>()
        {
            Ok(outputs) => {
                // Unzip the output IDs from the output values.
                let (_, outputs): (Vec<OutputID<A>>, _) = outputs.into_iter().unzip();
                // Return the outputs.
                outputs
            }
            Err(error) => A::halt(error.to_string()),
        }
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::Circuit;
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    pub(crate) const ITERATIONS: usize = 20;

    fn check_from_callback(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        use console::Network;

        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a `tvk`.
            let tvk = console::Field::rand(rng);
            // Compute the transition commitment as `Hash(tvk)`.
            let tcm = <Circuit as Environment>::Network::hash_psd2(&[tvk])?;

            // Compute the nonce.
            let index = console::Field::from_u64(8);
            let randomizer = <Circuit as Environment>::Network::hash_to_scalar_psd2(&[tvk, index]).unwrap();
            let nonce = <Circuit as Environment>::Network::g_scalar_multiply(&randomizer);

            // Construct the outputs.
            let output_constant = console::Value::<<Circuit as Environment>::Network>::Plaintext(
                console::Plaintext::from_str("{ token_amount: 9876543210u128 }").unwrap(),
            );
            let output_public = console::Value::<<Circuit as Environment>::Network>::Plaintext(
                console::Plaintext::from_str("{ token_amount: 9876543210u128 }").unwrap(),
            );
            let output_private = console::Value::<<Circuit as Environment>::Network>::Plaintext(
                console::Plaintext::from_str("{ token_amount: 9876543210u128 }").unwrap(),
            );
            let output_record = console::Value::<<Circuit as Environment>::Network>::Record(console::Record::from_str(&format!("{{ owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah.private, token_amount: 100u64.private, _nonce: {nonce}.public }}")).unwrap());
            let output_external_record = console::Value::<<Circuit as Environment>::Network>::Record(console::Record::from_str("{ owner: aleo1d5hg2z3ma00382pngntdp68e74zv54jdxy249qhaujhks9c72yrs33ddah.private, token_amount: 100u64.private, _nonce: 0group.public }").unwrap());
            let outputs = vec![output_constant, output_public, output_private, output_record, output_external_record];

            // Construct the output types.
            let output_types = vec![
                console::ValueType::from_str("amount.constant").unwrap(),
                console::ValueType::from_str("amount.public").unwrap(),
                console::ValueType::from_str("amount.private").unwrap(),
                console::ValueType::from_str("token.record").unwrap(),
                console::ValueType::from_str("token.aleo/token.record").unwrap(),
            ];

            // Construct the output registers.
            let output_registers = vec![
                Some(console::Register::Locator(5)),
                Some(console::Register::Locator(6)),
                Some(console::Register::Locator(7)),
                Some(console::Register::Locator(8)),
                Some(console::Register::Locator(9)),
            ];

            // Construct a network ID.
            let network_id = console::U16::new(<Circuit as Environment>::Network::ID);
            // Construct a program ID.
            let program_id = console::ProgramID::from_str("test.aleo")?;
            // Construct a function name.
            let function_name = console::Identifier::from_str("check")?;

            // Construct the response.
            let response = console::Response::new(
                &network_id,
                &program_id,
                &function_name,
                4,
                &tvk,
                &tcm,
                outputs.clone(),
                &output_types,
                &output_registers,
            )?;
            // assert!(response.verify());

            // Inject the network ID, program ID, function name, `tvk`, `tcm`.
            let network_id = U16::<Circuit>::constant(network_id);
            let program_id = ProgramID::<Circuit>::new(mode, program_id);
            let function_name = Identifier::<Circuit>::new(mode, function_name);
            let tvk = Field::<Circuit>::new(mode, tvk);
            let tcm = Field::<Circuit>::new(mode, tcm);

            Circuit::scope(format!("Response {i}"), || {
                let outputs = Response::process_outputs_from_callback(
                    &network_id,
                    &program_id,
                    &function_name,
                    4,
                    &tvk,
                    &tcm,
                    response.outputs().to_vec(),
                    &output_types,
                );
                assert_eq!(response.outputs(), outputs.eject_value());
                match mode.is_constant() {
                    true => assert_scope!(<=num_constants, num_public, num_private, num_constraints),
                    false => assert_scope!(<=num_constants, num_public, num_private, num_constraints),
                }
            });

            // Compute the response using outputs (circuit).
            let outputs = Inject::new(mode, response.outputs().to_vec());
            let candidate_b = Response::from_outputs(
                &network_id,
                &program_id,
                &function_name,
                4,
                &tvk,
                &tcm,
                outputs,
                &output_types,
                &output_registers,
            );
            assert_eq!(response, candidate_b.eject_value());

            Circuit::reset();
        }
        Ok(())
    }

    // Note: These counts are correct. At this (high) level of a program, we override the default mode in many cases,
    // based on the user-defined visibility in the types. Thus, we have nonzero public, private, and constraint values.
    // These bounds are determined experimentally.

    #[test]
    fn test_from_callback_constant() -> Result<()> {
        check_from_callback(Mode::Constant, 20844, 5, 4922, 4931)
    }

    #[test]
    fn test_from_callback_public() -> Result<()> {
        check_from_callback(Mode::Public, 20844, 5, 6217, 6226)
    }

    #[test]
    fn test_from_callback_private() -> Result<()> {
        check_from_callback(Mode::Private, 20844, 5, 6217, 6226)
    }
}
