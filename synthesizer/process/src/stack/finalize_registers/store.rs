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

impl<N: Network> RegistersStore<N> for FinalizeRegisters<N> {
    /// Assigns the given value to the given register, assuming the register is not already assigned.
    ///
    /// # Errors
    /// This method will halt if the given register is a register access.
    /// This method will halt if the given register is an input register.
    /// This method will halt if the register is already used.
    #[inline]
    fn store(
        &mut self,
        stack: &(impl StackMatches<N> + StackProgram<N>),
        register: &Register<N>,
        stack_value: Value<N>,
    ) -> Result<()> {
        // Store the value to the register.
        match (register, stack_value) {
            (Register::Locator(locator), stack_value) => {
                // Ensure the register assignments are monotonically increasing.
                match self.last_register {
                    None => ensure!(*locator == 0, "Out-of-order write operation at '{register}'"),
                    Some(last) => ensure!(*locator > last, "Out-of-order write operation at '{register}'"),
                };
                // Ensure the register does not already exist.
                ensure!(!self.registers.contains_key(locator), "Cannot write to occupied register '{register}'");

                // Ensure the type of the register is valid.
                match (self.finalize_types.get_type(stack, register), &stack_value) {
                    // Ensure the plaintext value matches the plaintext type.
                    (Ok(FinalizeType::Plaintext(plaintext_type)), Value::Plaintext(plaintext_value)) => {
                        stack.matches_plaintext(plaintext_value, &plaintext_type)?
                    }
                    // Ensure the future value matches the future type.
                    (Ok(FinalizeType::Future(locator)), Value::Future(future)) => {
                        stack.matches_future(future, &locator)?
                    }
                    // Ensure the store is valid in a finalize context.
                    (Ok(finalize_type), stack_value) => bail!(
                        "Attempted to store a '{stack_value}' value in a register '{register}' of type '{finalize_type}' in a finalize scope",
                    ),
                    // Ensure the register is defined.
                    (Err(error), _) => bail!("Register '{register}' is missing a type definition: {error}"),
                };

                // Store the plaintext value.
                match self.registers.insert(*locator, stack_value) {
                    // Ensure the register has not been previously stored.
                    Some(..) => bail!("Attempted to write to register '{register}' again"),
                    // Update the last register locator, and return on success.
                    None => {
                        // Update the last register locator.
                        self.last_register = Some(*locator);
                        // Return on success.
                        Ok(())
                    }
                }
            }
            // Ensure the register is not a register access.
            (Register::Access(..), _) => bail!("Cannot store to a register access: '{register}'"),
        }
    }
}
