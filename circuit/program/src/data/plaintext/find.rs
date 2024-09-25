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

impl<A: Aleo> Plaintext<A> {
    /// Returns the plaintext member from the given path.
    pub fn find<A0: Into<Access<A>> + Clone + Debug>(&self, path: &[A0]) -> Result<Plaintext<A>> {
        // Ensure the path is not empty.
        if path.is_empty() {
            A::halt("Attempted to find member with an empty path.")
        }

        match self {
            // Halts if the value is not a struct or an array.
            Self::Literal(..) => A::halt("A literal is not a struct or an array"),
            // Retrieve the value of the member (from the value).
            Self::Struct(..) | Self::Array(..) => {
                // Initialize the plaintext starting from the top-level.
                let mut plaintext = self;

                // Iterate through the path to retrieve the value.
                for access in path.iter() {
                    let access = access.clone().into();
                    match (plaintext, &access) {
                        (Self::Struct(members, ..), Access::Member(identifier)) => {
                            match members.get(identifier) {
                                // Retrieve the member and update `plaintext` for the next iteration.
                                Some(member) => plaintext = member,
                                // Halts if the member does not exist.
                                None => bail!("Failed to locate member '{identifier}'"),
                            }
                        }
                        (Self::Array(array, ..), Access::Index(index)) => {
                            let index = match index.eject_mode() {
                                Mode::Constant => index.eject_value(),
                                _ => bail!("'{index}' must be a constant"),
                            };
                            match array.get(*index as usize) {
                                // Retrieve the element and update `plaintext` for the next iteration.
                                Some(element) => plaintext = element,
                                // Halts if the element does not exist.
                                None => bail!("Failed to locate element '{index}'"),
                            }
                        }
                        _ => bail!("Invalid access `{access}``"),
                    }
                }

                // Return the output.
                Ok(plaintext.clone())
            }
        }
    }
}
