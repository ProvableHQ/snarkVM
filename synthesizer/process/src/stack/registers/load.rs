// Copyright (C) 2019-2023 Aleo Systems Inc.
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

impl<N: Network, A: circuit::Aleo<Network = N>> RegistersLoad<N> for Registers<N, A> {
    /// Loads the value of a given operand from the registers.
    ///
    /// # Errors
    /// This method will halt if the register locator is not found.
    /// In the case of register accesses, this method will halt if the access is not found.
    #[inline]
    fn load(&self, stack: &(impl StackMatches<N> + StackProgram<N>), operand: &Operand<N>) -> Result<Value<N>> {
        // Retrieve the register.
        let register = match operand {
            // If the operand is a literal, return the literal.
            Operand::Literal(literal) => return Ok(Value::Plaintext(Plaintext::from(literal))),
            // If the operand is a register, load the value from the register.
            Operand::Register(register) => register,
            // If the operand is the program ID, load the program address.
            Operand::ProgramID(program_id) => {
                return Ok(Value::Plaintext(Plaintext::from(Literal::Address(program_id.to_address()?))));
            }
            // If the operand is the signer, load the value of the signer.
            Operand::Signer => return Ok(Value::Plaintext(Plaintext::from(Literal::Address(self.signer()?)))),
            // If the operand is the caller, load the value of the caller.
            Operand::Caller => return Ok(Value::Plaintext(Plaintext::from(Literal::Address(self.caller()?)))),
            // If the operand is the block height, throw an error.
            Operand::BlockHeight => bail!("Cannot load the block height in a non-finalize context"),
            // If the operand is the network ID, throw an error.
            Operand::NetworkID => bail!("Cannot load the network ID in a non-finalize context"),
        };

        // Retrieve the stack value.
        let stack_value =
            self.console_registers.get(&register.locator()).ok_or_else(|| anyhow!("'{register}' does not exist"))?;

        // Return the value for the given register or register access.
        let stack_value = match register {
            // If the register is a locator, then return the stack value.
            Register::Locator(..) => stack_value.clone(),
            // If the register is a register access, then load the specific stack value.
            Register::Access(_, ref path) => {
                match stack_value {
                    // Retrieve the plaintext member from the path.
                    Value::Plaintext(plaintext) => Value::Plaintext(plaintext.find(path)?),
                    // Retrieve the record entry from the path.
                    Value::Record(record) => match record.find(path)? {
                        Entry::Constant(plaintext) | Entry::Public(plaintext) | Entry::Private(plaintext) => {
                            Value::Plaintext(plaintext)
                        }
                    },
                    // Retrieve the argument from the future.
                    Value::Future(future) => future.find(path)?,
                }
            }
        };

        // Retrieve the register type.
        match self.register_types.get_type(stack, register) {
            // Ensure the stack value matches the register type.
            Ok(register_type) => stack.matches_register_type(&stack_value, &register_type)?,
            // Ensure the register is defined.
            Err(error) => bail!("Register '{register}' is not a member of the function: {error}"),
        };

        Ok(stack_value)
    }
}

impl<N: Network, A: circuit::Aleo<Network = N>> RegistersLoadCircuit<N, A> for Registers<N, A> {
    /// Loads the value of a given operand from the registers.
    ///
    /// # Errors
    /// This method will halt if the register locator is not found.
    /// In the case of register accesses, this method will halt if the access is not found.
    #[inline]
    fn load_circuit(
        &self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        operand: &Operand<N>,
    ) -> Result<circuit::Value<A>> {
        use circuit::Inject;

        // Retrieve the register.
        let register = match operand {
            // If the operand is a literal, return the literal.
            Operand::Literal(literal) => {
                return Ok(circuit::Value::Plaintext(circuit::Plaintext::from(circuit::Literal::constant(
                    literal.clone(),
                ))));
            }
            // If the operand is a register, load the value from the register.
            Operand::Register(register) => register,
            // If the operand is the program ID, load the program address.
            Operand::ProgramID(program_id) => {
                return Ok(circuit::Value::Plaintext(circuit::Plaintext::from(circuit::Literal::constant(
                    Literal::Address(program_id.to_address()?),
                ))));
            }
            // If the operand is the signer, load the value of the signer.
            Operand::Signer => {
                return Ok(circuit::Value::Plaintext(circuit::Plaintext::from(circuit::Literal::Address(
                    self.signer_circuit()?,
                ))));
            }
            // If the operand is the caller, load the value of the caller.
            Operand::Caller => {
                return Ok(circuit::Value::Plaintext(circuit::Plaintext::from(circuit::Literal::Address(
                    self.caller_circuit()?,
                ))));
            }
            // If the operand is the block height, throw an error.
            Operand::BlockHeight => bail!("Cannot load the block height in a non-finalize context"),
            // If the operand is the network ID, throw an error.
            Operand::NetworkID => bail!("Cannot load the network ID in a non-finalize context"),
        };

        // Retrieve the circuit value.
        let circuit_value =
            self.circuit_registers.get(&register.locator()).ok_or_else(|| anyhow!("'{register}' does not exist"))?;

        // Return the value for the given register or register access.
        let circuit_value = match register {
            // If the register is a locator, then return the stack value.
            Register::Locator(..) => circuit_value.clone(),
            // If the register is a register access, then load the specific stack value.
            Register::Access(_, ref path) => {
                // Inject the path.
                let path = path.iter().map(|access| circuit::Access::constant(*access)).collect::<Vec<_>>();

                match circuit_value {
                    // Retrieve the plaintext member from the path.
                    circuit::Value::Plaintext(plaintext) => circuit::Value::Plaintext(plaintext.find(&path)?),
                    // Retrieve the record entry from the path.
                    circuit::Value::Record(record) => match record.find(&path)? {
                        circuit::Entry::Constant(plaintext)
                        | circuit::Entry::Public(plaintext)
                        | circuit::Entry::Private(plaintext) => circuit::Value::Plaintext(plaintext),
                    },
                    // Retrieve the argument from the future.
                    circuit::Value::Future(future) => future.find(&path)?,
                }
            }
        };

        // Retrieve the register type.
        match self.register_types.get_type(stack, register) {
            // Ensure the stack value matches the register type.
            Ok(register_type) => {
                stack.matches_register_type(&circuit::Eject::eject_value(&circuit_value), &register_type)?
            }
            // Ensure the register is defined.
            Err(error) => bail!("Register '{register}' is not a member of the function: {error}"),
        };

        Ok(circuit_value)
    }
}
