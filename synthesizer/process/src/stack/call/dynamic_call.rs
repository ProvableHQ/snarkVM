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

use super::*;

use crate::stack::helpers::{sample_identifier, sample_program_id};
use circuit::prelude::ToField;
use console::{
    prelude::FromField,
    program::{DynamicFuture, Identifier, Literal, Plaintext, ProgramID},
};

impl<N: Network> CallTrait<N> for DynamicCall<N> {
    /// Evaluates the instruction.
    #[inline]
    fn evaluate<A: circuit::Aleo<Network = N>>(
        &self,
        stack: &(impl StackEvaluate<N> + StackMatches<N> + StackProgram<N>),
        registers: &mut Registers<N, A>,
    ) -> Result<()> {
        let timer = timer!("DynamicCall::evaluate");

        // Load the program ID name.
        let program_id_name = match registers.load(stack, self.program_id_name())? {
            Value::Plaintext(Plaintext::Literal(Literal::Field(field), _)) => Identifier::from_field(&field)?,
            _ => bail!("Expected the operand '{}' to be a valid field element", self.program_id_name()),
        };

        let program_id = ProgramID::try_from((program_id_name, Identifier::from_str("aleo")?))?;

        // Load the function name.
        let function_name = match registers.load(stack, self.function_name())? {
            Value::Plaintext(Plaintext::Literal(Literal::Field(field), _)) => Identifier::from_field(&field)?,
            _ => bail!("Expected the operand '{}' to be a valid field element", self.function_name()),
        };

        // Ensure that the program ID is not the current program ID.
        ensure!(stack.program_id() != &program_id, "Cannot dynamically call a function in the current program");

        // Load the operands values.
        let inputs: Vec<_> = self.operands().iter().map(|operand| registers.load(stack, operand)).try_collect()?;

        // Retrieve the substack.
        let substack = stack.get_external_stack(&program_id)?;
        lap!(timer, "Retrieved the substack");

        // Retrieve the function from the substack.
        let function = substack.program().get_function_ref(&function_name)?;

        // Ensure the number of inputs matches the number of input statements.
        if function.inputs().len() != inputs.len() {
            bail!("Expected {} inputs, found {}", function.inputs().len(), inputs.len())
        }
        // Set the (console) caller.
        let console_caller = Some(*stack.program_id());
        // Evaluate the function.
        let response = substack.evaluate_function::<A>(registers.call_stack(), console_caller)?;
        // Load the outputs.
        let outputs = response.outputs().to_vec();
        lap!(timer, "Computed outputs");

        // Assign the outputs to the destination registers.
        for (output, register) in outputs.into_iter().zip_eq(&self.destinations()) {
            // If the output is a future, then store it as a dynamic future.
            let output = if let Value::Future(future) = output {
                // Convert the future to a dynamic future.
                Value::DynamicFuture(DynamicFuture::from_future(&future)?)
            } else {
                output
            };
            // Assign the output to the register.
            registers.store(stack, register, output)?;
        }
        finish!(timer);

        Ok(())
    }

