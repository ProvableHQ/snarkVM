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

use crate::{stack::Address, CallStack, Registers, RegistersCall, StackEvaluate, StackExecute};
use aleo_std::prelude::{finish, lap, timer};
use console::{
    account::Field,
    network::prelude::*,
    program::{Register, Request, Value, ValueType},
};
use synthesizer_program::{
    Call,
    CallOperator,
    Operand,
    RegistersLoad,
    RegistersLoadCircuit,
    RegistersSigner,
    RegistersSignerCircuit,
    RegistersStore,
    RegistersStoreCircuit,
    StackMatches,
    StackProgram,
};

pub trait CallTrait<N: Network> {
    /// Evaluates the instruction.
    fn evaluate<A: circuit::Aleo<Network = N>>(
        &self,
        stack: &(impl StackEvaluate<N> + StackMatches<N> + StackProgram<N>),
        registers: &mut Registers<N, A>,
    ) -> Result<()>;

    /// Executes the instruction.
    fn execute<A: circuit::Aleo<Network = N>, R: CryptoRng + Rng>(
        &self,
        stack: &(impl StackEvaluate<N> + StackExecute<N> + StackMatches<N> + StackProgram<N>),
        registers: &mut (
                 impl RegistersCall<N>
                 + RegistersSigner<N>
                 + RegistersSignerCircuit<N, A>
                 + RegistersLoadCircuit<N, A>
                 + RegistersStoreCircuit<N, A>
             ),
        rng: &mut R,
    ) -> Result<()>;
}

