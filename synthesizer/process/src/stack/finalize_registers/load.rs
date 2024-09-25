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
use console::program::FinalizeType;

impl<N: Network> RegistersLoad<N> for FinalizeRegisters<N> {
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
            // If the operand is the signer, throw an error.
            Operand::Signer => bail!("Forbidden operation: Cannot use 'self.signer' in 'finalize'"),
            // If the operand is the caller, throw an error.
            Operand::Caller => bail!("Forbidden operation: Cannot use 'self.caller' in 'finalize'"),
            // If the operand is the block height, load the block height.
            Operand::BlockHeight => {
                return Ok(Value::Plaintext(Plaintext::from(Literal::U32(U32::new(self.state.block_height())))));
            }
            // If the operand is the network ID, load the network ID.
            Operand::NetworkID => {
                return Ok(Value::Plaintext(Plaintext::from(Literal::U16(U16::new(N::ID)))));
            }
        };

        // Retrieve the value.
        let value = self.registers.get(&register.locator()).ok_or_else(|| anyhow!("'{register}' does not exist"))?;

        // Return the value for the given register or register access.
        let value = match register {
            // If the register is a locator, then return the plaintext value.
            Register::Locator(..) => value.clone(),
            // If the register is a register access, then load the specific plaintext value.
            Register::Access(_, ref path) => value.find(path)?,
        };

        // Retrieve the type of the register.
        match (self.finalize_types.get_type(stack, register), &value) {
            // Ensure the plaintext value matches the register type.
            (Ok(FinalizeType::Plaintext(plaintext_type)), Value::Plaintext(plaintext_value)) => {
                stack.matches_plaintext(plaintext_value, &plaintext_type)?
            }
            // Ensure the future value matches the register type.
            (Ok(FinalizeType::Future(locator)), Value::Future(future)) => stack.matches_future(future, &locator)?,
            // Ensure the load is valid in a finalize context.
            (Ok(finalize_type), stack_value) => bail!(
                "Attempted to load a '{stack_value}' value from a register '{register}' of type '{finalize_type}' in a finalize scope",
            ),
            // Ensure the register is defined.
            (Err(error), _) => bail!("Register '{register}' is not a member of the function: {error}"),
        };

        Ok(value)
    }
}
