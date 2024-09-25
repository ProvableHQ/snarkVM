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

mod input;
use input::*;

mod bytes;
mod parse;

use crate::traits::CommandTrait;
use console::{
    network::prelude::*,
    program::{FinalizeType, Identifier, Register},
};

use indexmap::IndexSet;
use std::collections::HashMap;

#[derive(Clone, PartialEq, Eq)]
pub struct FinalizeCore<N: Network, Command: CommandTrait<N>> {
    /// The name of the associated function.
    name: Identifier<N>,
    /// The input statements, added in order of the input registers.
    /// Input assignments are ensured to match the ordering of the input statements.
    inputs: IndexSet<Input<N>>,
    /// The commands, in order of execution.
    commands: Vec<Command>,
    /// The number of write commands.
    num_writes: u16,
    /// A mapping from `Position`s to their index in `commands`.
    positions: HashMap<Identifier<N>, usize>,
}

impl<N: Network, Command: CommandTrait<N>> FinalizeCore<N, Command> {
    /// Initializes a new finalize with the given name.
    pub fn new(name: Identifier<N>) -> Self {
        Self { name, inputs: IndexSet::new(), commands: Vec::new(), num_writes: 0, positions: HashMap::new() }
    }

    /// Returns the name of the associated function.
    pub const fn name(&self) -> &Identifier<N> {
        &self.name
    }

    /// Returns the finalize inputs.
    pub const fn inputs(&self) -> &IndexSet<Input<N>> {
        &self.inputs
    }

    /// Returns the finalize input types.
    pub fn input_types(&self) -> Vec<FinalizeType<N>> {
        self.inputs.iter().map(|input| input.finalize_type()).cloned().collect()
    }

    /// Returns the finalize commands.
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// Returns the number of write commands.
    pub const fn num_writes(&self) -> u16 {
        self.num_writes
    }

    /// Returns the mapping of `Position`s to their index in `commands`.
    pub const fn positions(&self) -> &HashMap<Identifier<N>, usize> {
        &self.positions
    }
}

impl<N: Network, Command: CommandTrait<N>> FinalizeCore<N, Command> {
    /// Adds the input statement to finalize.
    ///
    /// # Errors
    /// This method will halt if a command was previously added.
    /// This method will halt if the maximum number of inputs has been reached.
    /// This method will halt if the input statement was previously added.
    #[inline]
    fn add_input(&mut self, input: Input<N>) -> Result<()> {
        // Ensure there are no commands in memory.
        ensure!(self.commands.is_empty(), "Cannot add inputs after commands have been added");

        // Ensure the maximum number of inputs has not been exceeded.
        ensure!(self.inputs.len() < N::MAX_INPUTS, "Cannot add more than {} inputs", N::MAX_INPUTS);
        // Ensure the input statement was not previously added.
        ensure!(!self.inputs.contains(&input), "Cannot add duplicate input statement");

        // Ensure the input register is a locator.
        ensure!(matches!(input.register(), Register::Locator(..)), "Input register must be a locator");

        // Insert the input statement.
        self.inputs.insert(input);
        Ok(())
    }

    /// Adds the given command to finalize.
    ///
    /// # Errors
    /// This method will halt if the maximum number of commands has been reached.
    #[inline]
    pub fn add_command(&mut self, command: Command) -> Result<()> {
        // Ensure the maximum number of commands has not been exceeded.
        ensure!(self.commands.len() < N::MAX_COMMANDS, "Cannot add more than {} commands", N::MAX_COMMANDS);
        // Ensure the number of write commands has not been exceeded.
        ensure!(self.num_writes < N::MAX_WRITES, "Cannot add more than {} 'set' & 'remove' commands", N::MAX_WRITES);

        // Ensure the command is not a call instruction.
        ensure!(!command.is_call(), "Forbidden operation: Finalize cannot invoke a 'call'");
        // Ensure the command is not a cast to record instruction.
        ensure!(!command.is_cast_to_record(), "Forbidden operation: Finalize cannot cast to a record");

        // Check the destination registers.
        for register in command.destinations() {
            // Ensure the destination register is a locator.
            ensure!(matches!(register, Register::Locator(..)), "Destination register must be a locator");
        }

        // Check if the command is a branch command.
        if let Some(position) = command.branch_to() {
            // Ensure the branch target does not reference an earlier position.
            ensure!(!self.positions.contains_key(position), "Cannot branch to an earlier position '{position}'");
        }

        // Check if the command is a position command.
        if let Some(position) = command.position() {
            // Ensure the position is not yet defined.
            ensure!(!self.positions.contains_key(position), "Cannot redefine position '{position}'");
            // Ensure that there are less than `u8::MAX` positions.
            ensure!(self.positions.len() < u8::MAX as usize, "Cannot add more than {} positions", u8::MAX);
            // Insert the position.
            self.positions.insert(*position, self.commands.len());
        }

        // Check if the command is a write command.
        if command.is_write() {
            // Increment the number of write commands.
            self.num_writes += 1;
        }

        // Insert the command.
        self.commands.push(command);
        Ok(())
    }
}

