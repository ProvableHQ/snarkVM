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

mod helpers;
pub use helpers::*;

mod serialize;
mod string;

use console::{
    network::prelude::*,
    program::{Identifier, Literal, Locator, Plaintext, ProgramID},
    types::Field,
};
use ledger_block::{Execution, Input, Output, Transition};

use indexmap::IndexMap;

#[derive(Clone, PartialEq, Eq)]
pub struct Restrictions<N: Network> {
    /// The restrictions ID, for the current state of the `Restrictions` list.
    restrictions_id: Field<N>,
    /// The set of program IDs that are restricted from being executed.
    /// e.g. `restricted.aleo` => `..` (all blocks)
    /// e.g. `restricted.aleo` => `10..` (from block 10 onwards)
    /// e.g. `restricted.aleo` => `..10` (up to block 10)
    /// e.g. `restricted.aleo` => `10..20` (from block 10 to block 20)
    programs: IndexMap<ProgramID<N>, BlockRange>,
    /// The set of `(program ID, function name)` pairs that are restricted from being executed.
    /// e.g. `restricted.aleo/foo` => `..` (all blocks)
    /// e.g. `restricted.aleo/foo` => `10..` (from block 10 onwards)
    /// e.g. `restricted.aleo/foo` => `..10` (up to block 10)
    /// e.g. `restricted.aleo/foo` => `10..20` (from block 10 to block 20)
    functions: IndexMap<Locator<N>, BlockRange>,
    /// The set of `(program ID, function name, argument)` triples that are restricted from being executed.
    /// e.g. `restricted.aleo/bar _ aleo1zkpxxxxx _ _` => `..` (all blocks)
    /// e.g. `restricted.aleo/bar _ aleo1zkpxxxxx _ _` => `10..` (from block 10 onwards)
    /// e.g. `restricted.aleo/bar _ aleo1zkpxxxxx _ _` => `..10` (up to block 10)
    /// e.g. `restricted.aleo/bar _ aleo1zkpxxxxx _ _` => `10..20` (from block 10 to block 20)
    ///
    /// Note: This design intentionally minimizes the number of total lookups required to check
    /// for restrictions when a transition matches the described profile. In summary:
    /// - When a transition does not match the program ID or function name, the total lookup cost is `O(1)`.
    /// - When a transition matches the program ID & function name, the initial lookup cost is `O(num_inputs + num_outputs)`.
    ///    - If an input or output index does not match, the additional lookup cost is `0`.
    ///    - If an input or output index matches, the additional lookup cost is `O(n)` for `n` restricted arguments with the same index.
    arguments: IndexMap<Locator<N>, IndexMap<ArgumentLocator, IndexMap<Literal<N>, BlockRange>>>,
}

impl<N: Network> Restrictions<N> {
    /// Initializes the `Restrictions` instance for the current network.
    pub fn load() -> Result<Self> {
        // Load the restrictions list from the network.
        let restrictions = Self::from_str(N::restrictions_list_as_str())?;
        // Ensure the restrictions ID matches the computed value.
        let expected_restrictions_id =
            Self::compute_restrictions_id(&restrictions.programs, &restrictions.functions, &restrictions.arguments)?;
        if restrictions.restrictions_id != expected_restrictions_id {
            bail!(
                "The restrictions ID does not match the computed value upon initialization (expected - {expected_restrictions_id})"
            );
        }
        // Return the restrictions.
        Ok(restrictions)
    }

    /// Initializes a new `Restrictions` instance.
    pub fn new_blank() -> Result<Self> {
        Ok(Self {
            restrictions_id: Self::compute_restrictions_id(&IndexMap::new(), &IndexMap::new(), &IndexMap::new())?,
            programs: IndexMap::new(),
            functions: IndexMap::new(),
            arguments: IndexMap::new(),
        })
    }
}

impl<N: Network> Restrictions<N> {
    /// Returns the restrictions ID, for the current state of the `Restrictions` list.
    pub const fn restrictions_id(&self) -> Field<N> {
        self.restrictions_id
    }

    /// Returns the set of program IDs that are restricted from being executed.
    pub const fn programs(&self) -> &IndexMap<ProgramID<N>, BlockRange> {
        &self.programs
    }

    /// Returns the set of `(program ID, function ID)` pairs that are restricted from being executed.
    pub const fn functions(&self) -> &IndexMap<Locator<N>, BlockRange> {
        &self.functions
    }

    /// Returns the set of `(program ID, function ID, argument)` triples that are restricted from being executed.
    pub const fn arguments(
        &self,
    ) -> &IndexMap<Locator<N>, IndexMap<ArgumentLocator, IndexMap<Literal<N>, BlockRange>>> {
        &self.arguments
    }
}

impl<N: Network> Restrictions<N> {
    /// Returns `true` if the given program ID is restricted from being executed.
    pub fn is_program_restricted(&self, program_id: &ProgramID<N>, block_height: u32) -> bool {
        self.programs.get(program_id).map_or(false, |range| range.contains(block_height))
    }

