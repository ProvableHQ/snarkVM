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

impl<N: Network> StackEvaluate<N> for Stack<N> {
    /// Evaluates a program closure on the given inputs.
    ///
    /// # Errors
    /// This method will halt if the given inputs are not the same length as the input statements.
    #[inline]
    fn evaluate_closure<A: circuit::Aleo<Network = N>>(
        &self,
        closure: &Closure<N>,
        inputs: &[Value<N>],
        call_stack: CallStack<N>,
        signer: Address<N>,
        caller: Address<N>,
        tvk: Field<N>,
    ) -> Result<Vec<Value<N>>> {
        let timer = timer!("Stack::evaluate_closure");

        // Ensure the number of inputs matches the number of input statements.
        if closure.inputs().len() != inputs.len() {
            bail!("Expected {} inputs, found {}", closure.inputs().len(), inputs.len())
        }

        // Initialize the registers.
        let mut registers = Registers::<N, A>::new(call_stack, self.get_register_types(closure.name())?.clone());
        // Set the transition signer.
        registers.set_signer(signer);
        // Set the transition caller.
        registers.set_caller(caller);
        // Set the transition view key.
        registers.set_tvk(tvk);
        lap!(timer, "Initialize the registers");

        // Store the inputs.
        closure.inputs().iter().map(|i| i.register()).zip_eq(inputs).try_for_each(|(register, input)| {
            // Assign the input value to the register.
            registers.store(self, register, input.clone())
        })?;
        lap!(timer, "Store the inputs");

        // Evaluate the instructions.
        for instruction in closure.instructions() {
            // If the evaluation fails, bail and return the error.
            if let Err(error) = instruction.evaluate(self, &mut registers) {
                bail!("Failed to evaluate instruction ({instruction}): {error}");
            }
        }
        lap!(timer, "Evaluate the instructions");

        // Load the outputs.
        let outputs = closure
            .outputs()
            .iter()
            .map(|output| {
                match output.operand() {
                    // If the operand is a literal, use the literal directly.
                    Operand::Literal(literal) => Ok(Value::Plaintext(Plaintext::from(literal))),
                    // If the operand is a register, retrieve the stack value from the register.
                    Operand::Register(register) => registers.load(self, &Operand::Register(register.clone())),
                    // If the operand is the program ID, convert the program ID into an address.
                    Operand::ProgramID(program_id) => {
                        Ok(Value::Plaintext(Plaintext::from(Literal::Address(program_id.to_address()?))))
                    }
                    // If the operand is the signer, retrieve the signer from the registers.
                    Operand::Signer => Ok(Value::Plaintext(Plaintext::from(Literal::Address(registers.signer()?)))),
                    // If the operand is the caller, retrieve the caller from the registers.
                    Operand::Caller => Ok(Value::Plaintext(Plaintext::from(Literal::Address(registers.caller()?)))),
                    // If the operand is the block height, throw an error.
                    Operand::BlockHeight => bail!("Cannot retrieve the block height from a closure scope."),
                    // If the operand is the network id, throw an error.
                    Operand::NetworkID => bail!("Cannot retrieve the network ID from a closure scope."),
                }
            })
            .collect();
        lap!(timer, "Load the outputs");

        finish!(timer);
        outputs
    }

