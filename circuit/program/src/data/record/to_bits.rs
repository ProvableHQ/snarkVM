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

impl<A: Aleo> ToBits for Record<A, Plaintext<A>> {
    type Boolean = Boolean<A>;

    /// Returns this data as a list of **little-endian** bits.
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        // Compute the data bits.
        let mut data_bits_le = vec![];
        for (identifier, entry) in &self.data {
            identifier.write_bits_le(&mut data_bits_le);
            entry.write_bits_le(&mut data_bits_le);
        }

        // Construct the record bits.
        self.owner.write_bits_le(vec);
        U32::constant(console::U32::new(data_bits_le.len() as u32)).write_bits_le(vec);
        vec.extend_from_slice(&data_bits_le);
        self.nonce.write_bits_le(vec);
    }

    /// Returns this data as a list of **big-endian** bits.
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        // Compute the data bits.
        let mut data_bits_be = vec![];
        for (identifier, entry) in &self.data {
            identifier.write_bits_be(&mut data_bits_be);
            entry.write_bits_be(&mut data_bits_be);
        }

        // Construct the record bits.
        self.owner.write_bits_be(vec);
        U32::constant(console::U32::new(data_bits_be.len() as u32)).write_bits_be(vec);
        vec.extend_from_slice(&data_bits_be);
        self.nonce.write_bits_be(vec);
    }
}

impl<A: Aleo> ToBits for Record<A, Ciphertext<A>> {
    type Boolean = Boolean<A>;

    /// Returns this data as a list of **little-endian** bits.
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        // Compute the data bits.
        let mut data_bits_le = vec![];
        for (identifier, entry) in &self.data {
            identifier.write_bits_le(&mut data_bits_le);
            entry.write_bits_le(&mut data_bits_le);
        }

        // Construct the record bits.
        self.owner.write_bits_le(vec);
        U32::constant(console::U32::new(data_bits_le.len() as u32)).write_bits_le(vec);
        vec.extend_from_slice(&data_bits_le);
        self.nonce.write_bits_le(vec);
    }

    /// Returns this data as a list of **big-endian** bits.
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        // Compute the data bits.
        let mut data_bits_be = vec![];
        for (identifier, entry) in &self.data {
            identifier.write_bits_be(&mut data_bits_be);
            entry.write_bits_be(&mut data_bits_be);
        }

        // Construct the record bits.
        self.owner.write_bits_be(vec);
        U32::constant(console::U32::new(data_bits_be.len() as u32)).write_bits_be(vec);
        vec.extend_from_slice(&data_bits_be);
        self.nonce.write_bits_be(vec);
    }
}
