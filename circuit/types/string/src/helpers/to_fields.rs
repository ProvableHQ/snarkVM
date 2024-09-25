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
use snarkvm_circuit_types_field::Field;

impl<E: Environment> ToFields for StringType<E> {
    type Field = Field<E>;

    /// Casts a string into a list of base fields.
    fn to_fields(&self) -> Vec<Self::Field> {
        // Convert the string bytes into bits, then chunk them into lists of size
        // `E::BaseField::size_in_data_bits()` and recover the base field element for each chunk.
        // (For advanced users: Chunk into CAPACITY bits and create a linear combination per chunk.)
        self.to_bits_le().chunks(E::BaseField::size_in_data_bits()).map(Field::from_bits_le).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuit_environment::Circuit;
    use snarkvm_utilities::{bytes_from_bits_le, FromBytes};

    fn native_string_to_fields(string: &str) -> Vec<console::Field<<Circuit as Environment>::Network>> {
        string
            .to_string()
            .to_bits_le()
            .chunks(<Circuit as Environment>::BaseField::size_in_data_bits())
            .map(|chunk| {
                // Resize the list to match the field size in bits.
                let mut chunk = chunk.to_vec();
                chunk.resize(<Circuit as Environment>::BaseField::size_in_bits(), false);
                // Recover the field element.
                let chunk = bytes_from_bits_le(&chunk);
                console::Field::new(<Circuit as Environment>::BaseField::from_bytes_le(&chunk).unwrap())
            })
            .collect()
    }

    fn check_to_fields(
        name: &str,
        expected: &[console::Field<<Circuit as Environment>::Network>],
        candidate: &StringType<Circuit>,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        Circuit::scope(name, || {
            // Perform the operation.
            let candidate = candidate.to_fields();
            assert_eq!(expected.len(), candidate.len());
            assert_scope!(num_constants, num_public, num_private, num_constraints);

            // Ensure all field elements match with the expected result.
            for (expected_field, candidate_field) in expected.iter().zip_eq(&candidate) {
                assert_eq!(expected_field, &candidate_field.eject_value());
            }
        });
        Circuit::reset();
    }

    #[test]
    fn test_to_fields_constant() {
        let rng = &mut TestRng::default();

        // Sample a random string. Take 1/4th to ensure we fit for all code points.
        let given = rng.next_string(Circuit::MAX_STRING_BYTES / 4, false);

        let expected = native_string_to_fields(&given);
        let candidate = StringType::<Circuit>::new(Mode::Constant, console::StringType::new(&given));
        check_to_fields("Constant", &expected, &candidate, 0, 0, 0, 0);
    }

    #[test]
    fn test_to_fields_public() {
        let rng = &mut TestRng::default();

        // Sample a random string. Take 1/4th to ensure we fit for all code points.
        let given = rng.next_string(Circuit::MAX_STRING_BYTES / 4, false);

        let expected = native_string_to_fields(&given);
        let candidate = StringType::<Circuit>::new(Mode::Public, console::StringType::new(&given));
        check_to_fields("Public", &expected, &candidate, 0, 0, 0, 0);
    }

    #[test]
    fn test_to_fields_private() {
        let rng = &mut TestRng::default();

        // Sample a random string. Take 1/4th to ensure we fit for all code points.
        let given = rng.next_string(Circuit::MAX_STRING_BYTES / 4, false);

        let expected = native_string_to_fields(&given);
        let candidate = StringType::<Circuit>::new(Mode::Private, console::StringType::new(&given));
        check_to_fields("Private", &expected, &candidate, 0, 0, 0, 0);
    }
}
