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

#[cfg(console)]
impl<A: Aleo> ToFields for Signature<A> {
    type Field = Field<A>;

    /// Casts a compute key into a list of base fields.
    fn to_fields(&self) -> Vec<Self::Field> {
        let mut fields = vec![self.challenge.to_field(), self.response.to_field()];
        fields.extend(self.compute_key.to_fields());
        fields
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::Circuit;
    use snarkvm_circuit_network::AleoV0;

    use console::ToFields as ConsoleToFields;
    use snarkvm_utilities::TestRng;

    type CurrentAleo = AleoV0;

    const ITERATIONS: u64 = 128;

    fn check_to_fields(
        mode: Mode,
        rng: &mut TestRng,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) {
        for i in 0..ITERATIONS {
            // Sample a random signature.
            let expected = crate::helpers::generate_signature(i, rng);
            let candidate = Signature::<CurrentAleo>::new(mode, expected);

            CurrentAleo::scope(format!("{mode} {expected} {i}"), || {
                // Perform the operation.
                let candidate = candidate.to_fields();
                assert_eq!(4, candidate.len());
                assert_scope!(num_constants, num_public, num_private, num_constraints);

                // Extract the bits from the base field representation.
                let candidate_bits_le = candidate.to_bits_le();
                assert_eq!(1012, candidate_bits_le.len());

                // Ensure all integer bits match with the expected result.
                let expected_bits = expected.to_fields().unwrap().to_bits_le();
                for (expected_bit, candidate_bit) in expected_bits.iter().zip_eq(&candidate_bits_le) {
                    assert_eq!(*expected_bit, candidate_bit.eject_value());
                }
            });
            CurrentAleo::reset();
        }
    }

    #[test]
    fn test_signature_to_fields_constant() {
        let mut rng = TestRng::default();
        check_to_fields(Mode::Constant, &mut rng, 0, 0, 0, 0);
    }

    #[test]
    fn test_signature_to_fields_public() {
        let mut rng = TestRng::default();
        check_to_fields(Mode::Public, &mut rng, 0, 0, 0, 0);
    }

    #[test]
    fn test_signature_to_fields_private() {
        let mut rng = TestRng::default();
        check_to_fields(Mode::Private, &mut rng, 0, 0, 0, 0);
    }
}
