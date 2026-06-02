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

impl<N: Network> Plaintext<N> {
    /// Returns the plaintext member from the given path.
    pub fn find<A: Into<Access<N>> + Copy + Debug>(&self, path: &[A]) -> Result<Plaintext<N>> {
        // Ensure the path is not empty.
        ensure!(!path.is_empty(), "Attempted to find a member with an empty path.");
        // Walk the path and return an owned copy of the located value.
        self.find_cow(path).map(Cow::into_owned)
    }

    /// Walks the given path, returning the located value borrowed when possible and owned when a
    /// range access requires constructing a new sub-array.
    fn find_cow<A: Into<Access<N>> + Copy + Debug>(&self, path: &[A]) -> Result<Cow<'_, Plaintext<N>>> {
        // If the path is exhausted, return the current value.
        let Some((access, remaining)) = path.split_first() else {
            return Ok(Cow::Borrowed(self));
        };

        match (self, (*access).into()) {
            (Self::Struct(members, ..), Access::Member(identifier)) => match members.get(&identifier) {
                // Continue walking from the member.
                Some(member) => member.find_cow(remaining),
                // Halts if the member does not exist.
                None => bail!("Failed to locate member '{identifier}' in '{self}'"),
            },
            (Self::Array(array, ..), Access::Index(index)) => match array.get(*index as usize) {
                // Continue walking from the element.
                Some(element) => element.find_cow(remaining),
                // Halts if the index is out of bounds.
                None => bail!("Index '{index}' for '{self}' is out of bounds"),
            },
            (Self::Array(array, ..), Access::Range(start, end)) => match array.get(*start as usize..*end as usize) {
                // Construct the sub-array, then continue walking from it. As the sub-array is owned
                // locally, the remaining walk must return an owned value.
                Some(elements) => {
                    let sub_array = Self::Array(elements.to_vec(), Default::default());
                    Ok(Cow::Owned(sub_array.find_cow(remaining)?.into_owned()))
                }
                // Halts if the range is out of bounds.
                None => bail!("Range '{start}..{end}' for '{self}' is out of bounds"),
            },
            _ => bail!("Invalid access `{}` for `{self}`", (*access).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_find_range() -> Result<()> {
        let array = Plaintext::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8, 3u8, 4u8]")?;

        // A range returns the contiguous half-open sub-array `[start, end)`.
        assert_eq!(array.find(&[Access::Range(U32::new(1), U32::new(4))])?, Plaintext::from_str("[1u8, 2u8, 3u8]")?);
        // A range spanning the whole array returns a copy of the array.
        assert_eq!(array.find(&[Access::Range(U32::new(0), U32::new(5))])?, array);
        // A range of length one returns a single-element array (not the element itself).
        assert_eq!(array.find(&[Access::Range(U32::new(2), U32::new(3))])?, Plaintext::from_str("[2u8]")?);

        Ok(())
    }

    #[test]
    fn test_find_range_then_index() -> Result<()> {
        let array = Plaintext::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8, 3u8, 4u8]")?;
        // A range composes with a subsequent index into the resulting sub-array.
        assert_eq!(
            array.find(&[Access::Range(U32::new(1), U32::new(4)), Access::Index(U32::new(0))])?,
            Plaintext::from_str("1u8")?
        );
        assert_eq!(
            array.find(&[Access::Range(U32::new(1), U32::new(4)), Access::Index(U32::new(2))])?,
            Plaintext::from_str("3u8")?
        );
        Ok(())
    }

    #[test]
    fn test_find_range_out_of_bounds() -> Result<()> {
        let array = Plaintext::<CurrentNetwork>::from_str("[0u8, 1u8, 2u8, 3u8, 4u8]")?;
        // An end index past the length is out of bounds.
        assert!(array.find(&[Access::Range(U32::new(0), U32::new(6))]).is_err());
        // A reversed range (start > end) is out of bounds.
        assert!(array.find(&[Access::Range(U32::new(3), U32::new(1))]).is_err());
        // A range on a literal is an invalid access.
        let literal = Plaintext::<CurrentNetwork>::from_str("0u8")?;
        assert!(literal.find(&[Access::Range(U32::new(0), U32::new(1))]).is_err());
        Ok(())
    }
}
