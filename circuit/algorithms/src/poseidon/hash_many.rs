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

impl<E: Environment, const RATE: usize> HashMany for Poseidon<E, RATE> {
    type Input = Field<E>;
    type Output = Field<E>;

    #[inline]
    fn hash_many(&self, input: &[Self::Input], num_outputs: u16) -> Vec<Self::Output> {
        // Construct the preimage: [ DOMAIN || LENGTH(INPUT) || [0; RATE-2] || INPUT ].
        let mut preimage = Vec::with_capacity(RATE + input.len());
        preimage.push(self.domain.clone());
        preimage.push(Field::constant(console::Field::from_u128(input.len() as u128)));
        preimage.resize(RATE, Field::zero()); // Pad up to RATE.
        preimage.extend_from_slice(input);

        // Initialize a new sponge.
        let mut state = vec![Field::zero(); RATE + CAPACITY];
        let mut mode = DuplexSpongeMode::Absorbing { next_absorb_index: 0 };

        // Absorb the input and squeeze the output.
        self.absorb(&mut state, &mut mode, &preimage);
        self.squeeze(&mut state, &mut mode, num_outputs)
    }
}

#[allow(clippy::needless_borrow)]
impl<E: Environment, const RATE: usize> Poseidon<E, RATE> {
    /// Absorbs the input elements into state.
    #[inline]
    fn absorb(&self, state: &mut [Field<E>], mode: &mut DuplexSpongeMode, input: &[Field<E>]) {
        if !input.is_empty() {
            // Determine the absorb index.
            let (mut absorb_index, should_permute) = match *mode {
                DuplexSpongeMode::Absorbing { next_absorb_index } => match next_absorb_index == RATE {
                    true => (0, true),
                    false => (next_absorb_index, false),
                },
                DuplexSpongeMode::Squeezing { .. } => (0, true),
            };

            // Proceed to permute the state, if necessary.
            if should_permute {
                self.permute(state);
            }

            let mut remaining = input;
            loop {
                // Compute the starting index.
                let start = CAPACITY + absorb_index;

                // Check if we can exit the loop.
                if absorb_index + remaining.len() <= RATE {
                    // Absorb the state elements into the input.
                    remaining.iter().enumerate().for_each(|(i, element)| state[start + i] += element);
                    // Update the sponge mode.
                    *mode = DuplexSpongeMode::Absorbing { next_absorb_index: absorb_index + remaining.len() };
                    return;
                }

                // Otherwise, proceed to absorb `(rate - absorb_index)` elements.
                let num_absorbed = RATE - absorb_index;
                remaining.iter().enumerate().take(num_absorbed).for_each(|(i, element)| state[start + i] += element);

                // Permute the state.
                self.permute(state);

                // Repeat with the updated input slice and absorb index.
                remaining = &remaining[num_absorbed..];
                absorb_index = 0;
            }
        }
    }

    /// Squeeze the specified number of state elements into the output.
    #[inline]
    fn squeeze(&self, state: &mut [Field<E>], mode: &mut DuplexSpongeMode, num_outputs: u16) -> Vec<Field<E>> {
        let mut output = vec![Field::zero(); num_outputs as usize];
        if num_outputs != 0 {
            self.squeeze_internal(state, mode, &mut output);
        }
        output
    }

    /// Squeeze the state elements into the output.
    #[inline]
    fn squeeze_internal(&self, state: &mut [Field<E>], mode: &mut DuplexSpongeMode, output: &mut [Field<E>]) {
        // Determine the squeeze index.
        let (mut squeeze_index, should_permute) = match *mode {
            DuplexSpongeMode::Absorbing { .. } => (0, true),
            DuplexSpongeMode::Squeezing { next_squeeze_index } => match next_squeeze_index == RATE {
                true => (0, true),
                false => (next_squeeze_index, false),
            },
        };

        // Proceed to permute the state, if necessary.
        if should_permute {
            self.permute(state);
        }

        let mut remaining = output;
        loop {
            // Compute the starting index.
            let start = CAPACITY + squeeze_index;

            // Check if we can exit the loop.
            if squeeze_index + remaining.len() <= RATE {
                // Store the state elements into the output.
                remaining.clone_from_slice(&state[start..(start + remaining.len())]);
                // Update the sponge mode.
                *mode = DuplexSpongeMode::Squeezing { next_squeeze_index: squeeze_index + remaining.len() };
                return;
            }

            // Otherwise, proceed to squeeze `(rate - squeeze_index)` elements.
            let num_squeezed = RATE - squeeze_index;
            remaining[..num_squeezed].clone_from_slice(&state[start..(start + num_squeezed)]);

            // Permute.
            self.permute(state);

            // Repeat with the updated output slice and squeeze index.
            remaining = &mut remaining[num_squeezed..];
            squeeze_index = 0;
        }
    }

