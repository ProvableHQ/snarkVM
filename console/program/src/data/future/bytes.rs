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

impl<N: Network> FromBytes for Future<N> {
    /// Reads in a future from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the program ID.
        let program_id = ProgramID::read_le(&mut reader)?;
        // Read the function name.
        let function_name = Identifier::<N>::read_le(&mut reader)?;
        // Read the number of arguments to the future.
        let num_arguments = u8::read_le(&mut reader)? as usize;
        if num_arguments > N::MAX_INPUTS {
            return Err(error("Failed to read future: too many arguments"));
        };
        // Read the arguments.
        let mut arguments = Vec::with_capacity(num_arguments);
        for _ in 0..num_arguments {
            // Read the argument (in 2 steps to prevent infinite recursion).
            let num_bytes = u16::read_le(&mut reader)?;
            // Read the argument bytes.
            let mut bytes = Vec::new();
            (&mut reader).take(num_bytes as u64).read_to_end(&mut bytes)?;
            // Recover the argument.
            let entry = Argument::read_le(&mut bytes.as_slice())?;
            // Add the argument.
            arguments.push(entry);
        }
        // Return the future.
        Ok(Self::new(program_id, function_name, arguments))
    }
}

impl<N: Network> ToBytes for Future<N> {
    /// Writes a future to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the program ID.
        self.program_id.write_le(&mut writer)?;
        // Write the function name.
        self.function_name.write_le(&mut writer)?;
        // Write the number of arguments.
        if self.arguments.len() > N::MAX_INPUTS {
            return Err(error("Failed to write future: too many arguments"));
        };
        u8::try_from(self.arguments.len()).map_err(error)?.write_le(&mut writer)?;
        // Write each argument.
        for argument in &self.arguments {
            // Write the argument (performed in 2 steps to prevent infinite recursion).
            let bytes = argument.to_bytes_le().map_err(error)?;
            // Write the number of bytes.
            u16::try_from(bytes.len()).map_err(error)?.write_le(&mut writer)?;
            // Write the bytes.
            bytes.write_le(&mut writer)?;
        }
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
        let expected =
            Future::<CurrentNetwork>::from_str("{ program_id: credits.aleo, function_name: transfer, arguments: [] }")?;

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, Future::read_le(&expected_bytes[..])?);

        Ok(())
    }
}