    /// Returns `true` if the given `(program ID, function name)` pair is restricted from being executed.
    pub fn is_function_restricted(
        &self,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        block_height: u32,
    ) -> bool {
        self.functions
            .get(&Locator::new(*program_id, *function_name))
            .map_or(false, |range| range.contains(block_height))
    }

    /// Returns `true` if the given `(program ID, function ID, argument)` triple is restricted from being executed.
    pub fn is_argument_restricted(&self, transition: &Transition<N>, block_height: u32) -> bool {
        self.arguments.get(&Locator::new(*transition.program_id(), *transition.function_name())).map_or(
            false,
            |entries| {
                // Check if any argument is restricted and return `true` if one is found.
                for (argument_locator, arguments) in entries {
                    match argument_locator.is_input() {
                        true => {
                            if let Some(argument) = transition.inputs().get(argument_locator.index() as usize) {
                                match argument {
                                    Input::Constant(_, Some(plaintext)) | Input::Public(_, Some(plaintext)) => {
                                        match plaintext {
                                            Plaintext::Literal(literal, _) => {
                                                if let Some(range) = arguments.get(literal) {
                                                    if range.contains(block_height) {
                                                        return true;
                                                    }
                                                }
                                            }
                                            Plaintext::Struct(..) | Plaintext::Array(..) => continue,
                                        }
                                    }
                                    _ => continue,
                                }
                            }
                        }
                        false => {
                            if let Some(argument) = transition.outputs().get(argument_locator.index() as usize) {
                                match argument {
                                    Output::Constant(_, Some(plaintext)) | Output::Public(_, Some(plaintext)) => {
                                        match plaintext {
                                            Plaintext::Literal(literal, _) => {
                                                if let Some(range) = arguments.get(literal) {
                                                    if range.contains(block_height) {
                                                        return true;
                                                    }
                                                }
                                            }
                                            Plaintext::Struct(..) | Plaintext::Array(..) => continue,
                                        }
                                    }
                                    _ => continue,
                                }
                            }
                        }
                    }
                }
                // Otherwise, return `false`.
                false
            },
        )
    }
}

impl<N: Network> Restrictions<N> {
    /// Returns `true` if the given execution contains any restricted transitions for the given block height.
    pub fn contains_restricted_transitions(&self, execution: &Execution<N>, block_height: u32) -> bool {
        // Check if any transition is restricted.
        execution.transitions().any(|transition| {
            // Retrieve the program ID.
            let program_id = transition.program_id();
            // Retrieve the function name.
            let function_name = transition.function_name();

            // If the program is restricted, then the transition is restricted.
            if self.is_program_restricted(program_id, block_height) {
                return true;
            }
            // If the function is restricted, then the transition is restricted.
            if self.is_function_restricted(program_id, function_name, block_height) {
                return true;
            }
            // If any argument is restricted, then the transition is restricted.
            if self.is_argument_restricted(transition, block_height) {
                return true;
            }
            // Otherwise, the transition is not restricted.
            false
        })
    }
}

