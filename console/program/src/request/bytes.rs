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

impl<N: Network> FromBytes for Request<N> {
    /// Reads the request from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid request version"));
        }

        // Read the signer.
        let signer = FromBytes::read_le(&mut reader)?;
        // Read the network ID.
        let network_id = FromBytes::read_le(&mut reader)?;
        // Read the program ID.
        let program_id = FromBytes::read_le(&mut reader)?;
        // Read the function name.
        let function_name = FromBytes::read_le(&mut reader)?;

        // Read the number of inputs.
        let inputs_len = u16::read_le(&mut reader)?;
        // Read the input IDs.
        let input_ids = (0..inputs_len).map(|_| FromBytes::read_le(&mut reader)).collect::<Result<Vec<_>, _>>()?;
        // Read the inputs.
        let inputs = (0..inputs_len).map(|_| FromBytes::read_le(&mut reader)).collect::<Result<Vec<_>, _>>()?;

        // Read the signature.
        let signature = FromBytes::read_le(&mut reader)?;
        // Read the tag secret key.
        let sk_tag = FromBytes::read_le(&mut reader)?;
        // Read the transition view key.
        let tvk = FromBytes::read_le(&mut reader)?;
        // Read the transition commitment.
        let tcm = FromBytes::read_le(&mut reader)?;
        // Read the signer commitment.
        let scm = FromBytes::read_le(&mut reader)?;

        Ok(Self::from((
            signer,
            network_id,
            program_id,
            function_name,
            input_ids,
            inputs,
            signature,
            sk_tag,
            tvk,
            tcm,
            scm,
        )))
    }
}

impl<N: Network> ToBytes for Request<N> {
    /// Writes the request to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;

        // Write the signer.
        self.signer.write_le(&mut writer)?;
        // Write the network ID.
        self.network_id.write_le(&mut writer)?;
        // Write the program ID.
        self.program_id.write_le(&mut writer)?;
        // Write the function name.
        self.function_name.write_le(&mut writer)?;

        // Ensure the input IDs and inputs are the same length.
        if self.input_ids.len() != self.inputs.len() {
            return Err(error("Invalid request: mismatching number of input IDs and inputs"));
        }

        // Write the number of inputs.
        u16::try_from(self.input_ids.len())
            .or_halt_with::<N>("Request inputs length exceeds u16")
            .write_le(&mut writer)?;
        // Write the input IDs.
        for input_id in &self.input_ids {
            input_id.write_le(&mut writer)?;
        }
        // Write the inputs.
        for input in &self.inputs {
            input.write_le(&mut writer)?;
        }

        // Write the signature.
        self.signature.write_le(&mut writer)?;
        // Write the tag secret key.
        self.sk_tag.write_le(&mut writer)?;
        // Write the transition view key.
        self.tvk.write_le(&mut writer)?;
        // Write the transition commitment.
        self.tcm.write_le(&mut writer)?;
        // Write the signer commitment.
        self.scm.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() {
        let mut rng = TestRng::default();

        for expected in test_helpers::sample_requests(&mut rng).into_iter() {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, Request::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
