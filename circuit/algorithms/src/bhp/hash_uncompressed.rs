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

impl<E: Environment, const NUM_WINDOWS: u8, const WINDOW_SIZE: u8> HashUncompressed
    for BHP<E, NUM_WINDOWS, WINDOW_SIZE>
{
    type Input = Boolean<E>;
    type Output = Group<E>;

    /// Returns the BHP hash of the given input as an affine group element.
    ///
    /// This uncompressed variant of the BHP hash function is provided to support
    /// the BHP commitment scheme, as it is typically not used by applications.
    fn hash_uncompressed(&self, input: &[Self::Input]) -> Self::Output {
        // The number of hasher bits to fit.
        let num_hasher_bits = NUM_WINDOWS as usize * WINDOW_SIZE as usize * BHP_CHUNK_SIZE;
        // The number of data bits in the output.
        let num_data_bits = E::BaseField::size_in_data_bits();
        // The maximum number of input bits per iteration.
        let max_input_bits_per_iteration = num_hasher_bits - num_data_bits;

        debug_assert!(num_data_bits < num_hasher_bits);
        debug_assert_eq!(num_data_bits - 64, self.domain.len());

        // Initialize a variable to store the hash from the current iteration.
        let mut digest = Group::zero();

        // Prepare a reusable vector for the preimage.
        let mut preimage = Vec::with_capacity(num_hasher_bits);

        // Compute the hash of the input.
        for (i, input_bits) in input.chunks(max_input_bits_per_iteration).enumerate() {
            // Determine if this is the first iteration.
            match i == 0 {
                // Construct the first iteration as: [ 0...0 || DOMAIN || LENGTH(INPUT) || INPUT[0..BLOCK_SIZE] ].
                true => {
                    // Initialize a vector for the hash preimage.
                    preimage.extend(self.domain.clone());
                    U64::constant(console::U64::new(input.len() as u64)).write_bits_le(&mut preimage);
                    preimage.extend_from_slice(input_bits);
                }
                // Construct the subsequent iterations as: [ PREVIOUS_HASH[0..DATA_BITS] || INPUT[I * BLOCK_SIZE..(I + 1) * BLOCK_SIZE] ].
                false => {
                    // Initialize a vector for the hash preimage.
                    digest.to_x_coordinate().write_bits_le(&mut preimage);
                    preimage.truncate(num_data_bits);
                    preimage.extend_from_slice(input_bits);
                }
            }
            // Hash the preimage for this iteration.
            digest = self.hasher.hash_uncompressed(&preimage);
            // Clear the preimage vector for the next iteration.
            preimage.clear();
        }

        digest
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

    macro_rules! check_hash_uncompressed {
        ($bhp:ident, $mode:ident, $num_bits:expr, ($num_constants:expr, $num_public:expr, $num_private:expr, $num_constraints:expr), $rng:expr) => {{
            // Initialize BHP.
            let native = console::$bhp::<<Circuit as Environment>::Network>::setup(DOMAIN)?;
            let circuit = $bhp::<Circuit>::constant(native.clone());

            for i in 0..ITERATIONS {
                // Sample a random input.
                let input = (0..$num_bits).map(|_| Uniform::rand($rng)).collect::<Vec<_>>();
                // Compute the expected hash.
                let expected = console::HashUncompressed::hash_uncompressed(&native, &input)?;
                // Prepare the circuit input.
                let circuit_input: Vec<Boolean<_>> = Inject::new(Mode::$mode, input);

                Circuit::scope(format!("BHP {i}"), || {
                    // Perform the hash operation.
                    let candidate = circuit.hash_uncompressed(&circuit_input);
                    assert_scope!($num_constants, $num_public, $num_private, $num_constraints);
                    assert_eq!(expected, candidate.eject_value());
                    assert!(candidate.eject_value().to_affine().is_on_curve());
                    assert!(candidate.eject_value().to_affine().is_in_correct_subgroup_assuming_on_curve());
                });
                Circuit::reset();
            }
            Ok::<_, anyhow::Error>(())
        }};
    }

    fn check_hash_uncompressed<const NUM_WINDOWS: u8, const WINDOW_SIZE: u8>(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        use console::HashUncompressed as H;

        // Initialize BHP.
        let native = console::BHP::<<Circuit as Environment>::Network, NUM_WINDOWS, WINDOW_SIZE>::setup(DOMAIN)?;
        let circuit = BHP::<Circuit, NUM_WINDOWS, WINDOW_SIZE>::new(Mode::Constant, native.clone());
        // Determine the number of inputs.
        let num_input_bits = NUM_WINDOWS as usize * WINDOW_SIZE as usize * BHP_CHUNK_SIZE;

        let mut rng = TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random input.
            let input = (0..num_input_bits).map(|_| bool::rand(&mut rng)).collect::<Vec<bool>>();
            // Compute the expected hash.
            let expected = native.hash_uncompressed(&input).expect("Failed to hash native input");
            // Prepare the circuit input.
            let circuit_input: Vec<Boolean<_>> = Inject::new(mode, input);

            Circuit::scope(format!("BHP {mode} {i}"), || {
                // Perform the hash operation.
                let candidate = circuit.hash_uncompressed(&circuit_input);
                assert_scope!(num_constants, num_public, num_private, num_constraints);
                assert_eq!(expected, candidate.eject_value());
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_hash_uncompressed_constant() -> Result<()> {
        check_hash_uncompressed::<32, 48>(Mode::Constant, 7239, 0, 0, 0)
    }

    #[test]
    fn test_hash_uncompressed_public() -> Result<()> {
        check_hash_uncompressed::<32, 48>(Mode::Public, 470, 0, 8774, 8776)
    }

    #[test]
    fn test_hash_uncompressed_private() -> Result<()> {
        check_hash_uncompressed::<32, 48>(Mode::Private, 470, 0, 8774, 8776)
    }

    #[test]
    fn test_hash_uncompressed_bhp256_constant() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP256, Constant, 261, (756, 0, 0, 0), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_bhp256_public() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP256, Public, 261, (403, 0, 445, 445), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_bhp256_private() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP256, Private, 261, (403, 0, 445, 445), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_bhp512_constant() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP512, Constant, 522, (1113, 0, 0, 0), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_bhp512_public() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP512, Public, 522, (409, 0, 895, 895), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_bhp512_private() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP512, Private, 522, (409, 0, 895, 895), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_bhp768_constant() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP768, Constant, 783, (1488, 0, 0, 0), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_bhp768_public() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP768, Public, 783, (429, 0, 1365, 1365), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_bhp768_private() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP768, Private, 783, (429, 0, 1365, 1365), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_bhp1024_constant() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP1024, Constant, 1043, (1815, 0, 0, 0), &mut rng)?;
        check_hash_uncompressed!(BHP1024, Constant, 1044, (1815, 0, 0, 0), &mut rng)?;
        check_hash_uncompressed!(BHP1024, Constant, 1046, (2413, 0, 0, 0), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_bhp1024_public() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP1024, Public, 1043, (413, 0, 1775, 1775), &mut rng)?;
        check_hash_uncompressed!(BHP1024, Public, 1044, (413, 0, 1775, 1775), &mut rng)?;
        check_hash_uncompressed!(BHP1024, Public, 1046, (418, 0, 2709, 2711), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_bhp1024_private() -> Result<()> {
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP1024, Private, 1043, (413, 0, 1775, 1775), &mut rng)?;
        check_hash_uncompressed!(BHP1024, Private, 1044, (413, 0, 1775, 1775), &mut rng)?;
        check_hash_uncompressed!(BHP1024, Private, 1046, (418, 0, 2709, 2711), &mut rng)
    }

    #[test]
    fn test_hash_uncompressed_cost_comparison() -> Result<()> {
        // The cost to hash 512 bits for each BHP variant is:
        let mut rng = TestRng::default();
        check_hash_uncompressed!(BHP256, Private, 512, (410, 0, 1799, 1801), &mut rng)?;
        check_hash_uncompressed!(BHP512, Private, 512, (409, 0, 880, 880), &mut rng)?;
        check_hash_uncompressed!(BHP768, Private, 512, (423, 0, 900, 900), &mut rng)?;
        check_hash_uncompressed!(BHP1024, Private, 512, (407, 0, 875, 875), &mut rng)
    }
}