impl<N: Network, Command: CommandTrait<N>> TypeName for FinalizeCore<N, Command> {
    /// Returns the type name as a string.
    #[inline]
    fn type_name() -> &'static str {
        "finalize"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{Command, Finalize};

    type CurrentNetwork = console::network::MainnetV0;

    #[test]
    fn test_add_input() {
        // Initialize a new finalize instance.
        let name = Identifier::from_str("finalize_core_test").unwrap();
        let mut finalize = Finalize::<CurrentNetwork>::new(name);

        // Ensure that an input can be added.
        let input = Input::<CurrentNetwork>::from_str("input r0 as field.public;").unwrap();
        assert!(finalize.add_input(input.clone()).is_ok());

        // Ensure that adding a duplicate input will fail.
        assert!(finalize.add_input(input).is_err());

        // Ensure that adding more than the maximum number of inputs will fail.
        for i in 1..CurrentNetwork::MAX_INPUTS * 2 {
            let input = Input::<CurrentNetwork>::from_str(&format!("input r{i} as field.public;")).unwrap();

            match finalize.inputs.len() < CurrentNetwork::MAX_INPUTS {
                true => assert!(finalize.add_input(input).is_ok()),
                false => assert!(finalize.add_input(input).is_err()),
            }
        }
    }

    #[test]
    fn test_add_command() {
        // Initialize a new finalize instance.
        let name = Identifier::from_str("finalize_core_test").unwrap();
        let mut finalize = Finalize::<CurrentNetwork>::new(name);

        // Ensure that a command can be added.
        let command = Command::<CurrentNetwork>::from_str("add r0 r1 into r2;").unwrap();
        assert!(finalize.add_command(command).is_ok());

        // Ensure that adding more than the maximum number of commands will fail.
        for i in 3..CurrentNetwork::MAX_COMMANDS * 2 {
            let command = Command::<CurrentNetwork>::from_str(&format!("add r0 r1 into r{i};")).unwrap();

            match finalize.commands.len() < CurrentNetwork::MAX_COMMANDS {
                true => assert!(finalize.add_command(command).is_ok()),
                false => assert!(finalize.add_command(command).is_err()),
            }
        }

        // Ensure that adding more than the maximum number of writes will fail.

        // Initialize a new finalize instance.
        let name = Identifier::from_str("finalize_core_test").unwrap();
        let mut finalize = Finalize::<CurrentNetwork>::new(name);

        for _ in 0..CurrentNetwork::MAX_WRITES * 2 {
            let command = Command::<CurrentNetwork>::from_str("remove object[r0];").unwrap();

            match finalize.commands.len() < CurrentNetwork::MAX_WRITES as usize {
                true => assert!(finalize.add_command(command).is_ok()),
                false => assert!(finalize.add_command(command).is_err()),
            }
        }
    }

    #[test]
    fn test_add_command_duplicate_positions() {
        // Initialize a new finalize instance.
        let name = Identifier::from_str("finalize_core_test").unwrap();
        let mut finalize = Finalize::<CurrentNetwork>::new(name);

        // Ensure that a command can be added.
        let command = Command::<CurrentNetwork>::from_str("position start;").unwrap();
        assert!(finalize.add_command(command.clone()).is_ok());

        // Ensure that adding a duplicate position will fail.
        assert!(finalize.add_command(command).is_err());

        // Helper method to convert a number to a unique string.
        #[allow(clippy::cast_possible_truncation)]
        fn to_unique_string(mut n: usize) -> String {
            let mut s = String::new();
            while n > 0 {
                s.push((b'A' + (n % 26) as u8) as char);
                n /= 26;
            }
            s.chars().rev().collect::<String>()
        }

        // Ensure that adding more than the maximum number of positions will fail.
        for i in 1..u8::MAX as usize * 2 {
            let position = to_unique_string(i);
            println!("position: {}", position);
            let command = Command::<CurrentNetwork>::from_str(&format!("position {position};")).unwrap();

            match finalize.commands.len() < u8::MAX as usize {
                true => assert!(finalize.add_command(command).is_ok()),
                false => assert!(finalize.add_command(command).is_err()),
            }
        }
    }
}
