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

impl<N: Network> Plaintext<N> {
    /// Returns the plaintext member from the given path.
    pub fn find<A: Into<Access<N>> + Copy + Debug>(&self, path: &[A]) -> Result<Plaintext<N>> {
        // Ensure the path is not empty.
        ensure!(!path.is_empty(), "Attempted to find a member with an empty path.");

        match self {
            // Halts if the value is not a struct.
            Self::Literal(..) => bail!("'{self}' is not a struct"),
            // Retrieve the value of the member (from the value).
            Self::Struct(..) | Self::Array(..) => {
                // Initialize the plaintext starting from the top-level.
                let mut plaintext = self;

                // Iterate through the path to retrieve the value.
                for access in path.iter() {
                    let access = (*access).into();
                    match (plaintext, access) {
                        (Self::Struct(members, ..), Access::Member(identifier)) => {
                            match members.get(&identifier) {
                                // Retrieve the member and update `plaintext` for the next iteration.
                                Some(member) => plaintext = member,
                                // Halts if the member does not exist.
                                None => bail!("Failed to locate member '{identifier}' in '{self}'"),
                            }
                        }
                        (Self::Array(array, ..), Access::Index(index)) => {
                            match array.get(*index as usize) {
                                // Retrieve the element and update `plaintext` for the next iteration.
                                Some(element) => plaintext = element,
                                // Halts if the index is out of bounds.
                                None => bail!("Index '{index}' for '{self}' is out of bounds"),
                            }
                        }
                        _ => bail!("Invalid access `{access}` for `{plaintext}`"),
                    }
                }

                // Return the output.
                Ok(plaintext.clone())
            }
        }
    }
}