impl<N: Network> Restrictions<N> {
    /// Returns the restrictions ID.
    pub fn compute_restrictions_id(
        programs: &IndexMap<ProgramID<N>, BlockRange>,
        functions: &IndexMap<Locator<N>, BlockRange>,
        arguments: &IndexMap<Locator<N>, IndexMap<ArgumentLocator, IndexMap<Literal<N>, BlockRange>>>,
    ) -> Result<Field<N>> {
        // Prepare the preimage data.
        let mut preimage = Vec::new();

        // Append the number of programs.
        preimage.push(Field::from_u64(programs.len() as u64));
        // Encode the programs.
        for (program_id, range) in programs {
            preimage.extend_from_slice(&program_id.to_fields()?);
            preimage.extend_from_slice(&range.to_fields()?);
        }

        // Append the number of functions.
        preimage.push(Field::from_u64(functions.len() as u64));
        // Encode the functions.
        for (locator, range) in functions {
            preimage.extend_from_slice(&locator.program_id().to_fields()?);
            preimage.push(locator.resource().to_field()?);
            preimage.extend_from_slice(&range.to_fields()?);
        }

        // Append the number of arguments.
        preimage.push(Field::from_u64(arguments.len() as u64));
        // Encode the arguments.
        for (locator, entries) in arguments {
            preimage.extend_from_slice(&locator.program_id().to_fields()?);
            preimage.push(locator.resource().to_field()?);
            // Append the number of argument entries.
            preimage.push(Field::from_u64(entries.len() as u64));
            // Encode the argument entries.
            for (argument_locator, arguments) in entries {
                preimage.push(if argument_locator.is_input() { Field::one() } else { Field::zero() });
                preimage.push(Field::from_u16(argument_locator.index()));
                // Append the number of arguments.
                preimage.push(Field::from_u64(arguments.len() as u64));
                // Encode the arguments.
                for (literal, range) in arguments {
                    // Encode the literal.
                    preimage.extend_from_slice(&Plaintext::from(literal).to_fields()?);
                    // Encode the range.
                    preimage.extend_from_slice(&range.to_fields()?);
                }
            }
        }

        // Hash the preimage data.
        // Note: This call must be collision-resistant, and so we use BHP-1024.
        N::hash_bhp1024(&preimage.to_bits_le())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::types::I8;

    use indexmap::indexmap;
    use ledger_block::Input;

    type CurrentNetwork = console::network::MainnetV0;

    #[test]
    fn test_restrictions_program_restricted() {
        let mut restrictions = Restrictions::<CurrentNetwork>::new_blank().unwrap();
        let program_id = ProgramID::from_str("restricted.aleo").unwrap();
        let range = BlockRange::Range(10..20);
        restrictions.programs.insert(program_id, range);
        assert!(!restrictions.is_program_restricted(&program_id, 5));
        assert!(restrictions.is_program_restricted(&program_id, 10));
        assert!(restrictions.is_program_restricted(&program_id, 15));
        assert!(!restrictions.is_program_restricted(&program_id, 20));
        assert!(!restrictions.is_program_restricted(&program_id, 25));
    }

    #[test]
    fn test_restrictions_function_restricted() {
        let mut restrictions = Restrictions::<CurrentNetwork>::new_blank().unwrap();
        let program_id = ProgramID::from_str("restricted.aleo").unwrap();
        let function_id = Identifier::from_str("foo").unwrap();
        let range = BlockRange::Range(10..20);
        restrictions.functions.insert(Locator::new(program_id, function_id), range);
        assert!(!restrictions.is_function_restricted(&program_id, &function_id, 5));
        assert!(restrictions.is_function_restricted(&program_id, &function_id, 10));
        assert!(restrictions.is_function_restricted(&program_id, &function_id, 15));
        assert!(!restrictions.is_function_restricted(&program_id, &function_id, 20));
        assert!(!restrictions.is_function_restricted(&program_id, &function_id, 25));
    }

    #[test]
    fn test_restrictions_argument_restricted() {
        let rng = &mut TestRng::default();

        let mut restrictions = Restrictions::<CurrentNetwork>::new_blank().unwrap();
        let program_id = ProgramID::from_str("restricted.aleo").unwrap();
        let function_id = Identifier::from_str("bar").unwrap();
        let range = BlockRange::Range(10..20);

        let literal = Literal::I8(I8::new(42));
        let index = 0;
        restrictions.arguments.insert(
            Locator::new(program_id, function_id),
            indexmap!( ArgumentLocator::new(true, index) => indexmap!( literal.clone() => range )),
        );

        let input = Input::Public(rng.gen(), Some(literal.into()));
        let transition =
            Transition::new(program_id, function_id, vec![input], vec![], rng.gen(), rng.gen(), rng.gen()).unwrap();
        assert!(!restrictions.is_argument_restricted(&transition, 5));
        assert!(restrictions.is_argument_restricted(&transition, 10));
        assert!(restrictions.is_argument_restricted(&transition, 15));
        assert!(!restrictions.is_argument_restricted(&transition, 20));
        assert!(!restrictions.is_argument_restricted(&transition, 25));
    }

    /// **Attention**: This method is used to auto-generate the restrictions lists for each network
    /// to be used by the `snarkvm_parameters` crate.
    #[test]
    fn test_restrictions_list_comparison() {
        #[rustfmt::skip]
        macro_rules! check_restrictions {
            ($restrictions:expr, $network:ident) => {{
                // Write the restrictions to a JSON-compatible string.
                let restrictions_string = $restrictions.to_string();
                // Compute the restrictions ID.
                let restrictions_id = $restrictions.restrictions_id();
                // Print out the restrictions list.
                println!("========\n Restrictions for '{}' ({restrictions_id})\n========\n{restrictions_string}", Network::NAME);
                // Compare the restrictions list.
                assert_eq!(
                    restrictions_string,
                    Restrictions::<$network>::load().unwrap().to_string(),
                    "Ensure 'snarkvm_parameters/src/NETWORK/resources/restrictions.json' matches 'restrictions_string' in this test"
                );
            }};
        }

        // Attention: The 'restrictions' variable **must match** the 'restrictions.json' in 'snarkvm_parameters' for each network.
        {
            // Set the network.
            type Network = console::network::MainnetV0;
            // Initialize the restrictions.
            let restrictions = Restrictions::<Network>::new_blank().unwrap();
            // Check the restrictions.
            check_restrictions!(restrictions, Network);
        }

        // Attention: The 'restrictions' variable **must match** the 'restrictions.json' in 'snarkvm_parameters' for each network.
        {
            // Set the network.
            type Network = console::network::TestnetV0;
            // Initialize the restrictions.
            let restrictions = Restrictions::<Network>::new_blank().unwrap();
            // Check the restrictions.
            check_restrictions!(restrictions, Network);
        }

        // Attention: The 'restrictions' variable **must match** the 'restrictions.json' in 'snarkvm_parameters' for each network.
        {
            // Set the network.
            type Network = console::network::CanaryV0;
            // Initialize the restrictions.
            let restrictions = Restrictions::<Network>::new_blank().unwrap();
            // Check the restrictions.
            check_restrictions!(restrictions, Network);
        }
    }
}
