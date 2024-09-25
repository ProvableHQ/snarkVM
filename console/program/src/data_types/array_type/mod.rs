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

mod bytes;
mod parse;
pub(crate) mod serialize;

use crate::{PlaintextType, U32};
use snarkvm_console_network::prelude::*;

use core::fmt::{Debug, Display};

/// An `ArrayType` defines the type and size of an array.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ArrayType<N: Network> {
    /// The element type.
    element_type: Box<PlaintextType<N>>,
    /// The length of the array.
    length: U32<N>,
}

impl<N: Network> ArrayType<N> {
    /// Initializes a new multi-dimensional array type.
    /// Note that the dimensions must be specified from the outermost to the innermost.
    pub fn new(plaintext_type: PlaintextType<N>, mut dimensions: Vec<U32<N>>) -> Result<Self> {
        // Check that the number of dimensions are valid.
        ensure!(!dimensions.is_empty(), "An array must have at least one dimension");
        ensure!(dimensions.len() <= N::MAX_DATA_DEPTH, "An array can have at most {} dimensions", N::MAX_DATA_DEPTH);
        // Check that each dimension is valid.
        for length in &dimensions {
            ensure!(**length as usize >= N::MIN_ARRAY_ELEMENTS, "An array must have {} element", N::MIN_ARRAY_ELEMENTS);
            ensure!(
                **length as usize <= N::MAX_ARRAY_ELEMENTS,
                "An array can contain {} elements",
                N::MAX_ARRAY_ELEMENTS
            );
        }
        // Construct the array type.
        // Note that this `unwrap` is safe because we have already checked that the number of dimensions is greater than zero.
        let array_type = Self { element_type: Box::new(plaintext_type), length: dimensions.pop().unwrap() };
        Ok(dimensions.into_iter().rev().fold(array_type, |array_type, dimension| Self {
            element_type: Box::new(PlaintextType::Array(array_type)),
            length: dimension,
        }))
    }
}

impl<N: Network> ArrayType<N> {
    /// Returns the next element type.
    /// In the case of a one-dimensional array, this will return the element type of the array.
    /// In the case of a multi-dimensional array, this will return the element type of the **outermost** array.
    pub const fn next_element_type(&self) -> &PlaintextType<N> {
        &self.element_type
    }

    /// Returns the base element type.
    /// In the case of a one-dimensional array, this will return the element type of the array.
    /// In the case of a multi-dimensional array, this will return the element type of the **innermost** array.
    pub fn base_element_type(&self) -> &PlaintextType<N> {
        let mut element_type = self.next_element_type();
        // Note that this `while` loop must terminate because the number of dimensions of `ArrayType` is checked to be less then N::MAX_DATA_DEPTH.
        while let PlaintextType::Array(array_type) = element_type {
            element_type = array_type.next_element_type();
        }
        element_type
    }

    /// Returns `true` if the array is empty.
    pub fn is_empty(&self) -> bool {
        self.length.is_zero()
    }

    /// Returns the length of the array.
    pub const fn length(&self) -> &U32<N> {
        &self.length
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Identifier, LiteralType};
    use snarkvm_console_network::MainnetV0;

    use core::str::FromStr;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_array_type() -> Result<()> {
        // Test literal array types.
        let array = ArrayType::<CurrentNetwork>::from_str("[field; 4u32]")?;
        assert_eq!(array, ArrayType::<CurrentNetwork>::new(PlaintextType::from_str("field")?, vec![U32::new(4)])?);
        assert_eq!(
            array.to_bytes_le()?,
            ArrayType::<CurrentNetwork>::from_bytes_le(&array.to_bytes_le()?)?.to_bytes_le()?
        );
        assert_eq!(array.next_element_type(), &PlaintextType::Literal(LiteralType::Field));
        assert_eq!(array.length(), &U32::new(4));
        assert!(!array.is_empty());

        // Test struct array types.
        let array = ArrayType::<CurrentNetwork>::from_str("[foo; 1u32]")?;
        assert_eq!(array, ArrayType::<CurrentNetwork>::new(PlaintextType::from_str("foo")?, vec![U32::new(1)])?);
        assert_eq!(
            array.to_bytes_le()?,
            ArrayType::<CurrentNetwork>::from_bytes_le(&array.to_bytes_le()?)?.to_bytes_le()?
        );
        assert_eq!(array.next_element_type(), &PlaintextType::Struct(Identifier::from_str("foo")?));
        assert_eq!(array.length(), &U32::new(1));
        assert!(!array.is_empty());

        // Test array type with maximum length.
        let array = ArrayType::<CurrentNetwork>::from_str("[scalar; 32u32]")?;
        assert_eq!(array, ArrayType::<CurrentNetwork>::new(PlaintextType::from_str("scalar")?, vec![U32::new(32)])?);
        assert_eq!(
            array.to_bytes_le()?,
            ArrayType::<CurrentNetwork>::from_bytes_le(&array.to_bytes_le()?)?.to_bytes_le()?
        );
        assert_eq!(array.next_element_type(), &PlaintextType::Literal(LiteralType::Scalar));
        assert_eq!(array.length(), &U32::new(32));
        assert!(!array.is_empty());

        // Test multi-dimensional array types.
        let array = ArrayType::<CurrentNetwork>::from_str("[[field; 2u32]; 3u32]")?;
        assert_eq!(
            array,
            ArrayType::<CurrentNetwork>::new(
                PlaintextType::Array(ArrayType::<CurrentNetwork>::new(PlaintextType::from_str("field")?, vec![
                    U32::new(2)
                ])?),
                vec![U32::new(3)]
            )?
        );
        assert_eq!(
            array.to_bytes_le()?,
            ArrayType::<CurrentNetwork>::from_bytes_le(&array.to_bytes_le()?)?.to_bytes_le()?
        );
        assert_eq!(array.to_string(), "[[field; 2u32]; 3u32]");
        assert_eq!(
            array.next_element_type(),
            &PlaintextType::Array(ArrayType::<CurrentNetwork>::new(PlaintextType::Literal(LiteralType::Field), vec![
                U32::new(2)
            ])?)
        );
        assert_eq!(array.length(), &U32::new(3));
        assert!(!array.is_empty());

        Ok(())
    }

    #[test]
    fn test_array_type_fails() {
        let type_ = ArrayType::<CurrentNetwork>::from_str("[field; 0u32]");
        assert!(type_.is_err());

        let type_ = ArrayType::<CurrentNetwork>::from_str("[field; 4294967296u32]");
        assert!(type_.is_err());

        let type_ = ArrayType::<CurrentNetwork>::from_str("[foo; -1i32]");
        assert!(type_.is_err());

        let type_ = ArrayType::<CurrentNetwork>::from_str("[foo; 1u8]");
        assert!(type_.is_err());
    }
}
