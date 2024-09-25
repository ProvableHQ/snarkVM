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

impl<E: Environment, const RATE: usize> HashToGroup for Poseidon<E, RATE> {
    type Group = Group<E>;
    type Input = Field<E>;
    type Scalar = Scalar<E>;

    /// Returns an affine group element from hashing the input.
    #[inline]
    fn hash_to_group(&self, input: &[Self::Input]) -> Self::Group {
        // Ensure that the input is not empty.
        if input.is_empty() {
            E::halt("Input to hash to group cannot be empty")
        }
        // Compute `HashMany(input, 2)`.
        match self.hash_many(input, 2).iter().collect_tuple() {
            // Compute the group element as `MapToGroup(h0) + MapToGroup(h1)`.
            Some((h0, h1)) => Elligator2::encode(h1) + Elligator2::encode(h0),
            None => E::halt("Failed to compute the hash to group"),
        }
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_types::environment::Circuit;
    use snarkvm_curves::{AffineCurve, ProjectiveCurve};

    use anyhow::Result;

    const ITERATIONS: u64 = 100;
    const DOMAIN: &str = "PoseidonCircuit0";

    macro_rules! check_hash_to_group {
        ($poseidon:ident, $mode:ident, $num_fields:expr, ($num_constants:expr, $num_public:expr, $num_private:expr, $num_constraints:expr)) => {{
            // Initialize Poseidon.
            let native = console::$poseidon::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
            let circuit = $poseidon::<Circuit>::constant(native.clone());

            let rng = &mut TestRng::default();

            for i in 0..ITERATIONS {
                // Sample a random input.
                let input = (0..$num_fields).map(|_| Uniform::rand(rng)).collect::<Vec<_>>();
                // Compute the expected hash.
                let expected = console::HashToGroup::hash_to_group(&native, &input)?;
                // Prepare the circuit input.
                let circuit_input: Vec<Field<_>> = Inject::new(Mode::$mode, input);

                Circuit::scope(format!("Poseidon HashToGroup {i}"), || {
                    // Perform the hash operation.
                    let candidate = circuit.hash_to_group(&circuit_input);
                    assert_scope!($num_constants, $num_public, $num_private, $num_constraints);
                    assert_eq!(expected, candidate.eject_value());

                    // Eject the value to inspect it further.
                    let candidate = candidate.eject_value();
                    assert!((*candidate).to_affine().is_on_curve());
                    assert!((*candidate).to_affine().is_in_correct_subgroup_assuming_on_curve());
                    assert_ne!(console::Group::<<Circuit as Environment>::Network>::zero(), candidate);
                    assert_ne!(console::Group::<<Circuit as Environment>::Network>::generator(), candidate);

                    let candidate_cofactor_inv = candidate.div_by_cofactor();
                    assert_eq!(candidate, candidate_cofactor_inv.mul_by_cofactor());
                });
                Circuit::reset();
            }
            Ok::<_, anyhow::Error>(())
        }};
    }

    #[test]
    fn test_poseidon2_hash_to_group_constant() -> Result<()> {
        check_hash_to_group!(Poseidon2, Constant, 2, (1059, 0, 0, 0))
    }

    #[test]
    fn test_poseidon2_hash_to_group_public() -> Result<()> {
        check_hash_to_group!(Poseidon2, Public, 2, (529, 0, 2026, 2036))
    }

    #[test]
    fn test_poseidon2_hash_to_group_private() -> Result<()> {
        check_hash_to_group!(Poseidon2, Private, 2, (529, 0, 2026, 2036))
    }

    #[test]
    fn test_poseidon4_hash_to_group_constant() -> Result<()> {
        check_hash_to_group!(Poseidon4, Constant, 2, (1059, 0, 0, 0))
    }

    #[test]
    fn test_poseidon4_hash_to_group_public() -> Result<()> {
        check_hash_to_group!(Poseidon4, Public, 2, (529, 0, 2096, 2106))
    }

    #[test]
    fn test_poseidon4_hash_to_group_private() -> Result<()> {
        check_hash_to_group!(Poseidon4, Private, 2, (529, 0, 2096, 2106))
    }

    #[test]
    fn test_poseidon8_hash_to_group_constant() -> Result<()> {
        check_hash_to_group!(Poseidon8, Constant, 2, (1059, 0, 0, 0))
    }

    #[test]
    fn test_poseidon8_hash_to_group_public() -> Result<()> {
        check_hash_to_group!(Poseidon8, Public, 2, (529, 0, 2236, 2246))
    }

    #[test]
    fn test_poseidon8_hash_to_group_private() -> Result<()> {
        check_hash_to_group!(Poseidon8, Private, 2, (529, 0, 2236, 2246))
    }
}
