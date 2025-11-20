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

impl<N: Network> CallTrait<N> for CallDynamic<N> {
    /// Evaluates the instruction.
    #[inline]
    fn evaluate<A: circuit::Aleo<Network = N>, R: CryptoRng + Rng>(
        &self,
        stack: &Stack<N>,
        registers: &mut Registers<N, A>,
        rng: &mut R,
    ) -> Result<()> {
        let timer = timer!("Call::evaluate");

        // Load the operands values.
        let inputs: Vec<_> = self.operands().iter().map(|operand| registers.load(stack, operand)).try_collect()?;

        // Get the program name.
        let Value::Plaintext(Plaintext::Literal(Literal::Field(program_name_as_field), _)) = &inputs[0] else {
            bail!("Expected the first operand of `call.dynamic` to be a 'Field' literal.")
        };
        let program_name = Identifier::from_field(program_name_as_field)?;

        // Get the program network.
        let Value::Plaintext(Plaintext::Literal(Literal::Field(program_network_id), _)) = &inputs[1] else {
            bail!("Expected the second operand of `call.dynamic` to be a 'Field' literal.")
        };
        let program_network = Identifier::from_field(program_network_id)?;

        // Construct the program ID.
        let program_id = ProgramID::try_from((program_name, program_network))?;

        // Get the function name.
        let Value::Plaintext(Plaintext::Literal(Literal::Field(function_name_as_field), _)) = &inputs[2] else {
            bail!("Expected the third operand of `call.dynamic` to be a 'Field' literal.")
        };
        let function_name = Identifier::from_field(function_name_as_field)?;

        // Separate the remaining inputs as the function inputs.
        let inputs = &inputs[3..];

        // Retrieve the optional external stack and resource.
        let external_stack = match stack.program().id() == &program_id {
            // Retrieve the call stack and resource from the locator.
            false => {
                // Check the external call locator.
                let is_credits_program = &program_id.to_string() == "credits.aleo";
                let is_fee_private = function_name.to_string() == "fee_private";
                let is_fee_public = &function_name.to_string() == "fee_public";

                // Ensure the external call is not to 'credits.aleo/fee_private' or 'credits.aleo/fee_public'.
                if is_credits_program && (is_fee_private || is_fee_public) {
                    bail!("Cannot perform an external call to 'credits.aleo/fee_private' or 'credits.aleo/fee_public'.")
                } else {
                    Some(stack.get_stack_unchecked(&program_id)?)
                }
            }
            true => {
                // TODO (howardwu): Revisit this decision to forbid calling internal functions. A record cannot be spent again.
                //  But there are legitimate uses for passing a record through to an internal function.
                //  We could invoke the internal function without a state transition, but need to match visibility.
                // TODO (@d0cd): Resolve recursion with records.
                if stack.program().contains_function(&function_name) {
                    bail!("Cannot dynamically evaluate a local '{function_name}' ")
                }
                None
            }
        };
        // Retrieve the substack.
        let substack = match &external_stack {
            Some(external_stack) => external_stack.as_ref(),
            None => stack,
        };
        lap!(timer, "Retrieved the substack");

        // If the operator is a closure, retrieve the closure and compute the output.
        let outputs = if substack.program().get_closure(&function_name).is_ok() {
            // A closure cannot be dynamically called.
            bail!("Cannot dynamically evaluate a closure: {function_name}")
        }
        // If the operator is a function, retrieve the function and compute the output.
        else if let Ok(function) = substack.program().get_function(&function_name) {
            // Ensure the number of inputs matches the number of input statements.
            if function.inputs().len() != inputs.len() {
                bail!("Expected {} inputs, found {}", function.inputs().len(), inputs.len())
            }

            // Get the 'root_tvk'.
            let root_tvk = Some(registers.root_tvk()?);

            // Get the call stack.
            let mut call_stack = registers.call_stack();

            // In Authorize mode, we need to compute the new request and push it onto the call stack.
            if let CallStack::Authorize(requests, private_key, authorization) = &mut call_stack {
                // Set 'is_root'.
                let is_root = false;
                // Ensure that we have a private key to sign the new request.
                let Some(private_key) = private_key else {
                    bail!("Cannot authorize a new function call without a private key.")
                };
                // Retrieve the program checksum, if the program has a constructor.
                let program_checksum = match substack.program().contains_constructor() {
                    true => Some(substack.program_checksum_as_field()?),
                    false => None,
                };

                // Get the input types of the callee.
                let input_types = substack.program().get_function_ref(&function_name)?.input_types();
                // Ensure that the number of inputs match.
                if input_types.len() != inputs.len() {
                    bail!("Expected {} inputs, found {}", input_types.len(), inputs.len())
                }

                // Convert the inputs to the callee's context.
                // TODO (@d0cd): Do we need to check that they match? I think no because `CallDynamic::output_types should have`
                let callee_inputs = inputs
                    .iter()
                    .zip_eq(input_types.iter())
                    .map(|(input, input_type)| match (input, input_type) {
                        (Value::Record(record), ValueType::DynamicRecord) => {
                            Ok(Value::DynamicRecord(DynamicRecord::from_record(&record)?))
                        }
                        (Value::Future(future), ValueType::DynamicFuture) => {
                            Ok(Value::DynamicFuture(DynamicFuture::from_future(future)?))
                        }
                        (Value::DynamicRecord(dynamic_record), ValueType::Record(record_name)) => {
                            // Look up the owner visibility.
                            let owner_is_private = substack.program().get_record(record_name)?.owner().is_private();
                            Ok(Value::Record(dynamic_record.to_record(owner_is_private)?))
                        }
                        (Value::DynamicFuture(dynamic_future), ValueType::Future(locator)) => {
                            // Construct the dynamic future.
                            let future = dynamic_future.to_future()?;
                            // Ensure that the locator matches.
                            ensure!(
                                future.program_id() == locator.program_id(),
                                "Locator program ID does not match for dynamic future."
                            );
                            ensure!(
                                future.function_name() == locator.resource(),
                                "Locator resource does not match for dynamic future."
                            );

                            Ok(Value::Future(dynamic_future.to_future()?))
                        }
                        // For other types, we assume they are directly compatible.
                        _ => Ok(input.clone()),
                    })
                    .collect::<Result<Vec<_>>>()?;

                // Compute the request.
                let request = Request::sign_dynamic(
                    private_key,
                    *substack.program_id(),
                    *function.name(),
                    callee_inputs.iter(),
                    &function.input_types(),
                    inputs.iter(),
                    self.operand_types(),
                    self.destination_types(),
                    registers.request()?,
                    root_tvk,
                    is_root,
                    program_checksum,
                    rng,
                )?;
                // Add the request to the requests.
                requests.push(request.clone());
                // Add the request to the authorization.
                authorization.push(request)?;
            };

            // Set the (console) caller.
            let console_caller = Some(*stack.program_id());
            // Evaluate the function.
            let response = substack.evaluate_function::<A, R>(call_stack, console_caller, root_tvk, rng)?;
            // Convert the callee's outputs to the caller's context.
            response.dynamic_call_outputs(self.destination_types())?
        }
        // Else, throw an error.
        else {
            bail!("Dynamic call to '{program_id}/{function_name}' is invalid or unsupported.")
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
    fn execute<A: circuit::Aleo<Network = N>, R: CryptoRng + Rng>(
        &self,
        stack: &Stack<N>,
        registers: &mut Registers<N, A>,
        rng: &mut R,
    ) -> Result<()> {
        use circuit::Eject;

        // TODO (dynamic_dispatch) remove
        let mode = match registers.call_stack_ref() {
            CallStack::Authorize(..) => "Authorize",
            CallStack::Execute(..) => "Execute",
            CallStack::CheckDeployment(..) => "CheckDeployment",
            CallStack::Synthesize(..) => "Synthesize",
            CallStack::Evaluate(..) => "Evaluate",
            CallStack::PackageRun(..) => "PackageRun",
        };
        println!("**ENTERING EXECUTE in mode: {:?}**", mode);

        let timer = timer!("Call::execute");

        // Load the operands values.
        let inputs: Vec<_> =
            self.operands().iter().map(|operand| registers.load_circuit(stack, operand)).try_collect()?;

        // Get the program name.
        let circuit::Value::Plaintext(circuit::Plaintext::Literal(circuit::Literal::Field(program_name_as_field), _)) =
            &inputs[0]
        else {
            bail!("Expected the first operand of `call.dynamic` to be a 'Field' literal.")
        };

        // Get the program network.
        let circuit::Value::Plaintext(circuit::Plaintext::Literal(
            circuit::Literal::Field(program_network_as_field),
            _,
        )) = &inputs[1]
        else {
            bail!("Expected the second operand of `call.dynamic` to be a 'Field' literal.")
        };

        // Get the function name.
        let circuit::Value::Plaintext(circuit::Plaintext::Literal(circuit::Literal::Field(function_name_as_field), _)) =
            &inputs[2]
        else {
            bail!("Expected the third operand of `call.dynamic` to be a 'Field' literal.")
        };

        // Separate the remaining inputs as the function inputs.
        let inputs = &inputs[3..];

        // If we are not handling the root request, retrieve the root request's tvk
        let root_tvk = registers.root_tvk().ok();

        // Execute the function.
        let outputs = {
            lap!(timer, "Execute the function");

            // Retrieve the number of public variables in the circuit.
            let num_public = A::num_public();

            // Indicate that external calls are never a root request.
            let is_root = false;

            // Eject the existing circuit.
            let r1cs = A::eject_r1cs_and_reset();
            let (request, caller_response_outputs, translation_data) = {
                // Resolve the program and function.
                let target = resolve_dynamic_target(
                    registers.call_stack_ref(),
                    stack,
                    &program_name_as_field.eject_value(),
                    &program_network_as_field.eject_value(),
                    &function_name_as_field.eject_value(),
                )?;

                // Eject the circuit inputs.
                let inputs = inputs.eject_value();

                // Set the (console) caller.
                let console_caller = Some(*stack.program_id());

                match registers.call_stack_ref() {
                    // If the circuit is in authorize mode, then add any external calls to the stack.
                    CallStack::Authorize(_, private_key, authorization) => {
                        // Get the target.
                        let Some(target) = target else {
                            bail!("Failed to resolve the target of the dynamic call in 'Authorize' mode.")
                        };
                        // Get the function.
                        let function = target.substack().program().get_function_ref(target.function_name())?;
                        // Retrieve the number of inputs.
                        let num_inputs = function.inputs().len();
                        // Ensure the number of inputs matches the number of input statements.
                        if num_inputs != inputs.len() {
                            bail!("Expected {} inputs, found {}", num_inputs, inputs.len())
                        }
                        // Ensure that we have a private key to sign the new request.
                        let Some(private_key) = private_key else {
                            bail!("Cannot authorize a new function call without a private key.")
                        };
                        // Retrieve the program checksum, if the program has a constructor.
                        let program_checksum = match target.substack().program().contains_constructor() {
                            true => Some(target.substack().program_checksum_as_field()?),
                            false => None,
                        };

                        // Get the input types of the callee.
                        let input_types =
                            &target.substack().program().get_function_ref(target.function_name())?.input_types();
                        // Ensure that the number of inputs match.
                        if input_types.len() != inputs.len() {
                            bail!("Expected {} inputs, found {}", input_types.len(), inputs.len())
                        }
                        // Convert the inputs to the callee's context.
                        // TODO (@d0cd): Do we need to check that they match? I think no because `CallDynamic::output_types should have`
                        let callee_inputs = inputs
                            .iter()
                            .zip_eq(input_types.iter())
                            .map(|(input, input_type)| match (input, input_type) {
                                (Value::Record(record), ValueType::DynamicRecord) => {
                                    Ok(Value::DynamicRecord(DynamicRecord::from_record(&record)?))
                                }
                                (Value::Future(future), ValueType::DynamicFuture) => {
                                    Ok(Value::DynamicFuture(DynamicFuture::from_future(future)?))
                                }
                                (Value::DynamicRecord(dynamic_record), ValueType::Record(record_name)) => {
                                    // Look up the owner visibility.
                                    let owner_is_private =
                                        target.substack().program().get_record(record_name)?.owner().is_private();
                                    Ok(Value::Record(dynamic_record.to_record(owner_is_private)?))
                                }
                                (Value::DynamicFuture(dynamic_future), ValueType::Future(locator)) => {
                                    // Construct the dynamic future.
                                    let future = dynamic_future.to_future()?;
                                    // Ensure that the locator matches.
                                    ensure!(
                                        future.program_id() == locator.program_id(),
                                        "Locator program ID does not match for dynamic future."
                                    );
                                    ensure!(
                                        future.function_name() == locator.resource(),
                                        "Locator resource does not match for dynamic future."
                                    );

                                    Ok(Value::Future(dynamic_future.to_future()?))
                                }
                                // For other types, we assume they are directly compatible.
                                _ => Ok(input.clone()),
                            })
                            .collect::<Result<Vec<_>>>()?;

                        // Construct the callee's version of the request.
                        let callee_request = Request::sign_dynamic(
                            private_key,
                            *target.substack().program_id(),
                            *function.name(),
                            callee_inputs.iter(),
                            input_types,
                            inputs.iter(),
                            self.operand_types(),
                            self.destination_types(),
                            registers.request()?,
                            root_tvk,
                            is_root,
                            program_checksum,
                            rng,
                        )?;

                        // Construct the request verification inputs.
                        let request_verification_inputs = RequestVerificationInputs::from(&callee_request)?;
                        // Retrieve the call stack.
                        let mut call_stack = registers.call_stack();
                        // Push the callee's request onto the call stack.
                        call_stack.push(callee_request.clone())?;

                        // Add the callee's request to the authorization.
                        authorization.push(callee_request.clone())?;

                        // Execute the callee's request.
                        let callee_response =
                            target.substack().execute_function::<A, R>(call_stack, console_caller, root_tvk, rng)?;

                        // Convert the callee's outputs to the caller's context.
                        let caller_response_outputs = callee_response.dynamic_call_outputs(callee_request.caller_output_types().as_ref().unwrap())?;

                        // Return the request verification inputs and response.
                        (request_verification_inputs, caller_response_outputs, None)
                    }
                    // TODO (@d0cd): Synthesize based on the declared outputs of the instruction.
                    // In Synthesize mode (with an existing proving key) or CheckDeployment mode, we generate dummy outputs to avoid building a full sub-circuit.
                    CallStack::Synthesize(_, private_key, ..) | CallStack::CheckDeployment(_, private_key, ..) => {
                        // Sample a random program ID.
                        // Note. It does not matter what program ID we use here, since we are only synthesizing dummy outputs.
                        let program_id = ProgramID::from_str("a.aleo")?;
                        // Sample a random function name.
                        // Note. It does not matter what function name we use here, since we are only synthesizing dummy outputs.
                        let function_name = Identifier::<N>::from_str("a")?;

                        // Compute the address.
                        let address = Address::try_from(private_key)?;

                        // Construct the request verification inputs.
                        let request_verification_inputs = RequestVerificationInputs {
                            network_id: U16::new(N::ID),
                            program_id,
                            function_name,
                            signer: Address::rand(rng),
                            sk_tag: Field::rand(rng),
                            tvk: Field::rand(rng),
                            tcm: Field::rand(rng),
                            caller_input_ids: self
                                .operand_types()
                                .iter()
                                .map(|type_| match type_ {
                                    ValueType::Constant(..) => Ok(InputID::Constant(Field::rand(rng))),
                                    ValueType::Public(..) => Ok(InputID::Public(Field::rand(rng))),
                                    ValueType::Private(..) => Ok(InputID::Private(Field::rand(rng))),
                                    ValueType::Record(..) => Ok(InputID::Record(
                                        Field::rand(rng),
                                        Group::rand(rng),
                                        Field::rand(rng),
                                        Field::rand(rng),
                                        Field::rand(rng),
                                    )),
                                    ValueType::ExternalRecord(..) => Ok(InputID::ExternalRecord(Field::rand(rng))),
                                    ValueType::Future(..) => bail!("A future cannot be input directly"),
                                    ValueType::DynamicRecord => Ok(InputID::DynamicRecord(Field::rand(rng))),
                                    ValueType::DynamicFuture => bail!("A dynamic future cannot be input directly"),
                                })
                                .collect::<Result<Vec<_>>>()?,
                        };

                        // Sample the outputs.
                        let callee_response_outputs = self
                            .destination_types()
                            .iter()
                            .map(|output_type| match output_type {
                                ValueType::Record(_) => bail!("A dynamic call cannot return a record."),
                                ValueType::ExternalRecord(_) => {
                                    bail!("A dynamic call cannot return an external record.")
                                }
                                ValueType::Future(_) => bail!("A dynamic call cannot return a future."),
                                // Sample the value.
                                _ => stack.sample_value(&address, &output_type.into(), rng),
                            })
                            .collect::<Result<Vec<_>>>()?;

                        // Return the request verification inputs and response.
                        (request_verification_inputs, callee_response_outputs, None)
                    }
                    // In PackageRun mode, we sign and execute the request once.
                    CallStack::PackageRun(_, private_key, ..) => {
                        // Get the target.
                        let Some(target) = target else {
                            bail!("Failed to resolve the target of the dynamic call in 'Authorize' mode.")
                        };
                        // Get the function.
                        let function = target.substack().program().get_function_ref(target.function_name())?;
                        // Retrieve the number of inputs.
                        let num_inputs = function.inputs().len();
                        // Ensure the number of inputs matches the number of input statements.
                        if num_inputs != inputs.len() {
                            bail!("Expected {} inputs, found {}", num_inputs, inputs.len())
                        }
                        // Retrieve the program checksum, if the program has a constructor.
                        let program_checksum = match target.substack().program().contains_constructor() {
                            true => Some(target.substack().program_checksum_as_field()?),
                            false => None,
                        };

                        // Get the input types of the callee.
                        let input_types =
                            &target.substack().program().get_function_ref(target.function_name())?.input_types();
                        // Ensure that the number of inputs match.
                        if input_types.len() != inputs.len() {
                            bail!("Expected {} inputs, found {}", input_types.len(), inputs.len())
                        }
                        // Convert the inputs to the callee's context.
                        // TODO (@d0cd): Do we need to check that they match? I think no because `CallDynamic::output_types should have`
                        let callee_inputs = inputs
                            .iter()
                            .zip_eq(input_types.iter())
                            .map(|(input, input_type)| match (input, input_type) {
                                (Value::Record(record), ValueType::DynamicRecord) => {
                                    Ok(Value::DynamicRecord(DynamicRecord::from_record(&record)?))
                                }
                                (Value::Future(future), ValueType::DynamicFuture) => {
                                    Ok(Value::DynamicFuture(DynamicFuture::from_future(future)?))
                                }
                                (Value::DynamicRecord(dynamic_record), ValueType::Record(record_name)) => {
                                    // Look up the owner visibility.
                                    let owner_is_private =
                                        target.substack().program().get_record(record_name)?.owner().is_private();
                                    Ok(Value::Record(dynamic_record.to_record(owner_is_private)?))
                                }
                                (Value::DynamicFuture(dynamic_future), ValueType::Future(locator)) => {
                                    // Construct the dynamic future.
                                    let future = dynamic_future.to_future()?;
                                    // Ensure that the locator matches.
                                    ensure!(
                                        future.program_id() == locator.program_id(),
                                        "Locator program ID does not match for dynamic future."
                                    );
                                    ensure!(
                                        future.function_name() == locator.resource(),
                                        "Locator resource does not match for dynamic future."
                                    );

                                    Ok(Value::Future(dynamic_future.to_future()?))
                                }
                                // For other types, we assume they are directly compatible.
                                _ => Ok(input.clone()),
                            })
                            .collect::<Result<Vec<_>>>()?;
                        // Construct the callee's version of the request.
                        let callee_request = Request::sign_dynamic(
                            private_key,
                            *target.substack().program_id(),
                            *function.name(),
                            callee_inputs.iter(),
                            input_types,
                            inputs.iter(),
                            self.operand_types(),
                            self.destination_types(),
                            registers.request()?,
                            root_tvk,
                            is_root,
                            program_checksum,
                            rng,
                        )?;

                        // Construct the request verification inputs.
                        let request_verification_inputs = RequestVerificationInputs::from(&callee_request)?;

                        // Retrieve the call stack.
                        let mut call_stack = registers.call_stack();
                        // Push the callee's request onto the call stack.
                        call_stack.push(callee_request.clone())?;

                        // Evaluate the callee's request.
                        let callee_response =
                            target.substack().execute_function::<A, _>(call_stack, console_caller, root_tvk, rng)?;

                        // Convert the callee's outputs to the caller's context.
                        let caller_response_outputs = callee_response.dynamic_call_outputs(callee_request.caller_output_types().as_ref().unwrap())?;

                        // Return the request verification inputs and response.
                        (request_verification_inputs, caller_response_outputs, None)
                    }
                    // If the circuit is in evaluate mode, then throw an error.
                    CallStack::Evaluate(..) => {
                        bail!("Cannot 'execute' a function in 'evaluate' mode.")
                    }
                    // If the circuit is in execute mode, then evaluate and execute the instructions.
                    CallStack::Execute(authorization, ..) => {
                        // Get the target.
                        let Some(target) = target else {
                            bail!("Failed to resolve the target of the dynamic call in 'Authorize' mode.")
                        };
                        // Get the function.
                        let callee_function = target.substack().program().get_function_ref(target.function_name())?;
                        // Retrieve the number of inputs.
                        let num_inputs = callee_function.inputs().len();
                        // Ensure the number of inputs matches the number of input statements.
                        if num_inputs != inputs.len() {
                            bail!("Expected {} inputs, found {}", num_inputs, inputs.len())
                        }

                        // Retrieve the callee's request (without popping it).
                        let callee_request = authorization.peek_next()?;

                        // Construct the request verification inputs.
                        let callee_request_verification_inputs = RequestVerificationInputs::from(&callee_request)?;

                        // Evaluate the function, and load the outputs.
                        let console_callee_response = target.substack().evaluate_function::<A, R>(
                            registers.call_stack(),
                            console_caller,
                            root_tvk,
                            rng,
                        )?;
                        // Execute the request.
                        let callee_response = target.substack().execute_function::<A, R>(
                            registers.call_stack(),
                            console_caller,
                            root_tvk,
                            rng,
                        )?;

                        // Ensure the values are equal.
                        if console_callee_response.outputs() != callee_response.outputs() {
                            dev_eprintln!(
                                "\n{:#?} != {:#?}\n",
                                console_callee_response.outputs(),
                                callee_response.outputs()
                            );
                            bail!("Function '{}' outputs do not match in a 'call' instruction.", callee_function.name())
                        }

                        // Convert the callee's outputs to the caller's context.
                        let caller_response_outputs = callee_response.dynamic_call_outputs(callee_request.caller_output_types().as_ref().unwrap())?;

                        // Anonymous helper to get a record translation proving key.
                        let get_record_translation_proving_key =
                            |program_id: &ProgramID<N>, record_name: &Identifier<N>| -> Result<ProvingKey<N>> {
                                let record_stack = match program_id == stack.program_id() {
                                    true => stack,
                                    false => &stack.get_stack_unchecked(&program_id)?,
                                };
                                record_stack.get_translation_proving_key(record_name)
                            };
                        let caller_console_input_ids = callee_request.caller_input_ids().clone().unwrap_or_default();
                        let callee_console_input_ids = callee_request.input_ids();
                        let caller_console_request = registers.request()?;
                        let caller_console_function_id = compute_function_id(&caller_console_request.network_id(), caller_console_request.program_id(), caller_console_request.function_name(), caller_console_request.is_dynamic())?;
                        let callee_console_function_id = compute_function_id(
                            &U16::<N>::new(N::ID as u16),
                            callee_request.program_id(),
                            callee_request.function_name(),
                            false,
                        )?;
                        let caller_input_types = self.operand_types().into_iter().skip(3).collect::<Vec<_>>();
                        let callee_input_types = callee_function.input_types();
                        let caller_console_inputs = inputs;
                        let callee_console_inputs = callee_request.inputs();
                        let mut translation_data = Vec::new();

                        // TODO: ensure all of the iterators are the same length.
                        for (
                            operand_index,
                            (
                                caller_input_value,
                                caller_input_id,
                                caller_input_type,
                                callee_input_value,
                                callee_input_id,
                                callee_input_type,
                            ),
                        ) in itertools::izip!(
                            caller_console_inputs,
                            caller_console_input_ids,
                            caller_input_types,
                            callee_console_inputs,
                            callee_console_input_ids,
                            callee_input_types
                        )
                        .enumerate()
                        {
                            match (
                                caller_input_value,
                                caller_input_id,
                                caller_input_type,
                                callee_input_value,
                                callee_input_id,
                                callee_input_type,
                            ) {
                                (
                                    Value::Record(record),
                                    InputID::Record(_record_commitment, gamma, record_view_key, serial_number, _tag),
                                    ValueType::Record(record_name),
                                    Value::DynamicRecord(dynamic_record),
                                    InputID::DynamicRecord(dynamic_record_commitment),
                                    ValueType::DynamicRecord,
                                ) => {
                                    let program_id = *stack.program_id();
                                    let translation_proving_key =
                                        get_record_translation_proving_key(&program_id, &record_name)?;
                                    translation_data.push(RecordTranslationData {
                                        // TODO: consider using a mapping from (program_id, record_name) to (proving_key, other data)
                                        translation_proving_key, // caller record proving key
                                        record_static: record.clone(), // caller static_record
                                        record_dynamic: dynamic_record.clone(), // callee dynamic_record
                                        program_id,              // callee program_id
                                        function_id: callee_console_function_id, // callee function_id
                                        record_name: *record_name, // caller record_name
                                        record_consumed: true,   // misnomer, but yes it's the input direction
                                        tvk: *callee_request.tvk(), // callee tvk
                                        record_view_key: Some(record_view_key), // caller record_view_key
                                        gamma: Some(gamma.clone()), // caller gamma
                                        static_record_id: serial_number, // caller static_record_id
                                        dynamic_record_id: *dynamic_record_commitment, // callee dynamic_record_id
                                        input_output_index: operand_index as u16, // operand_index
                                    });
                                }
                                (
                                    Value::DynamicRecord(dynamic_record),
                                    InputID::DynamicRecord(dynamic_record_commitment),
                                    ValueType::DynamicRecord,
                                    Value::Record(record),
                                    InputID::Record(_record_commitment, gamma, record_view_key, serial_number, _tag),
                                    ValueType::Record(record_name),
                                ) => {
                                    let program_id = *callee_request.program_id();
                                    let translation_proving_key =
                                        get_record_translation_proving_key(&program_id, &record_name)?;
                                    translation_data.push(RecordTranslationData {
                                        // TODO: consider using a mapping from (program_id, record_name) to (proving_key, other data)
                                        translation_proving_key, // callee record proving key
                                        record_static: record.clone(), // callee static_record
                                        record_dynamic: dynamic_record.clone(), // caller dynamic_record
                                        program_id,              // callee program_id
                                        function_id: caller_console_function_id, // caller function_id
                                        record_name,             // callee record_name
                                        record_consumed: true,   // misnomer, but yes it's the input direction
                                        tvk: registers.tvk()?,   // caller tvk
                                        record_view_key: Some(*record_view_key), // callee record_view_key
                                        gamma: Some(*gamma),     // callee gamma
                                        static_record_id: *serial_number, // callee static_record_id
                                        dynamic_record_id: dynamic_record_commitment, // caller dynamic_record_id
                                        input_output_index: operand_index as u16, // callee operand_index
                                    });
                                }
                                _ => {} // No translation to perform.
                            }
                        }
                        // Collect record outputs to translate.
                        let caller_console_outputs = caller_response_outputs.clone();
                        let caller_console_output_ids: Vec<OutputID<N>> = vec![];  // TODO: we may need to add this to the Request or Response object.
                        let caller_output_types = self.destination_types();
                        let callee_console_outputs = console_callee_response.outputs();
                        let callee_console_output_ids = console_callee_response.output_ids();
                        let callee_output_types = callee_function.output_types();
                        for (
                            operand_index,
                            (
                                caller_output_value,
                                caller_output_id,
                                caller_output_type,
                                callee_output_value,
                                callee_output_id,
                                callee_output_type,
                            ),
                        ) in itertools::izip!(
                            caller_console_outputs,
                            caller_console_output_ids,
                            caller_output_types,
                            callee_console_outputs,
                            callee_console_output_ids,
                            callee_output_types
                        )
                        .enumerate()
                        {
                            match (
                                caller_output_value,
                                caller_output_id,
                                caller_output_type,
                                callee_output_value,
                                callee_output_id,
                                callee_output_type,
                            ) {
                                (
                                    Value::Record(record),
                                    OutputID::Record(record_commitment, _checksum, _sender_ciphertext),
                                    ValueType::Record(record_name),
                                    Value::DynamicRecord(dynamic_record),
                                    OutputID::DynamicRecord(dynamic_record_commitment),
                                    ValueType::DynamicRecord,
                                ) => {
                                    // let program_id = *caller_request.program_id();
                                    // let translation_proving_key = get_record_translation_proving_key(program_id, &record_name)?;
                                    // translation_data.push(RecordTranslationData {
                                    //     // TODO: consider using a mapping from (program_id, record_name) to (proving_key, other data)
                                    //     translation_proving_key,                 // caller record proving key
                                    //     record_static: record.clone(),           // caller static_record
                                    //     record_dynamic: dynamic_record.clone(),  // callee dynamic_record
                                    //     program_id,                              // callee program_id
                                    //     function_id: callee_console_function_id, // callee function_id
                                    //     record_name,                             // caller record_name
                                    //     to_static_record: true,                  // misnomer, but yes it's the input direction
                                    //     tvk: callee_request.tvk(),               // callee tvk
                                    //     record_view_key: None,
                                    //     gamma: None,
                                    //     static_record_id: *record_commitment,            // caller static_record_id
                                    //     dynamic_record_id: *dynamic_record_commitment,   // callee dynamic_record_id
                                    //     operand_index: operand_index as u16,             // operand_index
                                    // });
                                }
                                (
                                    Value::DynamicRecord(dynamic_record),
                                    OutputID::DynamicRecord(dynamic_record_commitment),
                                    ValueType::DynamicRecord,
                                    Value::Record(record),
                                    OutputID::Record(record_commitment, _checksum, _sender_ciphertext),
                                    ValueType::Record(record_name),
                                ) => {
                                    // let program_id = *callee_request.program_id();
                                    // let translation_proving_key = get_record_translation_proving_key(&program_id, &record_name)?;
                                    // translation_data.push(RecordTranslationData {
                                    //     // TODO: consider using a mapping from (program_id, record_name) to (proving_key, other data)
                                    //     translation_proving_key,                 // callee record proving key
                                    //     record_static: record.clone(),           // callee static_record
                                    //     record_dynamic: dynamic_record.clone(),  // caller dynamic_record
                                    //     program_id,                              // callee program_id
                                    //     function_id: caller_console_function_id, // caller function_id
                                    //     record_name,                             // callee record_name
                                    //     to_static_record: true,                  // misnomer, but yes it's the input direction
                                    //     tvk: registers.tvk()?,                   // caller tvk
                                    //     record_view_key: None,
                                    //     gamma: None,
                                    //     static_record_id: *record_commitment,            // callee static_record_id
                                    //     dynamic_record_id: *dynamic_record_commitment,   // caller dynamic_record_id
                                    //     operand_index: operand_index as u16,             // callee operand_index
                                    // });
                                }
                                _ => {} // No translation to perform.
                            }
                        }

                        // Return the caller's request and response.
                        (callee_request_verification_inputs, caller_response_outputs, Some(translation_data))
                    }
                }
            };
            lap!(timer, "Computed the request and response");

            // TODO(dynamic_dispatch): If we let Registers keep e.g. an Arc<Stack>, we can just access Registers above.
            if let Some(translation_data) = translation_data {
                for translation_datum in translation_data {
                    registers.insert_record_translation_data(translation_datum);
                }
            }

            // Inject the existing circuit.
            A::inject_r1cs(r1cs);

            use circuit::Inject;

            // Inject the network ID as `Mode::Constant`.
            let network_id = circuit::U16::constant(*request.network_id());
            // Inject the program ID name as `Mode::Public`.
            let program_id = circuit::ProgramID::public(*request.program_id());
            // Inject the function name as `Mode::Public`.
            let function_name = circuit::Identifier::public(*request.function_name());

            // Ensure that the program and function names in the registers match the witnessed values.
            A::assert_eq(program_id.name(), program_name_as_field);
            A::assert_eq(program_id.network(), program_network_as_field);
            A::assert_eq(&function_name, function_name_as_field);

            // Ensure the number of public variables remains the same.
            ensure!(A::num_public() == num_public + 3, "Forbidden: 'call.dynamic' injected excess public variables");

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

            // Inject the caller input IDs  as `Mode::Public`.
            let input_ids = request
                .caller_input_ids
                .iter()
                .map(|input_id| circuit::InputID::new(circuit::Mode::Public, *input_id))
                .collect::<Vec<_>>();

            // Ensure the candidate input IDs match their computed inputs.
            let (check_input_ids, _) = circuit::Request::check_input_ids::<false>(
                &network_id,
                &program_id,
                &function_name,
                &input_ids,
                inputs,
                self.operand_types(),
                &signer,
                &sk_tag,
                &tvk,
                &tcm,
                None,
                true,
            );
            A::assert(check_input_ids);
            lap!(timer, "Checked the input ids");

            // Checking that none of the outputs in the caller's context are records or futures.
            for output in caller_response_outputs.iter() {
                match output {
                    Value::Record(_) => bail!("A dynamic call cannot return a record."),
                    Value::Future(_) => bail!("A dynamic call cannot return a future."),
                    Value::Plaintext(_) | Value::DynamicRecord(_) | Value::DynamicFuture(_) => {} // Do nothing.
                }
            }

            // Use `None` for the output registers. This is safe since an output of a dynamic call cannot be a record.
            let output_registers = vec![None; caller_response_outputs.len()];

            // Inject the outputs as `Mode::Private` (with the 'tcm' and output IDs as `Mode::Public`).
            let outputs = circuit::Response::process_outputs_from_callback(
                &network_id,
                &program_id,
                &function_name,
                inputs.len(),
                &tvk,
                &tcm,
                caller_response_outputs,
                self.destination_types(),
                &output_registers,
                true,
            );
            lap!(timer, "Checked the outputs");

            // TODO (dynamic_dispatch) remove
            println!(
                "**INSIDE EXECUTE FOR FUNCTION {:?} PROCESS OUTPUTS FROM CALLBACK in mode: {:?}**",
                function_name, mode
            );

            // Return the circuit outputs.
            outputs
        };

        // Assign the outputs to the destination registers.
        for (output, register) in outputs.into_iter().zip_eq(&self.destinations()) {
            // Assign the output to the register.
            registers.store_circuit(stack, register, output)?;
        }
        lap!(timer, "Assigned the outputs to registers");

        finish!(timer);

        // TODO (dynamic_dispatch) remove
        println!("**COMPLETED CALL DYNAMIC in mode: {:?}**", mode);

        Ok(())
    }
}

// Information needed to verify the callee's request for a dynamic call.
struct RequestVerificationInputs<N: Network> {
    // The network ID.
    pub network_id: U16<N>,
    // The program ID.
    pub program_id: ProgramID<N>,
    // The function name.
    pub function_name: Identifier<N>,
    // The signer.
    pub signer: Address<N>,
    // The sk_tag.
    pub sk_tag: Field<N>,
    // The tvk.
    pub tvk: Field<N>,
    // The tcm.
    pub tcm: Field<N>,
    // The caller input IDs.
    pub caller_input_ids: Vec<InputID<N>>,
}

impl<N: Network> RequestVerificationInputs<N> {
    /// Constructs the request verification inputs from a request and caller input IDs.
    #[inline]
    pub fn from(request: &Request<N>) -> Result<Self> {
        // Ensure the the caller input IDs are present.
        let Some(caller_input_ids) = &request.caller_input_ids() else {
            bail!("Missing caller input IDs for request verification inputs.")
        };
        Ok(Self {
            network_id: *request.network_id(),
            program_id: *request.program_id(),
            function_name: *request.function_name(),
            signer: *request.signer(),
            sk_tag: *request.sk_tag(),
            tvk: *request.tvk(),
            tcm: *request.tcm(),
            caller_input_ids: caller_input_ids.clone(),
        })
    }
}

impl<N: Network> RequestVerificationInputs<N> {
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

    /// Returns the tag secret key `sk_tag`.
    pub const fn sk_tag(&self) -> &Field<N> {
        &self.sk_tag
    }

    /// Returns the transition view key `tvk`.
    pub const fn tvk(&self) -> &Field<N> {
        &self.tvk
    }

    /// Returns the transition commitment `tcm`.
    pub const fn tcm(&self) -> &Field<N> {
        &self.tcm
    }

    /// Returns the caller input IDs.
    pub fn caller_input_ids(&self) -> &[InputID<N>] {
        &self.caller_input_ids
    }
}

// A reference to a stack, either local or external.
enum StackRef<'a, N: Network> {
    Local(&'a Stack<N>),
    External(Arc<Stack<N>>),
}

impl<'a, N: Network> Deref for StackRef<'a, N> {
    type Target = Stack<N>;

    fn deref(&self) -> &Self::Target {
        match self {
            StackRef::Local(stack) => stack,
            StackRef::External(stack) => stack.as_ref(),
        }
    }
}

// A resolved target of a dynamic call.
struct ResolvedTarget<'a, N: Network> {
    // The program ID.
    program_id: ProgramID<N>,
    // The function name.
    function_name: Identifier<N>,
    // The stack.
    substack: StackRef<'a, N>,
}

impl<'a, N: Network> ResolvedTarget<'a, N> {
    /// Returns the program ID.
    #[inline]
    pub fn program_id(&self) -> &ProgramID<N> {
        &self.program_id
    }

