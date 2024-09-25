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

impl<N: Network> Transition<N> {
    /// Returns the transition root, by computing the root for a Merkle tree of the input and output IDs.
    pub fn to_root(&self) -> Result<Field<N>> {
        Ok(*self.to_tree()?.root())
    }

    /// Returns the Merkle path for the transition leaf.
    pub fn to_path(&self, leaf: &TransitionLeaf<N>) -> Result<TransitionPath<N>> {
        // Compute the Merkle path.
        self.to_tree()?.prove(leaf.index() as usize, &leaf.to_bits_le())
    }

    /// Returns the Merkle leaf for the given input or output ID in the transition.
    pub fn to_leaf(&self, id: &Field<N>, is_input: bool) -> Result<TransitionLeaf<N>> {
        // Match on the input or output ID.
        match is_input {
            true => {
                // Iterate through the transition inputs.
                for (index, input) in self.inputs.iter().enumerate() {
                    // Check if the input ID matches the given ID.
                    if id == input.id() {
                        // Return the transition leaf.
                        return Ok(input.to_transition_leaf(u8::try_from(index)?));
                    }
                }
                // Error if the input ID was not found.
                bail!("Input ID not found in transition")
            }
            false => {
                // Iterate through the transition outputs.
                for (index, output) in self.outputs.iter().enumerate() {
                    // Check if the output ID matches the given ID.
                    if id == output.id() {
                        // Return the transition leaf.
                        return Ok(output.to_transition_leaf(u8::try_from(self.inputs.len() + index)?));
                    }
                }
                // Error if the output ID was not found.
                bail!("Output ID not found in transition")
            }
        }
    }

    /// The Merkle tree of input and output IDs for the transition.
    pub fn to_tree(&self) -> Result<TransitionTree<N>> {
        Self::function_tree(&self.inputs, &self.outputs)
    }

    /// Returns the Merkle tree for the given inputs and outputs.
    pub(super) fn function_tree(inputs: &[Input<N>], outputs: &[Output<N>]) -> Result<TransitionTree<N>> {
        // Ensure the number of inputs is within the allowed range.
        ensure!(
            inputs.len() <= N::MAX_INPUTS,
            "Transition cannot exceed {} inputs, found {} inputs",
            N::MAX_INPUTS,
            inputs.len()
        );
        // Ensure the number of outputs is within the allowed range.
        ensure!(
            outputs.len() <= N::MAX_OUTPUTS,
            "Transition cannot exceed {} outputs, found {} outputs",
            N::MAX_OUTPUTS,
            outputs.len()
        );

        // Prepare the input leaves.
        let input_leaves = inputs
            .iter()
            .enumerate()
            .map(|(index, input)| Ok::<_, Error>(input.to_transition_leaf(u8::try_from(index)?).to_bits_le()));
        // Prepare the output leaves.
        let output_leaves = outputs
            .iter()
            .enumerate()
            .map(|(index, output)| Ok(output.to_transition_leaf(u8::try_from(inputs.len() + index)?).to_bits_le()));
        // Compute the function tree.
        N::merkle_tree_bhp::<TRANSITION_DEPTH>(&input_leaves.chain(output_leaves).collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_transition_depth() {
        // Ensure the log2 relationship between depth and the maximum number of transition inputs & outputs.
        assert_eq!(2usize.pow(TRANSITION_DEPTH as u32), CurrentNetwork::MAX_INPUTS + CurrentNetwork::MAX_OUTPUTS);
    }
}
