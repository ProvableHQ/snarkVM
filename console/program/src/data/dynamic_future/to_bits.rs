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

impl<N: Network> ToBits for DynamicFuture<N> {
    /// Returns the future as a list of **little-endian** bits.
    #[inline]
    fn write_bits_le(&self, vec: &mut Vec<bool>) {
        // Write the bits for the program ID.
        let program_id_bits = self.program_id.to_bits_le();
        u16::try_from(program_id_bits.len()).or_halt_with::<N>("Program ID exceeds u16::MAX bits").write_bits_le(vec);
        vec.extend_from_slice(&program_id_bits);

        // Write the bits for the function name.
        let function_name_bits = self.function_name.to_bits_le();
        u16::try_from(function_name_bits.len())
            .or_halt_with::<N>("Function name exceeds u16::MAX bits")
            .write_bits_le(vec);
        vec.extend_from_slice(&function_name_bits);

        // Write the commitment.
        let commitment_bits = self.commitment.to_bits_le();
        u16::try_from(commitment_bits.len()).or_halt_with::<N>("Commitment exceeds u16::MAX bits").write_bits_le(vec);
        vec.extend_from_slice(&commitment_bits);
    }

    /// Returns the future as a list of **big-endian** bits.
    #[inline]
    fn write_bits_be(&self, vec: &mut Vec<bool>) {
        // Write the bits for the program ID.
        let program_id_bits = self.program_id.to_bits_be();
        u16::try_from(program_id_bits.len()).or_halt_with::<N>("Program ID exceeds u16::MAX bits").write_bits_be(vec);
        vec.extend_from_slice(&program_id_bits);

        // Write the bits for the function name.
        let function_name_bits = self.function_name.to_bits_be();
        u16::try_from(function_name_bits.len())
            .or_halt_with::<N>("Function name exceeds u16::MAX bits")
            .write_bits_be(vec);
        vec.extend_from_slice(&function_name_bits);

        // Write the bits for the commitment.
        let commitment_bits = self.commitment.to_bits_be();
        u16::try_from(commitment_bits.len()).or_halt_with::<N>("Commitment exceeds u16::MAX bits").write_bits_be(vec);
        vec.extend_from_slice(&commitment_bits);
    }
}
