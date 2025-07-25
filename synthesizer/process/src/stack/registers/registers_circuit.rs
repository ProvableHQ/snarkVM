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

impl<N: Network, A: circuit::Aleo<Network = N>> RegistersCircuit<N, A> for Registers<N, A> {
    /// Returns the transition signer, as a circuit.
    #[inline]
    fn signer_circuit(&self) -> Result<circuit::Address<A>> {
        self.signer_circuit.clone().ok_or_else(|| anyhow!("Signer address (circuit) is not set in the registers."))
    }

    /// Sets the transition signer, as a circuit.
    #[inline]
    fn set_signer_circuit(&mut self, signer_circuit: circuit::Address<A>) {
        self.signer_circuit = Some(signer_circuit);
    }

    /// Returns the root transition view key, as a circuit.
    #[inline]
    fn root_tvk_circuit(&self) -> Result<circuit::Field<A>> {
        self.root_tvk_circuit.clone().ok_or_else(|| anyhow!("Root tvk (circuit) is not set in the registers."))
    }

    /// Sets the root transition view key, as a circuit.
    #[inline]
    fn set_root_tvk_circuit(&mut self, root_tvk_circuit: circuit::Field<A>) {
        self.root_tvk_circuit = Some(root_tvk_circuit);
    }

    /// Returns the transition caller, as a circuit.
    #[inline]
    fn caller_circuit(&self) -> Result<circuit::Address<A>> {
        self.caller_circuit.clone().ok_or_else(|| anyhow!("Caller address (circuit) is not set in the registers."))
    }

    /// Sets the transition caller, as a circuit.
    #[inline]
    fn set_caller_circuit(&mut self, caller_circuit: circuit::Address<A>) {
        self.caller_circuit = Some(caller_circuit);
    }

    /// Returns the transition view key, as a circuit.
    #[inline]
    fn tvk_circuit(&self) -> Result<circuit::Field<A>> {
        self.tvk_circuit.clone().ok_or_else(|| anyhow!("Transition view key (circuit) is not set in the registers."))
    }

    /// Sets the transition view key, as a circuit.
    #[inline]
    fn set_tvk_circuit(&mut self, tvk_circuit: circuit::Field<A>) {
        self.tvk_circuit = Some(tvk_circuit);
    }

    /// Loads the value of a given operand from the registers.
    ///
    /// # Errors
    /// This method will halt if the register locator is not found.
    /// In the case of register accesses, this method will halt if the access is not found.
    fn load_circuit(&self, stack: &impl StackTrait<N>, operand: &Operand<N>) -> Result<circuit::Value<A>> {
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
            // If the operand is the checksum, throw an error.
            Operand::Checksum(_) => bail!("Cannot load the checksum in a non-finalize context."),
            // If the operand is the edition, throw an error.
            Operand::Edition(_) => bail!("Cannot load the edition in a non-finalize context"),
            // If the operand is the program owner, throw an error.
            Operand::ProgramOwner(_) => bail!("Cannot load the program owner in a non-finalize context"),
        };

        // Retrieve the circuit value.
        let circuit_value =
            self.circuit_registers.get(&register.locator()).ok_or_else(|| anyhow!("'{register}' does not exist"))?;

        // Return the value for the given register or register access.
        let circuit_value = match register {
            // If the register is a locator, then return the stack value.
            Register::Locator(..) => circuit_value.clone(),
            // If the register is a register access, then load the specific stack value.
            Register::Access(_, path) => {
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

    /// Assigns the given value to the given register, assuming the register is not already assigned.
    ///
    /// # Errors
    /// This method will halt if the given register is a register access.
    /// This method will halt if the given register is an input register.
    /// This method will halt if the register is already used.
    fn store_circuit(
        &mut self,
        stack: &impl StackTrait<N>,
        register: &Register<N>,
        circuit_value: circuit::Value<A>,
    ) -> Result<()> {
        match register {
            Register::Locator(locator) => {
                // Ensure the register assignments are monotonically increasing.
                let expected_locator = self.circuit_registers.len() as u64;
                ensure!(expected_locator == *locator, "Out-of-order write operation at '{register}'");
                // Ensure the register does not already exist.
                ensure!(
                    !self.circuit_registers.contains_key(locator),
                    "Cannot write to occupied register '{register}'"
                );

                // Ensure the register type is valid.
                match self.register_types.get_type(stack, register) {
                    // Ensure the stack value matches the register type.
                    Ok(register_type) => {
                        stack.matches_register_type(&circuit::Eject::eject_value(&circuit_value), &register_type)?
                    }
                    // Ensure the register is defined.
                    Err(error) => bail!("Register '{register}' is missing a type definition: {error}"),
                };

                // Store the stack value.
                match self.circuit_registers.insert(*locator, circuit_value) {
                    // Ensure the register has not been previously stored.
                    Some(..) => bail!("Attempted to write to register '{register}' again"),
                    // Return on success.
                    None => Ok(()),
                }
            }
            // Ensure the register is not a register access.
            Register::Access(..) => bail!("Cannot store to a register access: '{register}'"),
        }
    }
}
