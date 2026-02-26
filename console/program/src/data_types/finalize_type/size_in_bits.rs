// Copyright (c) 2019-2026 Provable Inc.
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

use crate::{Identifier, StructType};

use super::*;

impl<N: Network> FinalizeType<N> {
    /// Returns the number of bits of a finalize type.
    /// Note. The plaintext variant is assumed to be an argument of a `Future` and this does not have a "raw" serialization.
    pub fn future_size_in_bits<F0, F1, F2>(
        locator: &Locator<N>,
        get_struct: &F0,
        get_external_struct: &F1,
        get_future: &F2,
    ) -> Result<usize>
    where
        F0: Fn(&Identifier<N>) -> Result<StructType<N>>,
        F1: Fn(&Locator<N>) -> Result<StructType<N>>,
        F2: Fn(&Locator<N>) -> Result<Vec<FinalizeType<N>>>,
    {
        FinalizeType::Future(*locator).size_in_bits_internal(get_struct, get_external_struct, get_future, 0)
    }

    /// A helper function to determine the number of bits of a plaintext type, while tracking the depth of the data.
    /// Note. The plaintext variant is assumed to be an argument of a `Future` and thus does not have a "raw" serialization.
    pub fn size_in_bits_internal<F0, F1, F2>(
        &self,
        get_struct: &F0,
        get_external_struct: &F1,
        get_future: &F2,
        depth: usize,
    ) -> Result<usize>
    where
        F0: Fn(&Identifier<N>) -> Result<StructType<N>>,
        F1: Fn(&Locator<N>) -> Result<StructType<N>>,
        F2: Fn(&Locator<N>) -> Result<Vec<FinalizeType<N>>>,
    {
        // Ensure that the depth is within the maximum limit.
        ensure!(depth <= N::MAX_DATA_DEPTH, "Finalize type depth exceeds maximum limit: {}", N::MAX_DATA_DEPTH);

        match self {
            Self::Plaintext(plaintext_type) => {
                plaintext_type.size_in_bits_internal(get_struct, get_external_struct, depth)
            }
            Self::Future(locator) => {
                // Initialize the size in bits.
                let mut size = 0usize;

                // Account for the length of the program ID bits.
                size = size.checked_add(16).ok_or(anyhow!("`size_in_bits` overflowed"))?;

                // Account for the bits of the program ID.
                size = size
                    .checked_add(locator.name().size_in_bits() as usize)
                    .ok_or(anyhow!("`size_in_bits` overflowed"))?;
                size = size
                    .checked_add(locator.network().size_in_bits() as usize)
                    .ok_or(anyhow!("`size_in_bits` overflowed"))?;

                // Account for the length of the function name bits.
                size = size.checked_add(16).ok_or(anyhow!("`size_in_bits` overflowed"))?;

                // Account for the bits of the function name.
                size = size
                    .checked_add(locator.resource().size_in_bits() as usize)
                    .ok_or(anyhow!("`size_in_bits` overflowed"))?;

                // Look up the argument types of the future.
                let arguments = get_future(locator)?;

                // Account for the number of arguments.
                size = size.checked_add(8).ok_or(anyhow!("`size_in_bits` overflowed"))?;

                // Account for each of the arguments.
                for argument in &arguments {
                    // Account for the argument variant bit.
                    size = size.checked_add(1).ok_or(anyhow!("`size_in_bits` overflowed"))?;

                    // Calculate argument bits size.
                    let argument_size_in_bits =
                        argument.size_in_bits_internal(get_struct, get_external_struct, get_future, depth + 1)?;

                    // Account for the size of the argument bits
                    match argument_size_in_bits <= u16::MAX as usize {
                        true => {
                            // Account for the size of the argument bits (u16).
                            size = size.checked_add(16).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                        }
                        false => {
                            // Account for the size of the argument bits (u32).
                            size = size.checked_add(32).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                        }
                    }

                    // Account for the argument bits.
                    size = size.checked_add(argument_size_in_bits).ok_or(anyhow!("`size_in_bits` overflowed"))?;
                }

                Ok(size)
            }
        }
    }

    /// Returns the number of raw bits of a finalize type.
    pub fn future_size_in_bits_raw<F0, F1, F2>(
        locator: &Locator<N>,
        get_struct: &F0,
        get_external_struct: &F1,
        get_future: &F2,
    ) -> Result<usize>
    where
        F0: Fn(&Identifier<N>) -> Result<StructType<N>>,
        F1: Fn(&Locator<N>) -> Result<StructType<N>>,
        F2: Fn(&Locator<N>) -> Result<Vec<FinalizeType<N>>>,
    {
        Self::future_size_in_bits(locator, get_struct, get_external_struct, get_future)
    }
}
