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

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> CommitUncompressed
    for BHP<E, NUM_WINDOWS, WINDOW_SIZE>
{
    type Input = Boolean<E>;
    type Output = Group<E>;
    type Randomizer = Scalar<E>;

    /// Returns the BHP commitment of the given input and randomizer as an affine group element.
    fn commit_uncompressed(&self, input: &[Self::Input], randomizer: &Self::Randomizer) -> Self::Output {
        let hash = self.hash_uncompressed(input);

        // Compute h^r.
        randomizer
            .to_bits_le()
            .iter()
            .zip_eq(self.hasher.random_base())
            .map(|(bit, power)| Group::ternary(bit, power, &Group::zero()))
            .fold(hash, |acc, x| acc + x)
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_types::environment::Circuit;
    use snarkvm_curves::{AffineCurve, ProjectiveCurve};
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const ITERATIONS: u64 = 100;
    const DOMAIN: &str = "BHPCircuit0";

    fn check_commit_uncompressed<const NUM_WINDOWS: u8, const WINDOW_SIZE: u8>(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        use console::CommitUncompressed as C;

        // Initialize BHP.
        let native = console::BHP::<<Circuit as Environment>::Network, NUM_WINDOWS, WINDOW_SIZE>::setup(DOMAIN)?;
        let circuit = BHP::<Circuit, NUM_WINDOWS, WINDOW_SIZE>::new(Mode::Constant, native.clone());
        // Determine the number of inputs.
        let num_input_bits = NUM_WINDOWS as usize * WINDOW_SIZE as usize * BHP_CHUNK_SIZE;

        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random input.
            let input = (0..num_input_bits).map(|_| bool::rand(&mut rng)).collect::<Vec<bool>>();
            // Sample a randomizer.
            let randomizer = Uniform::rand(&mut rng);
            // Compute the expected commitment.
            let expected = native.commit_uncompressed(&input, &randomizer).expect("Failed to commit native input");
            // Prepare the circuit input.
            let circuit_input: Vec<Boolean<_>> = Inject::new(mode, input);
            // Prepare the circuit randomizer.
            let circuit_randomizer: Scalar<_> = Inject::new(mode, randomizer);

            Circuit::scope(format!("BHP {mode} {i}"), || {
                // Perform the hash operation.
                let candidate = circuit.commit_uncompressed(&circuit_input, &circuit_randomizer);
                assert_scope!(<=num_constants, num_public, num_private, num_constraints);
                assert_eq!(expected, candidate.eject_value());
                assert!(candidate.eject_value().to_affine().is_on_curve());
                assert!(candidate.eject_value().to_affine().is_in_correct_subgroup_assuming_on_curve());
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_commit_uncompressed_constant() -> Result<()> {
        check_commit_uncompressed::<32, 48>(Mode::Constant, 8200, 0, 0, 0)
    }

    #[test]
    fn test_commit_uncompressed_public() -> Result<()> {
        check_commit_uncompressed::<32, 48>(Mode::Public, 1044, 0, 10781, 10785)
    }

    #[test]
    fn test_commit_uncompressed_private() -> Result<()> {
        check_commit_uncompressed::<32, 48>(Mode::Private, 1044, 0, 10781, 10785)
    }
}
