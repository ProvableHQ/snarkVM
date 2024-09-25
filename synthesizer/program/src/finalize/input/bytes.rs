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

impl<N: Network> FromBytes for Input<N> {
    /// Reads the input from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let register = FromBytes::read_le(&mut reader)?;
        let finalize_type = FromBytes::read_le(&mut reader)?;

        // Ensure the register is not a register member.
        match matches!(register, Register::Locator(..)) {
            true => Ok(Self { register, finalize_type }),
            false => Err(error(format!("Input '{register}' cannot be a register member"))),
        }
    }
}

impl<N: Network> ToBytes for Input<N> {
    /// Writes the input to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Ensure the register is not a register member.
        if !matches!(self.register, Register::Locator(..)) {
            return Err(error(format!("Input '{}' cannot be a register member", self.register)));
        }
        self.register.write_le(&mut writer)?;
        self.finalize_type.write_le(&mut writer)
    }
}