    /// Evaluates a program function on the given inputs.
    ///
    /// # Errors
    /// This method will halt if the given inputs are not the same length as the input statements.
    #[inline]
    fn evaluate_function<A: circuit::Aleo<Network = N>>(
        &self,
        call_stack: CallStack<N>,
        caller: Option<ProgramID<N>>,
    ) -> Result<Response<N>> {
        let timer = timer!("Stack::evaluate_function");

        // Retrieve the next request, based on the call stack mode.
        let (request, call_stack) = match &call_stack {
            CallStack::Evaluate(authorization) => (authorization.next()?, call_stack),
            // If the evaluation is performed in the `Execute` mode, create a new `Evaluate` mode.
            // This is done to ensure that evaluation during execution is performed consistently.
            CallStack::Execute(authorization, _) => {
                // Note: We need to replicate the authorization, so that 'execute' can call 'authorization.next()?'.
                // This way, the authorization remains unmodified in this 'evaluate' scope.
                let authorization = authorization.replicate();
                let request = authorization.next()?;
                let call_stack = CallStack::Evaluate(authorization);
                (request, call_stack)
            }
            _ => bail!("Illegal operation: call stack must be `Evaluate` or `Execute` in `evaluate_function`."),
        };
        lap!(timer, "Retrieve the next request");

        // Ensure the network ID matches.
        ensure!(
            **request.network_id() == N::ID,
            "Network ID mismatch. Expected {}, but found {}",
            N::ID,
            request.network_id()
        );

        // Retrieve the function, inputs, and transition view key.
        let function = self.get_function(request.function_name())?;
        let inputs = request.inputs();
        let signer = *request.signer();
        let (is_root, caller) = match caller {
            // If a caller is provided, then this is an evaluation of a child function.
            Some(caller) => (false, caller.to_address()?),
            // If no caller is provided, then this is an evaluation of a top-level function.
            None => (true, signer),
        };
        let tvk = *request.tvk();

        // Ensure the number of inputs matches.
        if function.inputs().len() != inputs.len() {
            bail!(
                "Function '{}' in the program '{}' expects {} inputs, but {} were provided.",
                function.name(),
                self.program.id(),
                function.inputs().len(),
                inputs.len()
            )
        }
        lap!(timer, "Perform input checks");

        // Initialize the registers.
        let mut registers = Registers::<N, A>::new(call_stack, self.get_register_types(function.name())?.clone());
        // Set the transition signer.
        registers.set_signer(signer);
        // Set the transition caller.
        registers.set_caller(caller);
        // Set the transition view key.
        registers.set_tvk(tvk);
        lap!(timer, "Initialize the registers");

        // Ensure the request is well-formed.
        ensure!(request.verify(&function.input_types(), is_root), "Request is invalid");
        lap!(timer, "Verify the request");

        // Store the inputs.
        function.inputs().iter().map(|i| i.register()).zip_eq(inputs).try_for_each(|(register, input)| {
            // Assign the input value to the register.
            registers.store(self, register, input.clone())
        })?;
        lap!(timer, "Store the inputs");

        // Evaluate the instructions.
        // Note: We handle the `call` instruction separately, as it requires special handling.
        for instruction in function.instructions() {
            // Evaluate the instruction.
            let result = match instruction {
                // If the instruction is a `call` instruction, we need to handle it separately.
                Instruction::Call(call) => CallTrait::evaluate(call, self, &mut registers),
                // Otherwise, evaluate the instruction normally.
                _ => instruction.evaluate(self, &mut registers),
            };
            // If the evaluation fails, bail and return the error.
            if let Err(error) = result {
                bail!("Failed to evaluate instruction ({instruction}): {error}");
            }
        }
        lap!(timer, "Evaluate the instructions");

        // Retrieve the output operands.
        let output_operands = &function.outputs().iter().map(|output| output.operand()).collect::<Vec<_>>();
        lap!(timer, "Retrieve the output operands");

        // Load the outputs.
        let outputs = output_operands
            .iter()
            .map(|operand| {
                match operand {
                    // If the operand is a literal, use the literal directly.
                    Operand::Literal(literal) => Ok(Value::Plaintext(Plaintext::from(literal))),
                    // If the operand is a register, retrieve the stack value from the register.
                    Operand::Register(register) => registers.load(self, &Operand::Register(register.clone())),
                    // If the operand is the program ID, convert the program ID into an address.
                    Operand::ProgramID(program_id) => {
                        Ok(Value::Plaintext(Plaintext::from(Literal::Address(program_id.to_address()?))))
                    }
                    // If the operand is the signer, retrieve the signer from the registers.
                    Operand::Signer => Ok(Value::Plaintext(Plaintext::from(Literal::Address(registers.signer()?)))),
                    // If the operand is the caller, retrieve the caller from the registers.
                    Operand::Caller => Ok(Value::Plaintext(Plaintext::from(Literal::Address(registers.caller()?)))),
                    // If the operand is the block height, throw an error.
                    Operand::BlockHeight => bail!("Cannot retrieve the block height from a function scope."),
                    // If the operand is the network id, throw an error.
                    Operand::NetworkID => bail!("Cannot retrieve the network ID from a function scope."),
                }
            })
            .collect::<Result<Vec<_>>>()?;
        lap!(timer, "Load the outputs");

        // Map the output operands to registers.
        let output_registers = output_operands
            .iter()
            .map(|operand| match operand {
                Operand::Register(register) => Some(register.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        lap!(timer, "Loaded the output registers");

        // Compute the response.
        let response = Response::new(
            request.network_id(),
            self.program.id(),
            function.name(),
            request.inputs().len(),
            request.tvk(),
            request.tcm(),
            outputs,
            &function.output_types(),
            &output_registers,
        );
        finish!(timer);

        response
    }
}
