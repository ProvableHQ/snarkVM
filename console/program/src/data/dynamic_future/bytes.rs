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

impl<N: Network> FromBytes for DynamicFuture<N> {
    /// Reads in a future from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the program ID.
        let program_id_name = Identifier::<N>::from_field(&Field::read_le(&mut reader)?)
            .map_err(|e| error(format!("Failed to read program ID name: {e}")))?;
        let program_id_network = Identifier::<N>::from_field(&Field::read_le(&mut reader)?)
            .map_err(|e| error(format!("Failed to read program ID network: {e}")))?;
        let program_id = ProgramID::try_from((program_id_name, program_id_network))
            .map_err(|e| error(format!("Failed to read program ID: {e}")))?;

        // Read the function name.
        let function_name = Identifier::<N>::from_field(&Field::read_le(&mut reader)?)
            .map_err(|e| error(format!("Failed to read function name: {e}")))?;

        // Read the commitment.
        let commitment = Field::read_le(&mut reader)?;
        // Return the future.
        Ok(Self::new(program_id, function_name, commitment))
    }
}

impl<N: Network> ToBytes for DynamicFuture<N> {
    /// Writes a future to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the program ID.
        self.program_id
            .name()
            .to_field()
            .map_err(|e| error(format!("Failed to write program ID name: {e}")))?
            .write_le(&mut writer)?;
        self.program_id
            .network()
            .to_field()
            .map_err(|e| error(format!("Failed to write program ID network: {e}")))?
            .write_le(&mut writer)?;

        // Write the function name.
        self.function_name
            .to_field()
            .map_err(|e| error(format!("Failed to write function name: {e}")))?
            .write_le(&mut writer)?;

        // Write the commitment.
        self.commitment.write_le(&mut writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_bytes() -> Result<()> {
        // Check the future manually.
        let expected = DynamicFuture::<CurrentNetwork>::from_str(
            "{ program_id: credits.aleo, function_name: transfer, commitment: 0field }",
        )?;

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, DynamicFuture::read_le(&expected_bytes[..])?);

        Ok(())
    }
}
