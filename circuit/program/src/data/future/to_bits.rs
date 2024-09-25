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
use snarkvm_circuit_types::U8;

impl<A: Aleo> ToBits for Future<A> {
    type Boolean = Boolean<A>;

    /// Returns the circuit future as a list of **little-endian** bits.
    #[inline]
    fn write_bits_le(&self, vec: &mut Vec<Boolean<A>>) {
        // Write the bits for the program ID.
        let program_id_bits = self.program_id.to_bits_le();
        U16::constant(console::U16::new(program_id_bits.len() as u16)).write_bits_le(vec);
        vec.extend_from_slice(&program_id_bits);

        // Write the bits for the function name.
        let function_name_bits = self.function_name.to_bits_le();
        U16::constant(console::U16::new(function_name_bits.len() as u16)).write_bits_le(vec);
        vec.extend_from_slice(&function_name_bits);

        // Write the number of arguments.
        U8::constant(console::U8::new(self.arguments.len() as u8)).write_bits_le(vec);

        // Write the arguments.
        for argument in &self.arguments {
            let argument_bits = argument.to_bits_le();
            // Write the size of the argument.
            U16::constant(console::U16::new(argument_bits.len() as u16)).write_bits_le(vec);
            // Write the argument.
            vec.extend_from_slice(&argument_bits);
        }
    }

    /// Returns the circuit future as a list of **big-endian** bits.
    #[inline]
    fn write_bits_be(&self, vec: &mut Vec<Boolean<A>>) {
        // Write the bits for the program ID.
        let program_id_bits = self.program_id.to_bits_be();
        U16::constant(console::U16::new(program_id_bits.len() as u16)).write_bits_be(vec);
        vec.extend_from_slice(&program_id_bits);

        // Write the bits for the function name.
        let function_name_bits = self.function_name.to_bits_be();
        U16::constant(console::U16::new(function_name_bits.len() as u16)).write_bits_be(vec);
        vec.extend_from_slice(&function_name_bits);

        // Write the number of arguments.
        U8::constant(console::U8::new(self.arguments.len() as u8)).write_bits_be(vec);

        // Write the arguments.
        for argument in &self.arguments {
            let argument_bits = argument.to_bits_be();
            // Write the size of the argument.
            U16::constant(console::U16::new(argument_bits.len() as u16)).write_bits_be(vec);
            // Write the argument.
            vec.extend_from_slice(&argument_bits);
        }
    }
}
