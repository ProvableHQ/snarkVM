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

impl<E: Environment, const RATE: usize> PRF for Poseidon<E, RATE> {
    type Input = Field<E>;
    type Output = Field<E>;
    type Seed = Field<E>;

    #[inline]
    fn prf(&self, seed: &Self::Seed, input: &[Self::Input]) -> Self::Output {
        // Construct the preimage: seed || input.
        let mut preimage = Vec::with_capacity(1 + input.len());
        preimage.push(seed.clone());
        preimage.extend_from_slice(input);

        // Hash the preimage to derive the PRF output.
        self.hash(&preimage)
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_types::environment::Circuit;
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const DOMAIN: &str = "PoseidonCircuit0";
    const ITERATIONS: usize = 10;
    const RATE: usize = 4;

    fn check_prf(
        mode: Mode,
        num_inputs: usize,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
        rng: &mut TestRng,
    ) -> Result<()> {
        use console::PRF as P;

        let native = console::Poseidon::<<Circuit as Environment>::Network, RATE>::setup(DOMAIN)?;
        let poseidon = Poseidon::<Circuit, RATE>::constant(native.clone());

        for i in 0..ITERATIONS {
            // Prepare the seed.
            let native_seed = Uniform::rand(rng);
            let seed = Field::new(mode, native_seed);

            // Prepare the preimage.
            let native_input = (0..num_inputs).map(|_| Uniform::rand(rng)).collect::<Vec<_>>();
            let input = native_input.iter().map(|v| Field::<Circuit>::new(mode, *v)).collect::<Vec<_>>();

            // Compute the native hash.
            let expected = native.prf(&native_seed, &native_input).expect("Failed to PRF native input");

            // Compute the circuit hash.
            Circuit::scope(format!("Poseidon PRF {mode} {i}"), || {
                let candidate = poseidon.prf(&seed, &input);
                assert_eq!(expected, candidate.eject_value());
                let case = format!("(mode = {mode}, num_inputs = {num_inputs})");
                assert_scope!(case, num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_prf_constant() -> Result<()> {
        let mut rng = TestRng::default();

        for num_inputs in 0..=RATE {
            check_prf(Mode::Constant, num_inputs, 1, 0, 0, 0, &mut rng)?;
        }
        Ok(())
    }

    #[test]
    fn test_prf_public() -> Result<()> {
        let mut rng = TestRng::default();

        check_prf(Mode::Public, 0, 1, 0, 335, 335, &mut rng)?;
        check_prf(Mode::Public, 1, 1, 0, 340, 340, &mut rng)?;
        check_prf(Mode::Public, 2, 1, 0, 345, 345, &mut rng)?;
        check_prf(Mode::Public, 3, 1, 0, 350, 350, &mut rng)?;
        check_prf(Mode::Public, 4, 1, 0, 705, 705, &mut rng)?;
        check_prf(Mode::Public, 5, 1, 0, 705, 705, &mut rng)?;
        check_prf(Mode::Public, 6, 1, 0, 705, 705, &mut rng)?;
        check_prf(Mode::Public, 7, 1, 0, 705, 705, &mut rng)?;
        check_prf(Mode::Public, 8, 1, 0, 1060, 1060, &mut rng)?;
        check_prf(Mode::Public, 9, 1, 0, 1060, 1060, &mut rng)?;
        check_prf(Mode::Public, 10, 1, 0, 1060, 1060, &mut rng)
    }

    #[test]
    fn test_prf_private() -> Result<()> {
        let mut rng = TestRng::default();

        check_prf(Mode::Private, 0, 1, 0, 335, 335, &mut rng)?;
        check_prf(Mode::Private, 1, 1, 0, 340, 340, &mut rng)?;
        check_prf(Mode::Private, 2, 1, 0, 345, 345, &mut rng)?;
        check_prf(Mode::Private, 3, 1, 0, 350, 350, &mut rng)?;
        check_prf(Mode::Private, 4, 1, 0, 705, 705, &mut rng)?;
        check_prf(Mode::Private, 5, 1, 0, 705, 705, &mut rng)?;
        check_prf(Mode::Private, 6, 1, 0, 705, 705, &mut rng)?;
        check_prf(Mode::Private, 7, 1, 0, 705, 705, &mut rng)?;
        check_prf(Mode::Private, 8, 1, 0, 1060, 1060, &mut rng)?;
        check_prf(Mode::Private, 9, 1, 0, 1060, 1060, &mut rng)?;
        check_prf(Mode::Private, 10, 1, 0, 1060, 1060, &mut rng)
    }
}
