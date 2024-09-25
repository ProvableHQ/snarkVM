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

impl<N: Network> Future<N> {
    /// Returns a value from the given path.
    pub fn find<A: Into<Access<N>> + Copy + Debug>(&self, path: &[A]) -> Result<Value<N>> {
        // Ensure the path is not empty.
        ensure!(!path.is_empty(), "Attempted to find an argument with an empty path.");

        // A helper enum to track the the argument.
        enum ArgumentRefType<'a, N: Network> {
            /// A plaintext type.
            Plaintext(&'a Plaintext<N>),
            /// A future.
            Future(&'a Future<N>),
        }

        // Initialize a value starting from the top-level.
        let mut value = ArgumentRefType::Future(self);

        // Iterate through the path to retrieve the value.
        for access in path.iter() {
            let access = (*access).into();
            match (value, access) {
                (ArgumentRefType::Plaintext(Plaintext::Struct(members, ..)), Access::Member(identifier)) => {
                    match members.get(&identifier) {
                        // Retrieve the member and update `value` for the next iteration.
                        Some(member) => value = ArgumentRefType::Plaintext(member),
                        // Halts if the member does not exist.
                        None => bail!("Failed to locate member '{identifier}'"),
                    }
                }
                (ArgumentRefType::Plaintext(Plaintext::Array(array, ..)), Access::Index(index)) => {
                    match array.get(*index as usize) {
                        // Retrieve the element and update `value` for the next iteration.
                        Some(element) => value = ArgumentRefType::Plaintext(element),
                        // Halts if the index is out of bounds.
                        None => bail!("Index '{index}' is out of bounds"),
                    }
                }
                (ArgumentRefType::Future(future), Access::Index(index)) => {
                    match future.arguments.get(*index as usize) {
                        // If the argument is a future, update `value` for the next iteration.
                        Some(Argument::Future(future)) => value = ArgumentRefType::Future(future),
                        // If the argument is a plaintext, update `value` for the next iteration.
                        Some(Argument::Plaintext(plaintext)) => value = ArgumentRefType::Plaintext(plaintext),
                        // Halts if the index is out of bounds.
                        None => bail!("Index '{index}' is out of bounds"),
                    }
                }
                _ => bail!("Invalid access `{access}`"),
            }
        }

        match value {
            ArgumentRefType::Plaintext(plaintext) => Ok(Value::Plaintext(plaintext.clone())),
            ArgumentRefType::Future(future) => Ok(Value::Future(future.clone())),
        }
    }
}
