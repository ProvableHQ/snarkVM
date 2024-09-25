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
impl<A: Aleo> FromBits for ComputeKey<A> {
    type Boolean = Boolean<A>;

    /// Initializes a new compute key from a list of **little-endian** bits.
    fn from_bits_le(bits_le: &[Self::Boolean]) -> Self {
        let group_size_in_bits = console::Group::<A::Network>::size_in_bits();

        let (pk_sig_start, pk_sig_end) = (0, group_size_in_bits);
        let (pr_sig_start, pr_sig_end) = (pk_sig_end, pk_sig_end + group_size_in_bits);

        let Some(pk_sig_bits) = bits_le.get(pk_sig_start..pk_sig_end) else {
            A::halt("Unable to recover the 'pk_sig' (LE) bits for the compute key")
        };
        let Some(pr_sig_bits) = bits_le.get(pr_sig_start..pr_sig_end) else {
            A::halt("Unable to recover the 'pr_sig' (LE) bits for the compute key")
        };

        Self::from((Group::from_bits_le(pk_sig_bits), Group::from_bits_le(pr_sig_bits)))
    }

    /// Initializes a new compute key from a list of **big-endian** bits.
    fn from_bits_be(bits_be: &[Self::Boolean]) -> Self {
        let group_size_in_bits = console::Group::<A::Network>::size_in_bits();

        let (pk_sig_start, pk_sig_end) = (0, group_size_in_bits);
        let (pr_sig_start, pr_sig_end) = (pk_sig_end, pk_sig_end + group_size_in_bits);

        let Some(pk_sig_bits) = bits_be.get(pk_sig_start..pk_sig_end) else {
            A::halt("Unable to recover the 'pk_sig' (BE) bits for the compute key")
        };
        let Some(pr_sig_bits) = bits_be.get(pr_sig_start..pr_sig_end) else {
            A::halt("Unable to recover the 'pr_sig' (BE) bits for the compute key")
        };

        Self::from((Group::from_bits_be(pk_sig_bits), Group::from_bits_be(pr_sig_bits)))
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::Circuit;
    use snarkvm_circuit_network::AleoV0;
    use snarkvm_utilities::TestRng;

    type CurrentAleo = AleoV0;

    const ITERATIONS: u64 = 100;

    fn check_from_bits_le(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random compute key.
            let expected = console::ComputeKey::try_from(console::PrivateKey::new(rng).unwrap()).unwrap();
            let candidate = ComputeKey::<CurrentAleo>::new(mode, expected).to_bits_le();

            CurrentAleo::scope(&format!("{mode} {i}"), || {
                let candidate = ComputeKey::<CurrentAleo>::from_bits_le(&candidate);
                assert_eq!(expected, candidate.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            CurrentAleo::reset();
        }
    }

    fn check_from_bits_be(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random compute key.
            let expected = console::ComputeKey::try_from(console::PrivateKey::new(rng).unwrap()).unwrap();
            let candidate = ComputeKey::<CurrentAleo>::new(mode, expected).to_bits_be();

            CurrentAleo::scope(&format!("{mode} {i}"), || {
                let candidate = ComputeKey::<CurrentAleo>::from_bits_be(&candidate);
                assert_eq!(expected, candidate.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            CurrentAleo::reset();
        }
    }

    #[test]
    fn test_from_bits_le_constant() {
        check_from_bits_le(Mode::Constant, 272, 0, 0, 0);
    }

    #[test]
    fn test_from_bits_le_public() {
        check_from_bits_le(Mode::Public, 9, 0, 1375, 1379);
    }

    #[test]
    fn test_from_bits_le_private() {
        check_from_bits_le(Mode::Private, 9, 0, 1375, 1379);
    }

    #[test]
    fn test_from_bits_be_constant() {
        check_from_bits_be(Mode::Constant, 272, 0, 0, 0);
    }

    #[test]
    fn test_from_bits_be_public() {
        check_from_bits_be(Mode::Public, 9, 0, 1375, 1379);
    }

    #[test]
    fn test_from_bits_be_private() {
        check_from_bits_be(Mode::Private, 9, 0, 1375, 1379);
    }
}
