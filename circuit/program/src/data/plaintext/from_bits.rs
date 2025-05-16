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

use super::*;

impl<A: Aleo> FromBits for Plaintext<A> {
    type Boolean = Boolean<A>;

    /// Initializes a new plaintext from a list of little-endian bits *without* trailing zeros.
    fn from_bits_le(bits_le: &[Boolean<A>]) -> Self {
        Self::from_bits_le_internal(bits_le, 0)
    }

    /// Initializes a new plaintext from a list of big-endian bits *without* trailing zeros.
    fn from_bits_be(bits_be: &[Boolean<A>]) -> Self {
        Self::from_bits_be_internal(bits_be, 0)
    }
}

impl<A: Aleo> Plaintext<A> {
    /// Initializes a new plaintext from a list of little-endian bits *without* trailing zeros, while tracking the depth of the data.
    fn from_bits_le_internal(bits_le: &[Boolean<A>], depth: usize) -> Self {
        // Ensure that the depth is within the maximum limit.
        if depth > <A::Network as console::Network>::MAX_DATA_DEPTH {
            A::halt(format!(
                "Plaintext depth exceeds maximum limit: {}",
                <A::Network as console::Network>::MAX_DATA_DEPTH
            ))
        }

        let bits = bits_le;

        // The starting index used to create subsequent subslices of the `bits` slice.
        let mut index = 0;

        // Helper function to get the next n bits as a slice.
        let mut next_bits = |n: usize| -> &[Boolean<A>] {
            // Safely procure a subslice with the length `n` starting at `index`.
            let subslice = bits.get(index..index + n);
            // Check if the range is within bounds.
            if let Some(next_bits) = subslice {
                // Move the starting index.
                index += n;
                // Return the subslice.
                next_bits
            } else {
                A::halt("Insufficient bits.")
            }
        };

        let mut variant = next_bits(2).iter().map(|b| b.eject_value());
        let variant1 = variant.next().unwrap();
        let variant2 = variant.next().unwrap();
        let variant = [variant1, variant2];

        // Literal
        if variant == [false, false] {
            let literal_variant = U8::from_bits_le(next_bits(8));
            let literal_size = U16::from_bits_le(next_bits(16)).eject_value();
            let literal = Literal::from_bits_le(&literal_variant, next_bits(*literal_size as usize));

            // Cache the plaintext bits, and return the literal.
            Self::Literal(literal, OnceCell::with_value(bits_le.to_vec()))
        }
        // Struct
        else if variant == [false, true] {
            let num_members = U8::from_bits_le(next_bits(8)).eject_value();

            let mut members = IndexMap::with_capacity(*num_members as usize);
            for _ in 0..*num_members {
                let identifier_size = U8::from_bits_le(next_bits(8)).eject_value();
                let identifier = Identifier::from_bits_le(next_bits(*identifier_size as usize));

                let member_size = U16::from_bits_le(next_bits(16)).eject_value();
                let value = Plaintext::from_bits_le_internal(next_bits(*member_size as usize), depth + 1);

                members.insert(identifier, value);
            }

            // Cache the plaintext bits, and return the struct.
            Self::Struct(members, OnceCell::with_value(bits_le.to_vec()))
        }
        // Array
        else if variant == [true, false] {
            let num_elements = U32::from_bits_le(next_bits(32)).eject_value();

            let mut elements = Vec::with_capacity(*num_elements as usize);
            for _ in 0..*num_elements {
                let element_size = U16::from_bits_le(next_bits(16)).eject_value();
                let value = Plaintext::from_bits_le_internal(next_bits(*element_size as usize), depth + 1);

                elements.push(value);
            }

            // Cache the plaintext bits, and return the array.
            Self::Array(elements, OnceCell::with_value(bits_le.to_vec()))
        }
        // Unknown variant.
        else {
            A::halt("Unknown plaintext variant.")
        }
    }

