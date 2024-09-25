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

impl<N: Network> EpochProgram<N> {
    /// Returns the R1CS assignment of the epoch program on the given inputs as a vector of bit sequences.
    pub fn to_leaves<A: circuit::Aleo<Network = N>>(&self, console_inputs: Vec<Value<N>>) -> Result<Vec<Vec<bool>>> {
        // Get the R1CS.
        let r1cs = self.to_r1cs::<A>(console_inputs)?;
        // Retrieve the public variables.
        let public_variables = r1cs.to_public_variables();
        // Retrieve the private variables.
        let private_variables = r1cs.to_private_variables();

        // Convert the public and private variables into leaves.
        let mut leaves = Vec::with_capacity(public_variables.len() + private_variables.len());
        // Append the public variables.
        for public_variable in public_variables {
            leaves.push(public_variable.value().to_bits_le());
        }
        // Append the private variables.
        for private_variable in private_variables {
            leaves.push(private_variable.value().to_bits_le());
        }

        // Pad the leaves to the next power of two.
        let Some(num_padded_leaves) = checked_next_power_of_n(leaves.len(), ARITY as usize) else {
            bail!("Integer overflow when computing the maximum number of leaves in the Merkle tree");
        };

        // Pad the leaves up to the next power of two.
        if leaves.len() < num_padded_leaves {
            leaves.resize(num_padded_leaves, vec![false; 254]);
        }

        Ok(leaves)
    }
}

/// Returns the next power of `n` that's greater than or equal to `base`.
/// Returns `None` for edge cases or in case of overflow.
fn checked_next_power_of_n(base: usize, n: usize) -> Option<usize> {
    if n <= 1 {
        return None;
    }

    let mut value = 1;
    while value < base {
        value = value.checked_mul(n)?;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_power_of_n() {
        // Test for regular input
        assert_eq!(checked_next_power_of_n(10, 2), Some(16));
        assert_eq!(checked_next_power_of_n(1, 3), Some(1));
        assert_eq!(checked_next_power_of_n(25, 5), Some(25));
        assert_eq!(checked_next_power_of_n(26, 5), Some(125));

        // Test for base being 0
        assert_eq!(checked_next_power_of_n(0, 2), Some(1));
    }

    #[test]
    fn test_next_power_of_n_edge_cases() {
        // n = 1 (should return None as it's an edge case)
        assert_eq!(checked_next_power_of_n(10, 1), None);

        // n = 0 (should return None as it's less than the minimum valid n)
        assert_eq!(checked_next_power_of_n(10, 0), None);
    }

    #[test]
    fn test_next_power_of_n_overflow() {
        // Test for potential overflow cases
        // Use a large base and n to test overflow, the exact values might need adjustments
        // depending on the system's usize limits
        assert_eq!(checked_next_power_of_n(usize::MAX, 2), None);
    }
}
