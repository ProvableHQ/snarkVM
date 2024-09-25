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

impl<E: Environment, const RATE: usize> HashToScalar for Poseidon<E, RATE> {
    type Input = Field<E>;
    type Scalar = Scalar<E>;

    /// Returns a scalar from hashing the input.
    /// This method uses truncation (up to data bits) to project onto the scalar field.
    #[inline]
    fn hash_to_scalar(&self, input: &[Self::Input]) -> Self::Scalar {
        // Hash the input to the base field.
        let output = self.hash(input);
        // Convert the output to the scalar field,
        // truncating to the size in data bits (1 bit less than the MODULUS) of the scalar.
        Scalar::from_field_lossy(&output)
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_types::environment::Circuit;

    use anyhow::Result;

    const DOMAIN: &str = "PoseidonCircuit0";
    const ITERATIONS: usize = 10;
    const RATE: usize = 4;

    fn check_hash_to_scalar(
        mode: Mode,
        num_inputs: usize,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
        rng: &mut TestRng,
    ) -> Result<()> {
        use console::HashToScalar as H;

        let native = console::Poseidon::<<Circuit as Environment>::Network, RATE>::setup(DOMAIN)?;
        let poseidon = Poseidon::<Circuit, RATE>::constant(native.clone());

        for i in 0..ITERATIONS {
            // Prepare the preimage.
            let native_input = (0..num_inputs).map(|_| Uniform::rand(rng)).collect::<Vec<_>>();
            let input = native_input.iter().map(|v| Field::<Circuit>::new(mode, *v)).collect::<Vec<_>>();

            // Compute the native hash to scalar.
            let expected = native.hash_to_scalar(&native_input)?;

            // Compute the circuit hash.
            Circuit::scope(format!("Poseidon {mode} {i}"), || {
                let candidate = poseidon.hash_to_scalar(&input);
                assert_eq!(expected, candidate.eject_value());
                let case = format!("(mode = {mode}, num_inputs = {num_inputs})");
                assert_scope!(case, num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_hash_to_scalar_constant() -> Result<()> {
        let mut rng = TestRng::default();

        for num_inputs in 0..=RATE {
            check_hash_to_scalar(Mode::Constant, num_inputs, 254, 0, 0, 0, &mut rng)?;
        }
        Ok(())
    }

    #[test]
    fn test_hash_to_scalar_public() -> Result<()> {
        let mut rng = TestRng::default();

        check_hash_to_scalar(Mode::Public, 0, 254, 0, 0, 0, &mut rng)?;
        check_hash_to_scalar(Mode::Public, 1, 1, 0, 840, 842, &mut rng)?;
        check_hash_to_scalar(Mode::Public, 2, 1, 0, 845, 847, &mut rng)?;
        check_hash_to_scalar(Mode::Public, 3, 1, 0, 850, 852, &mut rng)?;
        check_hash_to_scalar(Mode::Public, 4, 1, 0, 855, 857, &mut rng)?;
        check_hash_to_scalar(Mode::Public, 5, 1, 0, 1210, 1212, &mut rng)?;
        check_hash_to_scalar(Mode::Public, 6, 1, 0, 1210, 1212, &mut rng)?;
        check_hash_to_scalar(Mode::Public, 7, 1, 0, 1210, 1212, &mut rng)?;
        check_hash_to_scalar(Mode::Public, 8, 1, 0, 1210, 1212, &mut rng)?;
        check_hash_to_scalar(Mode::Public, 9, 1, 0, 1565, 1567, &mut rng)?;
        check_hash_to_scalar(Mode::Public, 10, 1, 0, 1565, 1567, &mut rng)
    }

    #[test]
    fn test_hash_to_scalar_private() -> Result<()> {
        let mut rng = TestRng::default();

        check_hash_to_scalar(Mode::Private, 0, 254, 0, 0, 0, &mut rng)?;
        check_hash_to_scalar(Mode::Private, 1, 1, 0, 840, 842, &mut rng)?;
        check_hash_to_scalar(Mode::Private, 2, 1, 0, 845, 847, &mut rng)?;
        check_hash_to_scalar(Mode::Private, 3, 1, 0, 850, 852, &mut rng)?;
        check_hash_to_scalar(Mode::Private, 4, 1, 0, 855, 857, &mut rng)?;
        check_hash_to_scalar(Mode::Private, 5, 1, 0, 1210, 1212, &mut rng)?;
        check_hash_to_scalar(Mode::Private, 6, 1, 0, 1210, 1212, &mut rng)?;
        check_hash_to_scalar(Mode::Private, 7, 1, 0, 1210, 1212, &mut rng)?;
        check_hash_to_scalar(Mode::Private, 8, 1, 0, 1210, 1212, &mut rng)?;
        check_hash_to_scalar(Mode::Private, 9, 1, 0, 1565, 1567, &mut rng)?;
        check_hash_to_scalar(Mode::Private, 10, 1, 0, 1565, 1567, &mut rng)
    }
}
