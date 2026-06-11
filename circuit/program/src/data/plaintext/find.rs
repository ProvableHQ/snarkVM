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

use super::*;

use std::borrow::Cow;

impl<A: Aleo> Plaintext<A> {
    /// Returns the plaintext member from the given path.
    pub fn find<A0: Into<Access<A>> + Clone + Debug>(&self, path: &[A0]) -> Result<Plaintext<A>> {
        // Ensure the path is not empty.
        if path.is_empty() {
            A::halt("Attempted to find member with an empty path.")
        }
        // Walk the path and return an owned copy of the located value.
        self.find_cow(path).map(Cow::into_owned)
    }

    /// Walks the given path, returning the located value borrowed when possible and owned when a
    /// range access requires constructing a new sub-array.
    fn find_cow<A0: Into<Access<A>> + Clone + Debug>(&self, path: &[A0]) -> Result<Cow<'_, Plaintext<A>>> {
        // If the path is exhausted, return the current value.
        let Some((access, remaining)) = path.split_first() else {
            return Ok(Cow::Borrowed(self));
        };

        match (self, access.clone().into()) {
            (Self::Struct(members, ..), Access::Member(identifier)) => match members.get(&identifier) {
                // Continue walking from the member.
                Some(member) => member.find_cow(remaining),
                // Halts if the member does not exist.
                None => bail!("Failed to locate member '{identifier}'"),
            },
            (Self::Array(array, ..), Access::Index(index)) => {
                // The index must be a constant, as array indices are resolved at synthesis time.
                let index = match index.eject_mode() {
                    Mode::Constant => index.eject_value(),
                    _ => bail!("'{index}' must be a constant"),
                };
                match array.get(*index as usize) {
                    // Continue walking from the element.
                    Some(element) => element.find_cow(remaining),
                    // Halts if the element does not exist.
                    None => bail!("Failed to locate element '{index}'"),
                }
            }
            (Self::Array(array, ..), Access::Range(start, end)) => {
                // The bounds must be constants, as array ranges are resolved at synthesis time.
                let start = match start.eject_mode() {
                    Mode::Constant => start.eject_value(),
                    _ => bail!("'{start}' must be a constant"),
                };
                let end = match end.eject_mode() {
                    Mode::Constant => end.eject_value(),
                    _ => bail!("'{end}' must be a constant"),
                };
                match array.get(*start as usize..*end as usize) {
                    // Construct the sub-array, then continue walking from it. As the sub-array is
                    // owned locally, the remaining walk must return an owned value.
                    Some(elements) => {
                        let sub_array = Self::Array(elements.to_vec(), Default::default());
                        Ok(Cow::Owned(sub_array.find_cow(remaining)?.into_owned()))
                    }
                    // Halts if the range is out of bounds.
                    None => bail!("Range '{start}..{end}' is out of bounds"),
                }
            }
            _ => bail!("Invalid access `{}`", access.clone().into()),
        }
    }
}
