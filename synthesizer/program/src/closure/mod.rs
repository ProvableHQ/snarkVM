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

mod output;
use output::*;

mod bytes;
mod parse;

use crate::InstructionTrait;
use console::{
    network::prelude::*,
    program::{Identifier, Register, RegisterType},
};

use indexmap::IndexSet;

#[derive(Clone, PartialEq, Eq)]
pub struct ClosureCore<N: Network, Instruction: InstructionTrait<N>> {
    /// The name of the closure.
    name: Identifier<N>,
    /// The input statements, added in order of the input registers.
    /// Input assignments are ensured to match the ordering of the input statements.
    inputs: IndexSet<Input<N>>,
    /// The instructions, in order of execution.
    instructions: Vec<Instruction>,
    /// The output statements, in order of the desired output.
    outputs: IndexSet<Output<N>>,
}

impl<N: Network, Instruction: InstructionTrait<N>> ClosureCore<N, Instruction> {
    /// Initializes a new closure with the given name.
    pub fn new(name: Identifier<N>) -> Self {
        Self { name, inputs: IndexSet::new(), instructions: Vec::new(), outputs: IndexSet::new() }
    }

    /// Returns the name of the closure.
    pub const fn name(&self) -> &Identifier<N> {
        &self.name
    }

    /// Returns the closure inputs.
    pub const fn inputs(&self) -> &IndexSet<Input<N>> {
        &self.inputs
    }

    /// Returns the closure instructions.
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    /// Returns the closure outputs.
    pub const fn outputs(&self) -> &IndexSet<Output<N>> {
        &self.outputs
    }
}

impl<N: Network, Instruction: InstructionTrait<N>> ClosureCore<N, Instruction> {
    /// Adds the input statement to the closure.
    ///
    /// # Errors
    /// This method will halt if there are instructions or output statements already.
    /// This method will halt if the maximum number of inputs has been reached.
    /// This method will halt if the input statement was previously added.
    #[inline]
    fn add_input(&mut self, input: Input<N>) -> Result<()> {
        // Ensure there are no instructions or output statements in memory.
        ensure!(self.instructions.is_empty(), "Cannot add inputs after instructions have been added");
        ensure!(self.outputs.is_empty(), "Cannot add inputs after outputs have been added");

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

    /// Adds the given instruction to the closure.
    ///
    /// # Errors
    /// This method will halt if there are output statements already.
    /// This method will halt if the maximum number of instructions has been reached.
    #[inline]
    pub fn add_instruction(&mut self, instruction: Instruction) -> Result<()> {
        // Ensure that there are no outputs in memory.
        ensure!(self.outputs.is_empty(), "Cannot add instructions after outputs have been added");

        // Ensure the maximum number of instructions has not been exceeded.
        ensure!(
            self.instructions.len() < N::MAX_INSTRUCTIONS,
            "Cannot add more than {} instructions",
            N::MAX_INSTRUCTIONS
        );

        // Ensure the destination register is a locator.
        for register in instruction.destinations() {
            ensure!(matches!(register, Register::Locator(..)), "Destination register must be a locator");
        }

        // Insert the instruction.
        self.instructions.push(instruction);
        Ok(())
    }

    /// Adds the output statement to the closure.
    ///
    /// # Errors
    /// This method will halt if the maximum number of outputs has been reached.
    #[inline]
    fn add_output(&mut self, output: Output<N>) -> Result<()> {
        // Ensure the maximum number of outputs has not been exceeded.
        ensure!(self.outputs.len() < N::MAX_OUTPUTS, "Cannot add more than {} outputs", N::MAX_OUTPUTS);

        // Ensure the closure output register is not a record.
        ensure!(!matches!(output.register_type(), RegisterType::Record(..)), "Output register cannot be a record");

        // Insert the output statement.
        self.outputs.insert(output);
        Ok(())
    }
}

impl<N: Network, Instruction: InstructionTrait<N>> TypeName for ClosureCore<N, Instruction> {
    /// Returns the type name as a string.
    #[inline]
    fn type_name() -> &'static str {
        "closure"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{Closure, Instruction};

    type CurrentNetwork = console::network::MainnetV0;

    #[test]
    fn test_add_input() {
        // Initialize a new closure instance.
        let name = Identifier::from_str("closure_core_test").unwrap();
        let mut closure = Closure::<CurrentNetwork>::new(name);

        // Ensure that an input can be added.
        let input = Input::<CurrentNetwork>::from_str("input r0 as field;").unwrap();
        assert!(closure.add_input(input.clone()).is_ok());

        // Ensure that adding a duplicate input will fail.
        assert!(closure.add_input(input).is_err());

        // Ensure that adding more than the maximum number of inputs will fail.
        for i in 1..CurrentNetwork::MAX_INPUTS * 2 {
            let input = Input::<CurrentNetwork>::from_str(&format!("input r{i} as field;")).unwrap();

            match closure.inputs.len() < CurrentNetwork::MAX_INPUTS {
                true => assert!(closure.add_input(input).is_ok()),
                false => assert!(closure.add_input(input).is_err()),
            }
        }
    }

    #[test]
    fn test_add_instruction() {
        // Initialize a new closure instance.
        let name = Identifier::from_str("closure_core_test").unwrap();
        let mut closure = Closure::<CurrentNetwork>::new(name);

        // Ensure that an instruction can be added.
        let instruction = Instruction::<CurrentNetwork>::from_str("add r0 r1 into r2;").unwrap();
        assert!(closure.add_instruction(instruction).is_ok());

        // Ensure that adding more than the maximum number of instructions will fail.
        for i in 3..CurrentNetwork::MAX_INSTRUCTIONS * 2 {
            let instruction = Instruction::<CurrentNetwork>::from_str(&format!("add r0 r1 into r{i};")).unwrap();

            match closure.instructions.len() < CurrentNetwork::MAX_INSTRUCTIONS {
                true => assert!(closure.add_instruction(instruction).is_ok()),
                false => assert!(closure.add_instruction(instruction).is_err()),
            }
        }
    }

    #[test]
    fn test_add_output() {
        // Initialize a new closure instance.
        let name = Identifier::from_str("closure_core_test").unwrap();
        let mut closure = Closure::<CurrentNetwork>::new(name);

        // Ensure that an output can be added.
        let output = Output::<CurrentNetwork>::from_str("output r0 as field;").unwrap();
        assert!(closure.add_output(output).is_ok());

        // Ensure that adding more than the maximum number of outputs will fail.
        for i in 1..CurrentNetwork::MAX_OUTPUTS * 2 {
            let output = Output::<CurrentNetwork>::from_str(&format!("output r{i} as field;")).unwrap();

            match closure.outputs.len() < CurrentNetwork::MAX_OUTPUTS {
                true => assert!(closure.add_output(output).is_ok()),
                false => assert!(closure.add_output(output).is_err()),
            }
        }
    }
}