    /// Executes the instruction.
    #[inline]
    fn execute<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(
        &self,
        stack: &(impl StackEvaluate<N> + StackExecute<N> + StackMatches<N> + StackKeys<N> + StackProgram<N>),
        registers: &mut (
                 impl RegistersCall<N>
                 + RegistersSigner<N>
                 + RegistersSignerCircuit<N, A>
                 + RegistersLoadCircuit<N, A>
                 + RegistersStoreCircuit<N, A>
             ),
        rng: &mut R,
    ) -> Result<()> {
        let timer = timer!("DynamicCall::execute");

        // Determine the input types from the instruction.
        let input_types = self.operand_types();
        // Determine the output types from the instruction.
        let output_types = self.destination_types();

        // Load the program ID name.
        let program_id_name_as_field = match registers.load_circuit(stack, self.program_id_name())? {
            circuit::Value::Plaintext(circuit::Plaintext::Literal(circuit::Literal::Field(field), _)) => field,
            _ => bail!("Expected the operand '{}' to be a valid field element", self.program_id_name()),
        };

        // Load the function name.
        let function_name_as_field = match registers.load_circuit(stack, self.function_name())? {
            circuit::Value::Plaintext(circuit::Plaintext::Literal(circuit::Literal::Field(field), _)) => field,
            _ => bail!("Expected the operand '{}' to be a valid field element", self.function_name()),
        };

        // Load the operands values.
        let inputs: Vec<_> =
            self.operands().iter().map(|operand| registers.load_circuit(stack, operand)).try_collect()?;

        // Get the the program ID and function name.
        // - If we are in `Synthesize` or `CheckDeployment` mode, sample random values.
        //      This is necessary because the registers are initialized to random values, which often
        //      do not map to valid identifiers.
        // - Otherwise, eject the values from the registers.
        let (program_id_console, function_name_console) = match registers.call_stack() {
            CallStack::Synthesize(..) | CallStack::CheckDeployment(..) => {
                // Sample a random program ID.
                let program_id_console = sample_program_id(rng)?;
                // Sample a random function name.
                let function_name_console = sample_identifier(false, rng)?;

                (program_id_console, function_name_console)
            }
            CallStack::Authorize(..) | CallStack::Evaluate(..) | CallStack::Execute(..) | CallStack::PackageRun(..) => {
                // Construct the program ID.
                let program_id_name_console = Identifier::from_field(&program_id_name_as_field.eject_value())?;
                let program_id_console = ProgramID::try_from((program_id_name_console, Identifier::from_str("aleo")?))?;

                // Construct the function name.
                let function_name_console = Identifier::from_field(&function_name_as_field.eject_value())?;
                let function_name_string = function_name_console.to_string();

                // Ensure that the program ID is not the current program ID.
                ensure!(
                    stack.program_id() != &program_id_console,
                    "Cannot dynamically call a function in the same program"
                );

                // Check the external call.
                let is_credits_program = program_id_console.to_string() == "credits.aleo";
                let is_fee_private = &function_name_string == "fee_private";
                let is_fee_public = &function_name_string == "fee_public";

                // Ensure the external call is not to 'credits.aleo/fee_private' or 'credits.aleo/fee_public'.
                if is_credits_program && (is_fee_private || is_fee_public) {
                    bail!("Cannot perform an external call to 'credits.aleo/fee_private' or 'credits.aleo/fee_public'.")
                }

                // Retrieve the substack.
                let substack = stack.get_external_stack(&program_id_console)?;

                // Retrieve the function from the substack.
                let function = substack.program().get_function_ref(&function_name_console)?;

                // Retrieve the number of inputs.
                let num_inputs = function.inputs().len();
                // Ensure the number of inputs matches the number of input statements.
                if num_inputs != inputs.len() {
                    bail!("Expected {} inputs, found {}", num_inputs, inputs.len())
                }

                (program_id_console, function_name_console)
            }
        };

        // A closure to lazily get the substack.
        let get_substack = || {
            // Retrieve the substack.
            stack.get_external_stack(&program_id_console)
        };

        // If we are not handling the root request, retrieve the root request's tvk
        let root_tvk = registers.root_tvk().ok();

        lap!(timer, "Execute the function");

        // Retrieve the number of public variables in the circuit.
        let num_public = A::num_public();

        // Indicate that external calls are never a root request.
        let is_root = false;

        use circuit::Eject;
        // Eject the existing circuit.
        let r1cs = A::eject_r1cs_and_reset();
        let (request, outputs) = {
            // Eject the circuit inputs.
            let inputs = inputs.eject_value();

            // Set the (console) caller.
            let console_caller = Some(*stack.program_id());
            // Check if the substack has a proving key or not.

            match registers.call_stack() {
                // If the circuit is in authorize mode, then add any external calls to the stack.
                CallStack::Authorize(_, private_key, authorization) => {
                    // Compute the request.
                    let request = Request::sign(
                        &private_key,
                        program_id_console,
                        function_name_console,
                        inputs.iter(),
                        input_types,
                        root_tvk,
                        is_root,
                        rng,
                    )?;

                    // Retrieve the call stack.
                    let mut call_stack = registers.call_stack();
                    // Push the request onto the call stack.
                    call_stack.push(request.clone())?;

                    // Add the request to the authorization.
                    authorization.push(request.clone())?;

                    // Execute the request.
                    let response =
                        get_substack()?.execute_function::<A, R>(call_stack, console_caller, root_tvk, rng)?;

                    // Return the request and outputs.
                    (request, response.outputs().to_vec())
                }
                // In Synthesize mode (with an existing proving key) or CheckDeployment mode, we generate dummy outputs to avoid building a full sub-circuit.
                CallStack::Synthesize(_, private_key, _) | CallStack::CheckDeployment(_, private_key, ..) => {
                    // Compute the request.
                    let request = Request::sign(
                        &private_key,
                        program_id_console,
                        function_name_console,
                        inputs.iter(),
                        input_types,
                        root_tvk,
                        is_root,
                        rng,
                    )?;

                    // Compute the address.
                    let address = Address::try_from(&private_key)?;

                    // For each output, sample a dummy value.
                    let outputs = output_types
                        .iter()
                        .map(|output_type| {
                            // TODO (@d0cd) Verify that this is sound.
                            // If the output type is a dynamic future, sample a random future value.
                            match output_type {
                                ValueType::DynamicFuture => Ok(Value::Future(stack.sample_random_future(rng)?)),
                                _ => stack.sample_value(&address, output_type, rng),
                            }
                        })
                        .collect::<Result<Vec<_>>>()?;

                    // Return the request and outputs.
                    (request, outputs)
                }
                // In PackageRun mode, we sign and execute the request once.
                CallStack::PackageRun(_, private_key, ..) => {
                    // Compute the request.
                    let request = Request::sign(
                        &private_key,
                        program_id_console,
                        function_name_console,
                        inputs.iter(),
                        input_types,
                        root_tvk,
                        is_root,
                        rng,
                    )?;

                    // Retrieve the call stack.
                    let mut call_stack = registers.call_stack();
                    // Push the request onto the call stack.
                    call_stack.push(request.clone())?;

                    // Evaluate the request.
                    let response =
                        get_substack()?.execute_function::<A, _>(call_stack, console_caller, root_tvk, rng)?;

                    // Return the request and outputs.
                    (request, response.outputs().to_vec())
                }
                // If the circuit is in evaluate mode, then throw an error.
                CallStack::Evaluate(..) => {
                    bail!("Cannot 'execute' a function in 'evaluate' mode.")
                }
                // If the circuit is in execute mode, then evaluate and execute the instructions.
                CallStack::Execute(authorization, ..) => {
                    // Retrieve the next request (without popping it).
                    let request = authorization.peek_next()?;
                    // Ensure the inputs match the original inputs.
                    request.inputs().iter().zip_eq(&inputs).try_for_each(|(request_input, input)| {
                        ensure!(request_input == input, "Inputs do not match in a 'call' instruction.");
                        Ok(())
                    })?;

                    // Get the substack.
                    let substack = get_substack()?;
                    // Evaluate the function, and load the outputs.
                    let console_response =
                        substack.evaluate_function::<A>(registers.call_stack().replicate(), console_caller)?;
                    // Execute the request.
                    let response =
                        substack.execute_function::<A, R>(registers.call_stack(), console_caller, root_tvk, rng)?;
                    // Ensure the values are equal.
                    if console_response.outputs() != response.outputs() {
                        #[cfg(debug_assertions)]
                        eprintln!("\n{:#?} != {:#?}\n", console_response.outputs(), response.outputs());
                        bail!("Function '{}' outputs do not match in a 'call' instruction.", function_name_console)
                    }
                    // Return the request and outputs.
                    (request, response.outputs().to_vec())
                }
            }
        };
        lap!(timer, "Computed the request and response");

        // Inject the existing circuit.
        A::inject_r1cs(r1cs);

        use circuit::Inject;

        // Inject the network ID as `Mode::Constant`.
        let network_id = circuit::U16::constant(*request.network_id());
        // Inject the program ID as `Mode::Public`.
        let program_id = circuit::ProgramID::new_unchecked(circuit::Mode::Public, program_id_console);
        // Inject the function name as `Mode::Public`.
        let function_name = circuit::Identifier::new_unchecked(circuit::Mode::Public, function_name_console);

        // Ensure the number of public variables remains the same.
        ensure!(A::num_public() == num_public + 3, "Forbidden: 'dcall' injected excess public variables");

        // Inject the `signer` (from the request) as `Mode::Private`.
        let signer = circuit::Address::new(circuit::Mode::Private, *request.signer());
        // Inject the `sk_tag` (from the request) as `Mode::Private`.
        let sk_tag = circuit::Field::new(circuit::Mode::Private, *request.sk_tag());
        // Inject the `tvk` (from the request) as `Mode::Private`.
        let tvk = circuit::Field::new(circuit::Mode::Private, *request.tvk());
        // Inject the `tcm` (from the request) as `Mode::Public`.
        let tcm = circuit::Field::new(circuit::Mode::Public, *request.tcm());
        // Compute the transition commitment as `Hash(tvk)`.
        let candidate_tcm = A::hash_psd2(&[tvk.clone()]);
        // Ensure the transition commitment matches the computed transition commitment.
        A::assert_eq(&tcm, candidate_tcm);
        // Inject the input IDs (from the request) as `Mode::Public`.
        let input_ids = request
            .input_ids()
            .iter()
            .map(|input_id| circuit::InputID::new(circuit::Mode::Public, *input_id))
            .collect::<Vec<_>>();

        // Ensure that the injected program ID matches the one in dynamic call.
        A::assert_eq(program_id.name().to_field(), program_id_name_as_field);

        // Ensure the function name matches the one in dynamic call.
        A::assert_eq(function_name.to_field(), &function_name);

        // Ensure the candidate input IDs match their computed inputs.
        let (check_input_ids, _) = circuit::Request::check_input_ids::<false>(
            &network_id,
            &program_id,
            &function_name,
            &input_ids,
            &inputs,
            input_types,
            &signer,
            &sk_tag,
            &tvk,
            &tcm,
            None,
        );
        A::assert(check_input_ids);
        lap!(timer, "Checked the input ids");

        // Inject the outputs as `Mode::Private` (with the 'tcm' and output IDs as `Mode::Public`).
        let outputs = circuit::Response::process_outputs_from_dynamic_callback(
            &network_id,
            &program_id,
            &function_name,
            self.operands().len(),
            &tvk,
            &tcm,
            outputs,
            output_types,
        );
        lap!(timer, "Checked the outputs");

        // Assign the outputs to the destination registers.
        for (output, register) in outputs.into_iter().zip_eq(&self.destinations()) {
            // Assign the output to the register.
            registers.store_circuit(stack, register, output)?;
        }
        lap!(timer, "Assigned the outputs to registers");

        finish!(timer);

        Ok(())
    }
}
