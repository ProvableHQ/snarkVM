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

impl<N: Network> FromBytes for Execution<N> {
    /// Reads the execution from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid execution version"));
        }
        // Read the number of transitions.
        let num_transitions = u8::read_le(&mut reader)?;
        // Ensure the number of transitions is nonzero.
        if num_transitions == 0 {
            return Err(error("Execution (from 'read_le') has no transitions"));
        }
        // Read the transitions.
        let transitions =
            (0..num_transitions).map(|_| Transition::read_le(&mut reader)).collect::<IoResult<Vec<_>>>()?;
        // Read the global state root.
        let global_state_root = N::StateRoot::read_le(&mut reader)?;
        // Read the proof variant.
        let proof_variant = u8::read_le(&mut reader)?;
        // Read the proof.
        let proof = match proof_variant {
            0 => None,
            1 => Some(Proof::read_le(&mut reader)?),
            _ => return Err(error(format!("Invalid proof variant '{proof_variant}'"))),
        };
        // Return the new `Execution` instance.
        Self::from(transitions.into_iter(), global_state_root, proof).map_err(|e| error(e.to_string()))
    }
}

impl<N: Network> ToBytes for Execution<N> {
    /// Writes the execution to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;
        // Write the number of transitions.
        (u8::try_from(self.transitions.len()).map_err(|e| error(e.to_string()))?).write_le(&mut writer)?;
        // Write the transitions.
        for transition in self.transitions.values() {
            transition.write_le(&mut writer)?;
        }
        // Write the global state root.
        self.global_state_root.write_le(&mut writer)?;
        // Write the proof.
        match self.proof {
            None => 0u8.write_le(&mut writer)?,
            Some(ref proof) => {
                1u8.write_le(&mut writer)?;
                proof.write_le(&mut writer)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() -> Result<()> {
        let rng = &mut TestRng::default();

        // Construct a new execution.
        let expected = crate::transaction::execution::test_helpers::sample_execution(rng);

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, Execution::read_le(&expected_bytes[..])?);
        Ok(())
    }
}
