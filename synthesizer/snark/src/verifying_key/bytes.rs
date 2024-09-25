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

impl<N: Network> FromBytes for VerifyingKey<N> {
    /// Reads the verifying key from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid verifying key version"));
        }
        // Read the verifying key.
        let verifying_key = Arc::new(FromBytes::read_le(&mut reader)?);
        // Read the number of variables.
        let num_variables = u64::read_le(&mut reader)?;
        // Return the verifying key.
        Ok(Self { verifying_key, num_variables })
    }
}

impl<N: Network> ToBytes for VerifyingKey<N> {
    /// Writes the verifying key to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;
        // Write the verifying key.
        self.verifying_key.write_le(&mut writer)?;
        // Write the number of variables.
        self.num_variables.write_le(&mut writer)
    }
}
