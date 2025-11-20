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

impl<N: Network> FromBytes for Request<N> {
    /// Reads the request from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version and ensure that it is valid.
        let version = match u8::read_le(&mut reader)? {
            version @ (1 | 2) => version,
            _ => return Err(error("Invalid request version")),
        };
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
        // Ensure the number of inputs is within bounds.
        if inputs_len as usize > N::MAX_INPUTS {
            return Err(error(format!(
                "Request (from 'read_le') has too many inputs ({} > {})",
                inputs_len,
                N::MAX_INPUTS
            )));
        }
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

        // Read the optional caller input IDs and caller inputs.
        let (caller_input_ids, caller_inputs) = match version {
            1 => (None, None),
            2 => (
                Some((0..inputs_len).map(|_| FromBytes::read_le(&mut reader)).collect::<Result<Vec<_>, _>>()?),
                Some((0..inputs_len).map(|_| FromBytes::read_le(&mut reader)).collect::<Result<Vec<_>, _>>()?),
            ),
            _ => return Err(error("Invalid request version")),
        };
        // Read the optional caller Request.
        let caller_request = match version {
            1 => None,
            2 => Some(Box::new(FromBytes::read_le(&mut reader)?)),
            _ => return Err(error("Invalid request version")),
        };
        // Read the optional caller output types.
        let caller_output_types = match version {
            1 => None,
            2 => {
                let num_caller_output_types = u16::read_le(&mut reader)?;
                Some((0..num_caller_output_types).map(|_| FromBytes::read_le(&mut reader)).collect::<Result<Vec<_>, _>>()?)
            },
            _ => return Err(error("Invalid request version")),
        };

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
            caller_input_ids,
            caller_inputs,
            caller_output_types,
            caller_request,
        )))
    }
}

impl<N: Network> ToBytes for Request<N> {
    /// Writes the request to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        match self.caller_input_ids.is_some() {
            false => 1u8.write_le(&mut writer)?,
            true => 2u8.write_le(&mut writer)?,
        }
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
        self.scm.write_le(&mut writer)?;

        // Ensure the number of caller input IDs and caller inputs are consistent.
        match (&self.caller_input_ids, &self.caller_inputs) {
            (Some(caller_input_ids), Some(caller_inputs)) => {
                // Ensure that the lengths match.
                if caller_input_ids.len() != caller_inputs.len() {
                    return Err(error("Invalid request: mismatching number of caller input IDs and caller inputs"));
                }
                // Ensre that the number of caller inputs match the number of inputs.
                if caller_inputs.len() != self.inputs.len() {
                    return Err(error("Invalid request: mismatching number of caller inputs and inputs"));
                }
                // Write the caller input IDs.
                for caller_input_id in caller_input_ids {
                    caller_input_id.write_le(&mut writer)?;
                }
                // Write the caller inputs.
                for caller_input in caller_inputs {
                    caller_input.write_le(&mut writer)?;
                }
            }
            (None, Some(_)) | (Some(_), None) => {
                return Err(error(
                    "Invalid request: caller input IDs and caller inputs must both be present or both be absent",
                ));
            }
            _ => {}
        }
        // Write the optional caller output types.
        if let Some(caller_output_types) = &self.caller_output_types {
            u16::try_from(caller_output_types.len())
                .or_halt_with::<N>("Caller output types length exceeds u16")
                .write_le(&mut writer)?;
            for caller_output_type in caller_output_types {
                caller_output_type.write_le(&mut writer)?;
            }
        }
        // Write the optional caller Request.
        if let Some(caller_request) = &self.caller_request {
            caller_request.write_le(&mut writer)?;
        }

        Ok(())
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
