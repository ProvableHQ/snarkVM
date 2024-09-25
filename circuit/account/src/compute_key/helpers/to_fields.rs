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

impl<A: Aleo> ToFields for ComputeKey<A> {
    type Field = Field<A>;

    /// Casts a compute key into a list of base fields.
    fn to_fields(&self) -> Vec<Self::Field> {
        vec![self.pk_sig().to_field(), self.pr_sig().to_field()]
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::Circuit;
    use snarkvm_circuit_network::AleoV0;
    use snarkvm_utilities::TestRng;

    use console::ToFields as ConsoleToFields;

    type CurrentAleo = AleoV0;

    const ITERATIONS: u64 = 100;

    fn check_to_fields(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random compute key.
            let expected = console::ComputeKey::try_from(console::PrivateKey::new(rng).unwrap()).unwrap();
            let candidate = ComputeKey::<CurrentAleo>::new(mode, expected);

            CurrentAleo::scope(&format!("{mode} {i}"), || {
                let candidate = candidate.to_fields();
                assert_eq!(candidate.len(), 2);

                let expected = expected.to_fields().unwrap();
                assert_eq!(expected.len(), 2);

                for (expected_field, candidate_field) in expected.iter().zip_eq(candidate.iter()) {
                    assert_eq!(*expected_field, candidate_field.eject_value());
                }
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
        }
    }

    #[test]
    fn test_to_fields_constant() {
        check_to_fields(Mode::Constant, 0, 0, 0, 0);
    }

    #[test]
    fn test_to_fields_public() {
        check_to_fields(Mode::Public, 0, 0, 0, 0);
    }

    #[test]
    fn test_to_fields_private() {
        check_to_fields(Mode::Private, 0, 0, 0, 0);
    }
}