    /// Returns the function name.
    #[inline]
    pub fn function_name(&self) -> &Identifier<N> {
        &self.function_name
    }

    /// Returns the stack.
    #[inline]
    pub fn substack(&self) -> &Stack<N> {
        &self.substack
    }
}

// A helper function that attempts to resolve the target of a dynamic call.
// This function returns:
// - Some(ResolvedTarget) if the target is successfully resolved.
// - Ok(None) in `Synthesize` or `CheckDeployment` mode when the target cannot be resolved.
// - Err(_) in other modes when the target cannot be resolved.
fn resolve_dynamic_target<'a, N: Network>(
    call_stack: &'a CallStack<N>,
    stack: &'a Stack<N>,
    program_name_as_field: &Field<N>,
    program_network_as_field: &Field<N>,
    function_name_as_field: &Field<N>,
) -> Result<Option<ResolvedTarget<'a, N>>> {
    // Determine whether we are in "dummy" (`Synthesize` or `CheckDeployment`) mode.
    let in_dummy_mode = match call_stack {
        CallStack::Synthesize(..) | CallStack::CheckDeployment(..) => true,
        _ => false,
    };

    // Decode the program name, exiting gracefully in dummy mode if it fails.
    let program_name = match Identifier::from_field(program_name_as_field) {
        Ok(id) => id,
        Err(_) if in_dummy_mode => {
            return Ok(None);
        }
        Err(e) => bail!("Failed to decode the program name in a dynamic call: {e}"),
    };

    // Decode the program network, exiting gracefully in dummy mode if it fails.
    let program_network = match Identifier::from_field(program_network_as_field) {
        Ok(id) => id,
        Err(_) if in_dummy_mode => {
            return Ok(None);
        }
        Err(e) => bail!("Failed to decode the program network in a dynamic call: {e}"),
    };

    // Decode the function name, exiting gracefully in dummy mode if it fails.
    let function_name = match Identifier::from_field(function_name_as_field) {
        Ok(id) => id,
        Err(_) if in_dummy_mode => {
            return Ok(None);
        }
        Err(e) => bail!("Failed to decode the function name in a dynamic call: {e}"),
    };

    // Construct the program ID.
    let program_id = match ProgramID::try_from((program_name, program_network)) {
        Ok(id) => id,
        Err(_) if in_dummy_mode => {
            return Ok(None);
        }
        Err(e) => bail!("Failed to construct the program ID in a dynamic call: {e}"),
    };

    // Verify that the call is not to 'credits.aleo/fee_private' or 'credits.aleo/fee_public'.
    let is_credits_program = &program_id.to_string() == "credits.aleo";
    let is_fee_private = function_name.to_string() == "fee_private";
    let is_fee_public = &function_name.to_string() == "fee_public";
    if is_credits_program && (is_fee_private || is_fee_public) {
        bail!("Cannot perform an external call to 'credits.aleo/fee_private' or 'credits.aleo/fee_public'.")
    }

    // Retrieve the optional external stack.
    let external_stack = match stack.program().id() == &program_id {
        false => match stack.get_stack_unchecked(&program_id) {
            Ok(ext_stack) => Some(ext_stack),
            Err(_) if in_dummy_mode => {
                return Ok(None);
            }
            Err(e) => bail!("Failed to retrieve the external stack in a dynamic call: {e}"),
        },
        true => None,
    };

    // Retrieve the substack.
    let substack = match &external_stack {
        Some(external_stack) => StackRef::External(external_stack.clone()),
        None => StackRef::Local(stack),
    };

    // Verify that the function is not a closure.
    if substack.program().get_closure(&function_name).is_ok() {
        bail!("Cannot dynamically evaluate a closure: {function_name}")
    } else if substack.program().contains_function(&function_name) {
        Ok(Some(ResolvedTarget { program_id, function_name, substack }))
    } else if in_dummy_mode {
        Ok(None)
    } else {
        bail!("Dynamic call to '{program_id}/{function_name}' is invalid or unsupported.")
    }
}
