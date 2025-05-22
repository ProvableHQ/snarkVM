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

impl<A: Aleo> ToBits for DynamicFuture<A> {
    type Boolean = Boolean<A>;

    /// Returns the circuit future as a list of **little-endian** bits.
    #[inline]
    fn write_bits_le(&self, vec: &mut Vec<Boolean<A>>) {
        // Write the bits for the program ID.
        vec.extend_from_slice(&self.program_id.name().to_field().to_bits_le());
        vec.extend_from_slice(&self.program_id.network().to_bits_le());

        // Write the bits for the function name.
        vec.extend_from_slice(&self.function_name.to_field().to_bits_le());

        // Write the bits for the commitment.
        vec.extend_from_slice(&self.commitment.to_bits_le());
    }

    /// Returns the circuit future as a list of **big-endian** bits.
    #[inline]
    fn write_bits_be(&self, vec: &mut Vec<Boolean<A>>) {
        // Write the bits for the program ID.
        vec.extend_from_slice(&self.program_id.name().to_field().to_bits_be());
        vec.extend_from_slice(&self.program_id.network().to_bits_be());

        // Write the bits for the function name.
        vec.extend_from_slice(&self.function_name.to_field().to_bits_be());

        // Write the bits for the commitment.
        vec.extend_from_slice(&self.commitment.to_bits_be());
    }
}