    /// Apply the additive round keys in-place.
    #[inline]
    fn apply_ark(&self, state: &mut [Field<E>], round: usize) {
        for (i, element) in state.iter_mut().enumerate() {
            *element += &self.ark[round][i];
        }
    }

    /// Apply the S-Box based on whether it is a full round or partial round.
    #[inline]
    fn apply_s_box(&self, state: &mut [Field<E>], is_full_round: bool) {
        if is_full_round {
            // Full rounds apply the S Box (x^alpha) to every element of state
            for element in state.iter_mut() {
                *element = (&*element).pow(&self.alpha);
            }
        } else {
            // Partial rounds apply the S Box (x^alpha) to just the first element of state
            state[0] = (&state[0]).pow(&self.alpha);
        }
    }

    /// Apply the Maximally Distance Separating (MDS) matrix in-place.
    #[inline]
    fn apply_mds(&self, state: &mut [Field<E>]) {
        let mut new_state = Vec::with_capacity(state.len());
        for i in 0..state.len() {
            let mut accumulator = Field::zero();
            for (j, element) in state.iter().enumerate() {
                accumulator += element * &self.mds[i][j];
            }
            new_state.push(accumulator);
        }
        state.clone_from_slice(&new_state);
    }

    /// Apply the permutation for all rounds in-place.
    #[inline]
    fn permute(&self, state: &mut [Field<E>]) {
        // Determine the partial rounds range bound.
        let full_rounds_over_2 = self.full_rounds / 2;
        let partial_round_range = full_rounds_over_2..(full_rounds_over_2 + self.partial_rounds);

        // Iterate through all rounds to permute.
        for i in 0..(self.partial_rounds + self.full_rounds) {
            let is_full_round = !partial_round_range.contains(&i);
            self.apply_ark(state, i);
            self.apply_s_box(state, is_full_round);
            self.apply_mds(state);
        }
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_types::environment::Circuit;

    use anyhow::Result;

    const DOMAIN: &str = "PoseidonCircuit0";
    const ITERATIONS: usize = 10;
    const RATE: u16 = 4;

    fn check_hash_many(
        mode: Mode,
        num_inputs: usize,
        num_outputs: u16,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
        rng: &mut TestRng,
    ) -> Result<()> {
        use console::HashMany as H;

        let native = console::Poseidon::<<Circuit as Environment>::Network, { RATE as usize }>::setup(DOMAIN)?;
        let poseidon = Poseidon::<Circuit, { RATE as usize }>::constant(native.clone());

        for i in 0..ITERATIONS {
            // Prepare the preimage.
            let native_input = (0..num_inputs)
                .map(|_| console::Field::<<Circuit as Environment>::Network>::rand(rng))
                .collect::<Vec<_>>();
            let input = native_input.iter().map(|v| Field::<Circuit>::new(mode, *v)).collect::<Vec<_>>();

            // Compute the native hash.
            let expected = native.hash_many(&native_input, num_outputs);

            // Compute the circuit hash.
            Circuit::scope(format!("Poseidon {mode} {i} {num_outputs}"), || {
                let candidate = poseidon.hash_many(&input, num_outputs);
                for (expected_element, candidate_element) in expected.iter().zip_eq(&candidate) {
                    assert_eq!(*expected_element, candidate_element.eject_value());
                }
                let case = format!("(mode = {mode}, num_inputs = {num_inputs}, num_outputs = {num_outputs})");
                assert_scope!(case, num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
        Ok(())
    }

    #[test]
    fn test_hash_many_constant() -> Result<()> {
        let mut rng = TestRng::default();

        for num_inputs in 0..=RATE {
            for num_outputs in 0..=RATE {
                check_hash_many(Mode::Constant, num_inputs as usize, num_outputs, 1, 0, 0, 0, &mut rng)?;
            }
        }
        Ok(())
    }

    #[test]
    fn test_hash_many_public() -> Result<()> {
        let mut rng = TestRng::default();

        for num_outputs in 0..=RATE {
            check_hash_many(Mode::Public, 0, num_outputs, 1, 0, 0, 0, &mut rng)?;
        }
        for num_outputs in 1..=RATE {
            check_hash_many(Mode::Public, 1, num_outputs, 1, 0, 335, 335, &mut rng)?;
            check_hash_many(Mode::Public, 2, num_outputs, 1, 0, 340, 340, &mut rng)?;
            check_hash_many(Mode::Public, 3, num_outputs, 1, 0, 345, 345, &mut rng)?;
            check_hash_many(Mode::Public, 4, num_outputs, 1, 0, 350, 350, &mut rng)?;
            check_hash_many(Mode::Public, 5, num_outputs, 1, 0, 705, 705, &mut rng)?;
            check_hash_many(Mode::Public, 6, num_outputs, 1, 0, 705, 705, &mut rng)?;
        }
        for num_outputs in (RATE + 1)..=(RATE * 2) {
            check_hash_many(Mode::Public, 1, num_outputs, 1, 0, 690, 690, &mut rng)?;
            check_hash_many(Mode::Public, 2, num_outputs, 1, 0, 695, 695, &mut rng)?;
            check_hash_many(Mode::Public, 3, num_outputs, 1, 0, 700, 700, &mut rng)?;
            check_hash_many(Mode::Public, 4, num_outputs, 1, 0, 705, 705, &mut rng)?;
            check_hash_many(Mode::Public, 5, num_outputs, 1, 0, 1060, 1060, &mut rng)?;
            check_hash_many(Mode::Public, 6, num_outputs, 1, 0, 1060, 1060, &mut rng)?;
        }
        Ok(())
    }

    #[test]
    fn test_hash_many_private() -> Result<()> {
        let mut rng = TestRng::default();

        for num_outputs in 0..=RATE {
            check_hash_many(Mode::Private, 0, num_outputs, 1, 0, 0, 0, &mut rng)?;
        }
        for num_outputs in 1..=RATE {
            check_hash_many(Mode::Private, 1, num_outputs, 1, 0, 335, 335, &mut rng)?;
            check_hash_many(Mode::Private, 2, num_outputs, 1, 0, 340, 340, &mut rng)?;
            check_hash_many(Mode::Private, 3, num_outputs, 1, 0, 345, 345, &mut rng)?;
            check_hash_many(Mode::Private, 4, num_outputs, 1, 0, 350, 350, &mut rng)?;
            check_hash_many(Mode::Private, 5, num_outputs, 1, 0, 705, 705, &mut rng)?;
            check_hash_many(Mode::Private, 6, num_outputs, 1, 0, 705, 705, &mut rng)?;
        }
        for num_outputs in (RATE + 1)..=(RATE * 2) {
            check_hash_many(Mode::Private, 1, num_outputs, 1, 0, 690, 690, &mut rng)?;
            check_hash_many(Mode::Private, 2, num_outputs, 1, 0, 695, 695, &mut rng)?;
            check_hash_many(Mode::Private, 3, num_outputs, 1, 0, 700, 700, &mut rng)?;
            check_hash_many(Mode::Private, 4, num_outputs, 1, 0, 705, 705, &mut rng)?;
            check_hash_many(Mode::Private, 5, num_outputs, 1, 0, 1060, 1060, &mut rng)?;
            check_hash_many(Mode::Private, 6, num_outputs, 1, 0, 1060, 1060, &mut rng)?;
        }
        Ok(())
    }
}
