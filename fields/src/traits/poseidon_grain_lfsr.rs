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

#![allow(dead_code)]

use crate::{FieldParameters, PrimeField};
use snarkvm_utilities::{vec::Vec, FromBits};

use anyhow::{bail, Result};

pub struct PoseidonGrainLFSR {
    pub field_size_in_bits: u64,
    pub state: [bool; 80],
    pub head: usize,
}

impl PoseidonGrainLFSR {
    pub fn new(
        is_sbox_an_inverse: bool,
        field_size_in_bits: u64,
        state_len: u64,
        num_full_rounds: u64,
        num_partial_rounds: u64,
    ) -> Self {
        let mut state = [false; 80];

        // b0, b1 describes the field
        state[1] = true;

        // b2, ..., b5 describes the S-BOX
        state[5] = is_sbox_an_inverse;

        // b6, ..., b17 are the binary representation of n (prime_num_bits)
        {
            let mut cur = field_size_in_bits;
            for i in (6..=17).rev() {
                state[i] = cur & 1 == 1;
                cur >>= 1;
            }
        }

        // b18, ..., b29 are the binary representation of t (state_len, rate + capacity)
        {
            let mut cur = state_len;
            for i in (18..=29).rev() {
                state[i] = cur & 1 == 1;
                cur >>= 1;
            }
        }

        // b30, ..., b39 are the binary representation of R_F (the number of full rounds)
        {
            let mut cur = num_full_rounds;
            for i in (30..=39).rev() {
                state[i] = cur & 1 == 1;
                cur >>= 1;
            }
        }

        // b40, ..., b49 are the binary representation of R_P (the number of partial rounds)
        {
            let mut cur = num_partial_rounds;
            for i in (40..=49).rev() {
                state[i] = cur & 1 == 1;
                cur >>= 1;
            }
        }

        // b50, ..., b79 are set to 1
        state[50..=79].copy_from_slice(&[true; 30]);

        // Initialize.
        let mut res = Self { field_size_in_bits, state, head: 0 };
        for _ in 0..160 {
            res.next_bit();
        }
        res
    }

    pub fn get_field_elements_rejection_sampling<F: PrimeField>(&mut self, num_elements: usize) -> Result<Vec<F>> {
        // Ensure the number of bits matches the modulus.
        if self.field_size_in_bits != F::Parameters::MODULUS_BITS as u64 {
            bail!("The number of bits in the field must match the modulus");
        }

        let mut output = Vec::with_capacity(num_elements);
        let mut bits = Vec::with_capacity(self.field_size_in_bits as usize);
        for _ in 0..num_elements {
            // Perform rejection sampling.
            loop {
                // Obtain `n` bits and make it most-significant-bit first.
                bits.extend(self.get_bits(self.field_size_in_bits as usize));
                bits.reverse();
                // Construct the number.
                let bigint = F::BigInteger::from_bits_le(&bits)?;
                bits.clear();
                // Ensure the number is in the field.
                if let Some(element) = F::from_bigint(bigint) {
                    output.push(element);
                    break;
                }
            }
        }
        Ok(output)
    }

    pub fn get_field_elements_mod_p<F: PrimeField>(&mut self, num_elems: usize) -> Result<Vec<F>> {
        // Ensure the number of bits matches the modulus.
        let num_bits = self.field_size_in_bits;
        if num_bits != F::Parameters::MODULUS_BITS as u64 {
            bail!("The number of bits in the field must match the modulus");
        }

        // Prepare reusable vectors for the intermediate bits and bytes.
        let mut bits = Vec::with_capacity(num_bits as usize);
        let mut bytes = Vec::with_capacity((num_bits as usize + 7) / 8);

        let mut output = Vec::with_capacity(num_elems);
        for _ in 0..num_elems {
            // Obtain `n` bits and make it most-significant-bit first.
            let bits_iter = self.get_bits(num_bits as usize);
            for bit in bits_iter {
                bits.push(bit);
            }
            bits.reverse();

            for byte in bits
                .chunks(8)
                .map(|chunk| {
                    let mut sum = chunk[0] as u8;
                    let mut cur = 1;
                    for i in chunk.iter().skip(1) {
                        cur *= 2;
                        sum += cur * (*i as u8);
                    }
                    sum
                })
                .rev()
            {
                bytes.push(byte);
            }

            output.push(F::from_bytes_be_mod_order(&bytes));

            // Clear the vectors of bits and bytes so they can be reused
            // in the next iteration.
            bits.clear();
            bytes.clear();
        }
        Ok(output)
    }
}

impl PoseidonGrainLFSR {
    #[inline]
    fn get_bits(&mut self, num_bits: usize) -> LFSRIter<'_> {
        LFSRIter { lfsr: self, num_bits, current_bit: 0 }
    }

    #[inline]
    fn next_bit(&mut self) -> bool {
        let next_bit = self.state[(self.head + 62) % 80]
            ^ self.state[(self.head + 51) % 80]
            ^ self.state[(self.head + 38) % 80]
            ^ self.state[(self.head + 23) % 80]
            ^ self.state[(self.head + 13) % 80]
            ^ self.state[self.head];
        self.state[self.head] = next_bit;
        self.head += 1;
        self.head %= 80;

        next_bit
    }
}

pub struct LFSRIter<'a> {
    lfsr: &'a mut PoseidonGrainLFSR,
    num_bits: usize,
    current_bit: usize,
}

impl<'a> Iterator for LFSRIter<'a> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_bit < self.num_bits {
            // Obtain the first bit
            let mut new_bit = self.lfsr.next_bit();

            // Loop until the first bit is true
            while !new_bit {
                // Discard the second bit
                let _ = self.lfsr.next_bit();
                // Obtain another first bit
                new_bit = self.lfsr.next_bit();
            }
            self.current_bit += 1;

            // Obtain the second bit
            Some(self.lfsr.next_bit())
        } else {
            None
        }
    }
}

impl<'a> ExactSizeIterator for LFSRIter<'a> {
    fn len(&self) -> usize {
        self.num_bits
    }
}