    /// Initializes a new plaintext from a list of big-endian bits *without* trailing zeros, while tracking the depth of the data.
    fn from_bits_be_internal(bits_be: &[Boolean<A>], depth: usize) -> Self {
        // Ensure that the depth is within the maximum limit.
        if depth > <A::Network as console::Network>::MAX_DATA_DEPTH {
            A::halt(format!(
                "Plaintext depth exceeds maximum limit: {}",
                <A::Network as console::Network>::MAX_DATA_DEPTH
            ))
        }

        let bits = bits_be;

        // The starting index used to create subsequent subslices of the `bits` slice.
        let mut index = 0;

        // Helper function to get the next n bits as a slice.
        let mut next_bits = |n: usize| -> &[Boolean<A>] {
            // Safely procure a subslice with the length `n` starting at `index`.
            let subslice = bits.get(index..index + n);
            // Check if the range is within bounds.
            if let Some(next_bits) = subslice {
                // Move the starting index.
                index += n;
                // Return the subslice.
                next_bits
            } else {
                A::halt("Insufficient bits.")
            }
        };

        let mut variant = next_bits(2).iter().map(|b| b.eject_value());
        let variant1 = variant.next().unwrap();
        let variant2 = variant.next().unwrap();
        let variant = [variant1, variant2];

        // Literal
        if variant == [false, false] {
            let literal_variant = U8::from_bits_be(next_bits(8));
            let literal_size = U16::from_bits_be(next_bits(16)).eject_value();
            let literal = Literal::from_bits_be(&literal_variant, next_bits(*literal_size as usize));

            // Cache the plaintext bits, and return the literal.
            Self::Literal(literal, OnceCell::with_value(bits_be.to_vec()))
        }
        // Struct
        else if variant == [false, true] {
            let num_members = U8::from_bits_be(next_bits(8)).eject_value();

            let mut members = IndexMap::with_capacity(*num_members as usize);
            for _ in 0..*num_members {
                let identifier_size = U8::from_bits_be(next_bits(8)).eject_value();
                let identifier = Identifier::from_bits_be(next_bits(*identifier_size as usize));

                let member_size = U16::from_bits_be(next_bits(16)).eject_value();
                let value = Plaintext::from_bits_be_internal(next_bits(*member_size as usize), depth + 1);

                members.insert(identifier, value);
            }

            // Cache the plaintext bits, and return the struct.
            Self::Struct(members, OnceCell::with_value(bits_be.to_vec()))
        }
        // Array
        else if variant == [true, false] {
            let num_elements = U32::from_bits_be(next_bits(32)).eject_value();

            let mut elements = Vec::with_capacity(*num_elements as usize);
            for _ in 0..*num_elements {
                let element_size = U16::from_bits_be(next_bits(16)).eject_value();
                let value = Plaintext::from_bits_be_internal(next_bits(*element_size as usize), depth + 1);

                elements.push(value);
            }

            // Cache the plaintext bits, and return the array.
            Self::Array(elements, OnceCell::with_value(bits_be.to_vec()))
        }
        // Unknown variant.
        else {
            A::halt("Unknown plaintext variant.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_root::network::{MainnetV0, Network};
    use snarkvm_circuit_network::AleoV0;

    type CurrentAleo = AleoV0;
    type CurrentNetwork = MainnetV0;

    // A helper function to get the depth of the plaintext.
    fn get_depth(plaintext: &Plaintext<CurrentAleo>) -> usize {
        match plaintext {
            Plaintext::Literal(_, _) => 0,
            Plaintext::Struct(members, _) => members.values().map(get_depth).max().unwrap_or(0) + 1,
            Plaintext::Array(elements, _) => elements.iter().map(get_depth).max().unwrap_or(0) + 1,
        }
    }

    #[test]
    fn test_deeply_nested_plaintext_bits_le() {
        // Creates a nested array-like `Plaintext` structure by wrapping a root value `depth` times.
        fn create_nested_array(mode: Mode, depth: usize, root: impl Display) -> Vec<Boolean<CurrentAleo>> {
            // Start from the innermost value.
            let mut result = console::Plaintext::<CurrentNetwork>::from_str(&root.to_string()).unwrap().to_bits_le();
            // Reverse the bytes.
            result.reverse();
            // Build up the structure in reverse.
            for _ in 0..depth {
                // Write the size of the object in bits in reverse.
                let mut length = (u16::try_from(result.len()).unwrap()).to_bits_le();
                length.reverse();
                result.extend(length);
                // Write the number of elements in the array in reverse.
                let mut num_elements = 1u32.to_bits_le();
                num_elements.reverse();
                result.extend(num_elements);
                // Write the plaintext variant in reverse.
                result.extend([false, true]);
            }
            // Reverse the result to get the correct order.
            result.reverse();
            // Transform the bits into a vector of booleans.
            let result: Vec<Boolean<CurrentAleo>> = result.into_iter().map(|input| Boolean::new(mode, input)).collect();
            result
        }

        // Creates a nested struct-like `Plaintext` structure by wrapping a root value `depth` times.
        fn create_nested_struct(mode: Mode, depth: usize, root: impl Display) -> Vec<Boolean<CurrentAleo>> {
            // Start from the innermost value.
            let mut result = console::Plaintext::<CurrentNetwork>::from_str(&root.to_string()).unwrap().to_bits_le();
            // Reverse the bytes.
            result.reverse();
            // Build up the structure in reverse.
            for _ in 0..depth {
                // Write the size of the object in bits in reverse.
                let mut length = (u16::try_from(result.len()).unwrap()).to_bits_le();
                length.reverse();
                result.extend(length);
                // Write the member name in reverse.
                let mut member_name = console::Identifier::<CurrentNetwork>::from_str("inner").unwrap().to_bits_le();
                let mut member_name_length = u8::try_from(member_name.len()).unwrap().to_bits_le();
                member_name.reverse();
                result.extend(member_name);
                // Write the length of the member name in reverse.
                member_name_length.reverse();
                result.extend(member_name_length);
                // Write the number of members in the struct in reverse.
                let mut num_members = 1u8.to_bits_le();
                num_members.reverse();
                result.extend(num_members);
                // Write the plaintext variant in reverse.
                result.extend([true, false]);
            }
            // Reverse the result to get the correct order.
            result.reverse();
            // Transform the bits into a vector of booleans.
            let result: Vec<Boolean<CurrentAleo>> = result.into_iter().map(|input| Boolean::new(mode, input)).collect();
            result
        }

        // Creates a nested `Plaintext` structure with alternating array and struct wrappers.
        fn create_alternated_nested(mode: Mode, depth: usize, root: impl Display) -> Vec<Boolean<CurrentAleo>> {
            // Start from the innermost value.
            let mut result = console::Plaintext::<CurrentNetwork>::from_str(&root.to_string()).unwrap().to_bits_le();
            // Reverse the bytes.
            result.reverse();
            // Build up the structure in reverse.
            for i in 0..depth {
                // Write the size of the object in bits in reverse.
                let mut length = (u16::try_from(result.len()).unwrap()).to_bits_le();
                length.reverse();
                result.extend(length);
                // Determine the type of the wrapper (array or struct) and handle accordingly.
                if i % 2 == 0 {
                    // Write the number of elements in the array in reverse.
                    let mut num_elements = 1u32.to_bits_le();
                    num_elements.reverse();
                    result.extend(num_elements);
                    // Write the plaintext variant for array in reverse.
                    result.extend([false, true]);
                } else {
                    // Write the member name in reverse.
                    let mut member_name =
                        console::Identifier::<CurrentNetwork>::from_str("inner").unwrap().to_bits_le();
                    let mut member_name_length = u8::try_from(member_name.len()).unwrap().to_bits_le();
                    member_name.reverse();
                    result.extend(member_name);
                    // Write the member name length in reverse.
                    member_name_length.reverse();
                    result.extend(member_name_length);
                    // Write the number of members in the struct in reverse.
                    let mut num_members = 1u8.to_bits_le();
                    num_members.reverse();
                    result.extend(num_members);
                    // Write the plaintext variant for struct in reverse.
                    result.extend([true, false]);
                }
            }
            // Reverse the result to get the correct order.
            result.reverse();
            // Transform the bits into a vector of booleans.
            let result: Vec<Boolean<CurrentAleo>> = result.into_iter().map(|input| Boolean::new(mode, input)).collect();
            result
        }

        // A helper function to run the test.
        fn run_test(expected_depth: usize, input: Vec<Boolean<CurrentAleo>>, expected_error: bool) {
            // Parse the input string, catching any panics.
            let result = std::panic::catch_unwind(|| Plaintext::<CurrentAleo>::from_bits_le(&input));
            CurrentAleo::reset();
            // Check if the result is an error.
            match expected_error {
                true => {
                    assert!(result.is_err());
                    return;
                }
                false => assert!(result.is_ok()),
            };
            // Unwrap the result.
            let candidate = result.unwrap();
            // Check if the candidate is equal to the input.
            for (i, expected_bit) in input.iter().enumerate() {
                assert_eq!(expected_bit.eject_value(), candidate.to_bits_le()[i].eject_value());
            }
            // Check if the depth of the candidate is equal to the expected depth.
            assert_eq!(get_depth(&candidate), expected_depth);
        }

        // Initialize a sequence of depths to check.
        // Note that 890 is approximate maximum depth that can be constructed in this test.
        let mut depths = (0usize..100).collect_vec();
        depths.extend((100..890).step_by(10));

        // Test deeply nested arrays with different literal types.
        for i in depths.iter().copied() {
            run_test(i, create_nested_array(Mode::Constant, i, "false"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_array(Mode::Public, i, "1u8"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_array(Mode::Private, i, "0u128"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_array(Mode::Constant, i, "10field"), i > CurrentNetwork::MAX_DATA_DEPTH);
        }

        // Test deeply nested structs with different literal types.
        for i in depths.iter().copied() {
            run_test(i, create_nested_struct(Mode::Public, i, "false"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_struct(Mode::Private, i, "1u8"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_struct(Mode::Constant, i, "0u128"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_struct(Mode::Public, i, "10field"), i > CurrentNetwork::MAX_DATA_DEPTH);
        }

        // Test alternating nested arrays and structs.
        for i in depths.iter().copied() {
            run_test(i, create_alternated_nested(Mode::Private, i, "false"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_alternated_nested(Mode::Constant, i, "1u8"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_alternated_nested(Mode::Public, i, "0u128"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_alternated_nested(Mode::Private, i, "10field"), i > CurrentNetwork::MAX_DATA_DEPTH);
        }
    }

    #[test]
    fn test_deeply_nested_plaintext_bits_be() {
        // Creates a nested array-like `Plaintext` structure by wrapping a root value `depth` times.
        fn create_nested_array(mode: Mode, depth: usize, root: impl Display) -> Vec<Boolean<CurrentAleo>> {
            // Start from the innermost value.
            let mut result = console::Plaintext::<CurrentNetwork>::from_str(&root.to_string()).unwrap().to_bits_be();
            // Reverse the bits.
            result.reverse();
            // Build up the structure in reverse.
            for _ in 0..depth {
                // Write the size of the object in bits in reverse.
                let mut length = (u16::try_from(result.len()).unwrap()).to_bits_be();
                length.reverse();
                result.extend(length);
                // Write the number of elements in the array in reverse.
                let mut num_elements = 1u32.to_bits_be();
                num_elements.reverse();
                result.extend(num_elements);
                // Write the plaintext variant in reverse.
                result.extend([false, true]);
            }
            // Reverse the result to get the correct order.
            result.reverse();
            // Transform the bits into a vector of booleans.
            let result: Vec<Boolean<CurrentAleo>> = result.into_iter().map(|input| Boolean::new(mode, input)).collect();
            result
        }

        // Creates a nested struct-like `Plaintext` structure by wrapping a root value `depth` times.
        fn create_nested_struct(mode: Mode, depth: usize, root: impl Display) -> Vec<Boolean<CurrentAleo>> {
            // Start from the innermost value.
            let mut result = console::Plaintext::<CurrentNetwork>::from_str(&root.to_string()).unwrap().to_bits_be();
            // Reverse the bytes.
            result.reverse();
            // Build up the structure in reverse.
            for _ in 0..depth {
                // Write the size of the object in bits in reverse.
                let mut length = (u16::try_from(result.len()).unwrap()).to_bits_be();
                length.reverse();
                result.extend(length);
                // Write the member name in reverse.
                let mut member_name = console::Identifier::<CurrentNetwork>::from_str("inner").unwrap().to_bits_be();
                let mut member_name_length = u8::try_from(member_name.len()).unwrap().to_bits_be();
                member_name.reverse();
                result.extend(member_name);
                // Write the length of the member name in reverse.
                member_name_length.reverse();
                result.extend(member_name_length);
                // Write the number of members in the struct in reverse.
                let mut num_members = 1u8.to_bits_be();
                num_members.reverse();
                result.extend(num_members);
                // Write the plaintext variant in reverse.
                result.extend([true, false]);
            }
            // Reverse the result to get the correct order.
            result.reverse();
            // Transform the bits into a vector of booleans.
            let result: Vec<Boolean<CurrentAleo>> = result.into_iter().map(|input| Boolean::new(mode, input)).collect();
            result
        }

        // Creates a nested `Plaintext` structure with alternating array and struct wrappers.
        fn create_alternated_nested(mode: Mode, depth: usize, root: impl Display) -> Vec<Boolean<CurrentAleo>> {
            // Start from the innermost value.
            let mut result = console::Plaintext::<CurrentNetwork>::from_str(&root.to_string()).unwrap().to_bits_be();
            // Reverse the bytes.
            result.reverse();
            // Build up the structure in reverse.
            for i in 0..depth {
                // Write the size of the object in bits in reverse.
                let mut length = (u16::try_from(result.len()).unwrap()).to_bits_be();
                length.reverse();
                result.extend(length);
                // Determine the type of the wrapper (array or struct) and handle accordingly.
                if i % 2 == 0 {
                    // Write the number of elements in the array in reverse.
                    let mut num_elements = 1u32.to_bits_be();
                    num_elements.reverse();
                    result.extend(num_elements);
                    // Write the plaintext variant for array in reverse.
                    result.extend([false, true]);
                } else {
                    // Write the member name in reverse.
                    let mut member_name =
                        console::Identifier::<CurrentNetwork>::from_str("inner").unwrap().to_bits_be();
                    let mut member_name_length = u8::try_from(member_name.len()).unwrap().to_bits_be();
                    member_name.reverse();
                    result.extend(member_name);
                    // Write the member name length in reverse.
                    member_name_length.reverse();
                    result.extend(member_name_length);
                    // Write the number of members in the struct in reverse.
                    let mut num_members = 1u8.to_bits_be();
                    num_members.reverse();
                    result.extend(num_members);
                    // Write the plaintext variant for struct in reverse.
                    result.extend([true, false]);
                }
            }
            // Reverse the result to get the correct order.
            result.reverse();
            // Transform the bits into a vector of booleans.
            let result: Vec<Boolean<CurrentAleo>> = result.into_iter().map(|input| Boolean::new(mode, input)).collect();
            result
        }

        // A helper function to run the test.
        fn run_test(expected_depth: usize, input: Vec<Boolean<CurrentAleo>>, expected_error: bool) {
            // Parse the input string, catching any panics.
            let result = std::panic::catch_unwind(|| Plaintext::<CurrentAleo>::from_bits_be(&input));
            CurrentAleo::reset();
            // Check if the result is an error.
            match expected_error {
                true => {
                    assert!(result.is_err());
                    return;
                }
                false => assert!(result.is_ok()),
            };
            // Unwrap the result.
            let candidate = result.unwrap();
            // Check if the candidate is equal to the input.
            for (i, expected_bit) in input.iter().enumerate() {
                assert_eq!(expected_bit.eject_value(), candidate.to_bits_be()[i].eject_value());
            }
            // Check if the depth of the candidate is equal to the expected depth.
            assert_eq!(get_depth(&candidate), expected_depth);
        }

        // Initialize a sequence of depths to check.
        // Note that 890 is approximate maximum depth that can be constructed in this test.
        let mut depths = (0usize..100).collect_vec();
        depths.extend((100..890).step_by(10));

        // Test deeply nested arrays with different literal types.
        for i in depths.iter().copied() {
            run_test(i, create_nested_array(Mode::Constant, i, "false"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_array(Mode::Public, i, "1u8"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_array(Mode::Private, i, "0u128"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_array(Mode::Constant, i, "10field"), i > CurrentNetwork::MAX_DATA_DEPTH);
        }

        // Test deeply nested structs with different literal types.
        for i in depths.iter().copied() {
            run_test(i, create_nested_struct(Mode::Public, i, "false"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_struct(Mode::Private, i, "1u8"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_struct(Mode::Constant, i, "0u128"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_nested_struct(Mode::Public, i, "10field"), i > CurrentNetwork::MAX_DATA_DEPTH);
        }

        // Test alternating nested arrays and structs.
        for i in depths.iter().copied() {
            run_test(i, create_alternated_nested(Mode::Private, i, "false"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_alternated_nested(Mode::Constant, i, "1u8"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_alternated_nested(Mode::Public, i, "0u128"), i > CurrentNetwork::MAX_DATA_DEPTH);
            run_test(i, create_alternated_nested(Mode::Private, i, "10field"), i > CurrentNetwork::MAX_DATA_DEPTH);
        }
    }
}
