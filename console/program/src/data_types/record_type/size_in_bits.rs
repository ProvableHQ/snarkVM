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

use snarkvm_console_types::{Address, Group, U8};

use crate::{Locator, StructType};

use super::*;

impl<N: Network> RecordType<N> {
    /// Returns the number of bits of a record type.
    pub fn size_in_bits<F0, F1>(&self, get_struct: &F0, get_external_struct: &F1) -> Result<usize>
    where
        F0: Fn(&Identifier<N>) -> Result<StructType<N>>,
        F1: Fn(&Locator<N>) -> Result<StructType<N>>,
    {
        // Initialize the counter.
        let mut size = 0usize;

        // Account for the owner visibility bit.
        size = size.checked_add(1).ok_or(anyhow!("`size_in_bits` overflowed"))?;

        // Account for the owner bits.
        size = size.checked_add(Address::<N>::size_in_bits()).ok_or(anyhow!("`size_in_bits` overflowed"))?;

        // Tally the data bits.
        let mut data_size = 0usize;
        for (identifier, entry_type) in &self.entries {
            // Account for the identifier bits,
            data_size = data_size
                .checked_add(identifier.size_in_bits() as usize)
                .ok_or(anyhow!("`size_in_bits` overflowed"))?;
            // Account for the entry mode bits.
            data_size = data_size.checked_add(2).ok_or(anyhow!("`size_in_bits` overflowed"))?;
            // Account for the entry data bits.
            data_size = data_size
                .checked_add(entry_type.plaintext_type().size_in_bits(get_struct, get_external_struct)?)
                .ok_or(anyhow!("`size_in_bits` overflowed"))?;
        }

        // Ensure the data length is less than 2^31 bits.
        if data_size >= (1 << 31) {
            bail!("Record data exceeds (1 << 31) bits")
        }

        // Account for the first 31 bits of the data length (as we know it is less than 2^31).
        size = size.checked_add(31).ok_or(anyhow!("`size_in_bits` overflowed"))?;

        // Account for the hiding bit.
        size = size.checked_add(1).ok_or(anyhow!("`size_in_bits` overflowed"))?;

        // Account for the data bits.
        size = size.checked_add(data_size).ok_or(anyhow!("`size_in_bits` overflowed"))?;

        // Account for the nonce bits.
        size = size.checked_add(Group::<N>::size_in_bits()).ok_or(anyhow!("`size_in_bits` overflowed"))?;

        // Account for the version bits.
        size = size.checked_add(U8::<N>::size_in_bits()).ok_or(anyhow!("`size_in_bits` overflowed"))?;

        Ok(size)
    }

    /// Returns the number of raw bits of a record type.
    pub fn size_in_bits_raw<F0, F1>(&self, get_struct: &F0, get_external_struct: &F1) -> Result<usize>
    where
        F0: Fn(&Identifier<N>) -> Result<StructType<N>>,
        F1: Fn(&Locator<N>) -> Result<StructType<N>>,
    {
        self.size_in_bits(get_struct, get_external_struct)
    }
}