impl<N: Network> CallTrait<N> for Call<N> {
    /// Evaluates the instruction.
    #[inline]
    fn evaluate<A: circuit::Aleo<Network = N>>(
        &self,
        stack: &(impl StackEvaluate<N> + StackMatches<N> + StackProgram<N>),
        registers: &mut Registers<N, A>,
    ) -> Result<()> {
        let timer = timer!("Call::evaluate");

        // Load the operands values.
        let inputs: Vec<_> = self.operands().iter().map(|operand| registers.load(stack, operand)).try_collect()?;

        // Retrieve the substack and resource.
        let (substack, resource) = match self.operator() {
            // Retrieve the call stack and resource from the locator.
            CallOperator::Locator(locator) => {
                (stack.get_external_stack(locator.program_id())?.as_ref(), locator.resource())
            }
            CallOperator::Resource(resource) => {
                // TODO (howardwu): Revisit this decision to forbid calling internal functions. A record cannot be spent again.
                //  But there are legitimate uses for passing a record through to an internal function.
                //  We could invoke the internal function without a state transition, but need to match visibility.
                if stack.program().contains_function(resource) {
                    bail!("Cannot call '{resource}'. Use a closure ('closure {resource}:') instead.")
                }

                (stack, resource)
            }
        };
        lap!(timer, "Retrieved the substack and resource");

        // If the operator is a closure, retrieve the closure and compute the output.
        let outputs = if let Ok(closure) = substack.program().get_closure(resource) {
            // Ensure the number of inputs matches the number of input statements.
            if closure.inputs().len() != inputs.len() {
                bail!("Expected {} inputs, found {}", closure.inputs().len(), inputs.len())
            }
            // Evaluate the closure, and load the outputs.
            substack.evaluate_closure::<A>(
                &closure,
                &inputs,
                registers.call_stack(),
                registers.signer()?,
                registers.caller()?,
                registers.tvk()?,
            )?
        }
        // If the operator is a function, retrieve the function and compute the output.
        else if let Ok(function) = substack.program().get_function(resource) {
            // Ensure the number of inputs matches the number of input statements.
            if function.inputs().len() != inputs.len() {
                bail!("Expected {} inputs, found {}", function.inputs().len(), inputs.len())
            }
            // Set the (console) caller.
            let console_caller = Some(*stack.program_id());
            // Evaluate the function.
            let response = substack.evaluate_function::<A>(registers.call_stack(), console_caller)?;
            // Load the outputs.
            response.outputs().to_vec()
        }
        // Else, throw an error.
        else {
            bail!("Call operator '{}' is invalid or unsupported.", self.operator())
        };
        lap!(timer, "Computed outputs");

        // Assign the outputs to the destination registers.
        for (output, register) in outputs.into_iter().zip_eq(&self.destinations()) {
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
        stack: &(impl StackEvaluate<N> + StackExecute<N> + StackMatches<N> + StackProgram<N>),
        registers: &mut (
                 impl RegistersCall<N>
                 + RegistersSigner<N>
                 + RegistersSignerCircuit<N, A>
                 + RegistersLoadCircuit<N, A>
                 + RegistersStoreCircuit<N, A>
             ),
        rng: &mut R,
    ) -> Result<()> {
        let timer = timer!("Call::execute");

        // Load the operands values.
        let inputs: Vec<_> =
            self.operands().iter().map(|operand| registers.load_circuit(stack, operand)).try_collect()?;

        // Retrieve the substack and resource.
        let (substack, resource) = match self.operator() {
            // Retrieve the call stack and resource from the locator.
            CallOperator::Locator(locator) => {
                // Check the external call locator.
                let function_name = locator.name().to_string();
                let is_credits_program = &locator.program_id().to_string() == "credits.aleo";
                let is_fee_private = &function_name == "fee_private";
                let is_fee_public = &function_name == "fee_public";

                // Ensure the external call is not to 'credits.aleo/fee_private' or 'credits.aleo/fee_public'.
                if is_credits_program && (is_fee_private || is_fee_public) {
                    bail!("Cannot perform an external call to 'credits.aleo/fee_private' or 'credits.aleo/fee_public'.")
                } else {
                    (stack.get_external_stack(locator.program_id())?.as_ref(), locator.resource())
                }
            }
            CallOperator::Resource(resource) => {
                // TODO (howardwu): Revisit this decision to forbid calling internal functions. A record cannot be spent again.
                //  But there are legitimate uses for passing a record through to an internal function.
                //  We could invoke the internal function without a state transition, but need to match visibility.
                if stack.program().contains_function(resource) {
                    bail!("Cannot call '{resource}'. Use a closure ('closure {resource}:') instead.")
                }

                (stack, resource)
            }
        };
        lap!(timer, "Retrieve the substack and resource");

        // If we are not handling the root request, retrieve the root request's tvk
        let root_tvk = registers.root_tvk().ok();

        // If the operator is a closure, retrieve the closure and compute the output.
        let outputs = if let Ok(closure) = substack.program().get_closure(resource) {
            lap!(timer, "Execute the closure");
            // Execute the closure, and load the outputs.
            substack.execute_closure(
                &closure,
                &inputs,
                registers.call_stack(),
                registers.signer_circuit()?,
                registers.caller_circuit()?,
                registers.tvk_circuit()?,
            )?
        }
        // If the operator is a function, retrieve the function and compute the output.
        else if let Ok(function) = substack.program().get_function(resource) {
            lap!(timer, "Execute the function");
            // Retrieve the number of inputs.
            let num_inputs = function.inputs().len();
            // Ensure the number of inputs matches the number of input statements.
            if num_inputs != inputs.len() {
                bail!("Expected {} inputs, found {}", num_inputs, inputs.len())
            }

            // Retrieve the number of public variables in the circuit.
            let num_public = A::num_public();

            // Indicate that external calls are never a root request.
            let is_root = false;

            use circuit::Eject;
            // Eject the existing circuit.
            let r1cs = A::eject_r1cs_and_reset();
            let (request, response) = {
                // Eject the circuit inputs.
                let inputs = inputs.eject_value();

                // Set the (console) caller.
                let console_caller = Some(*stack.program_id());

                match registers.call_stack() {
                    // If the circuit is in authorize or synthesize mode, then add any external calls to the stack.
                    CallStack::Authorize(_, private_key, authorization)
                    | CallStack::Synthesize(_, private_key, authorization) => {
                        // Compute the request.
                        let request = Request::sign(
                            &private_key,
                            *substack.program_id(),
                            *function.name(),
                            inputs.iter(),
                            &function.input_types(),
                            root_tvk,
                            is_root,
                            rng,
                        )?;

                        // Retrieve the call stack.
                        let mut call_stack = registers.call_stack();
                        // Push the request onto the call stack.
                        call_stack.push(request.clone())?;

                        // Add the request to the authorization.
                        authorization.push(request.clone());

                        // Execute the request.
                        let response = substack.execute_function::<A, R>(call_stack, console_caller, root_tvk, rng)?;

                        // Return the request and response.
                        (request, response)
                    }
                    CallStack::PackageRun(_, private_key, ..) => {
                        // Compute the request.
                        let request = Request::sign(
                            &private_key,
                            *substack.program_id(),
                            *function.name(),
                            inputs.iter(),
                            &function.input_types(),
                            root_tvk,
                            is_root,
                            rng,
                        )?;

                        // Retrieve the call stack.
                        let mut call_stack = registers.call_stack();
                        // Push the request onto the call stack.
                        call_stack.push(request.clone())?;

                        // Evaluate the request.
                        let response = substack.execute_function::<A, _>(call_stack, console_caller, root_tvk, rng)?;

                        // Return the request and response.
                        (request, response)
                    }
                    CallStack::CheckDeployment(_, private_key, ..) => {
                        // Compute the request.
                        let request = Request::sign(
                            &private_key,
                            *substack.program_id(),
                            *function.name(),
                            inputs.iter(),
                            &function.input_types(),
                            root_tvk,
                            is_root,
                            rng,
                        )?;

                        // Compute the address.
                        let address = Address::try_from(&private_key)?;
                        // Sample dummy outputs
                        let outputs = function
                            .outputs()
                            .iter()
                            .map(|output| match output.value_type() {
                                ValueType::Record(record_name) => {
                                    // Get the register index containing the record.
                                    let index = match output.operand() {
                                        Operand::Register(Register::Locator(index)) => Field::from_u64(*index),
                                        _ => bail!("Expected a `Register::Locator` operand for a record output."),
                                    };
                                    // Compute the encryption randomizer as `HashToScalar(tvk || index)`.
                                    let randomizer = N::hash_to_scalar_psd2(&[*request.tvk(), index])?;
                                    // Construct the record nonce.
                                    let record_nonce = N::g_scalar_multiply(&randomizer);
                                    Ok(Value::Record(substack.sample_record(
                                        &address,
                                        record_name,
                                        record_nonce,
                                        rng,
                                    )?))
                                }
                                _ => substack.sample_value(&address, output.value_type(), rng),
                            })
                            .collect::<Result<Vec<_>>>()?;
                        // Map the output operands to registers.
                        let output_registers = function
                            .outputs()
                            .iter()
                            .map(|output| match output.operand() {
                                Operand::Register(register) => Some(register.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>();

                        // Compute the response.
                        let response = crate::Response::new(
                            request.network_id(),
                            substack.program().id(),
                            function.name(),
                            request.inputs().len(),
                            request.tvk(),
                            request.tcm(),
                            outputs,
                            &function.output_types(),
                            &output_registers,
                        )?;

                        // Return the request and response.
                        (request, response)
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
                            bail!("Function '{}' outputs do not match in a 'call' instruction.", function.name())
                        }
                        // Return the request and response.
                        (request, response)
                    }
                }
            };
            lap!(timer, "Computed the request and response");

            // Inject the existing circuit.
            A::inject_r1cs(r1cs);

            use circuit::Inject;

            // Inject the network ID as `Mode::Constant`.
            let network_id = circuit::U16::constant(*request.network_id());
            // Inject the program ID as `Mode::Constant`.
            let program_id = circuit::ProgramID::constant(*substack.program_id());
            // Inject the function name as `Mode::Constant`.
            let function_name = circuit::Identifier::constant(*function.name());

            // Ensure the number of public variables remains the same.
            ensure!(A::num_public() == num_public, "Forbidden: 'call' injected excess public variables");

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

            // Ensure the candidate input IDs match their computed inputs.
            let (check_input_ids, _) = circuit::Request::check_input_ids::<false>(
                &network_id,
                &program_id,
                &function_name,
                &input_ids,
                &inputs,
                &function.input_types(),
                &signer,
                &sk_tag,
                &tvk,
                &tcm,
                None,
            );
            A::assert(check_input_ids);
            lap!(timer, "Checked the input ids");

            // Inject the outputs as `Mode::Private` (with the 'tcm' and output IDs as `Mode::Public`).
            let outputs = circuit::Response::process_outputs_from_callback(
                &network_id,
                &program_id,
                &function_name,
                num_inputs,
                &tvk,
                &tcm,
                response.outputs().to_vec(),
                &function.output_types(),
            );
            lap!(timer, "Checked the outputs");
            // Return the circuit outputs.
            outputs
        }
        // Else, throw an error.
        else {
            bail!("Call operator '{}' is invalid or unsupported.", self.operator())
        };

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
