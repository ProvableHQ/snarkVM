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
impl<A: Aleo> FromBits for Signature<A> {
    type Boolean = Boolean<A>;

    /// Initializes a new signature from a list of **little-endian** bits.
    fn from_bits_le(bits_le: &[Self::Boolean]) -> Self {
        let scalar_size_in_bits = console::Scalar::<A::Network>::size_in_bits();
        let compute_key_size_in_bits = console::ComputeKey::<A::Network>::size_in_bits();

        let (challenge_start, challenge_end) = (0, scalar_size_in_bits);
        let (response_start, response_end) = (challenge_end, challenge_end + scalar_size_in_bits);
        let (compute_key_start, compute_key_end) = (response_end, response_end + compute_key_size_in_bits);

        let Some(challenge_bits) = bits_le.get(challenge_start..challenge_end) else {
            A::halt("Unable to recover the signature challenge from (LE) bits")
        };
        let Some(response_bits) = bits_le.get(response_start..response_end) else {
            A::halt("Unable to recover the signature response from (LE) bits")
        };
        let Some(compute_key_bits) = bits_le.get(compute_key_start..compute_key_end) else {
            A::halt("Unable to recover the signature compute key from (LE) bits")
        };

        Self {
            challenge: Scalar::from_bits_le(challenge_bits),
            response: Scalar::from_bits_le(response_bits),
            compute_key: ComputeKey::from_bits_le(compute_key_bits),
        }
    }

    /// Initializes a new signature from a list of **big-endian** bits.
    fn from_bits_be(bits_be: &[Self::Boolean]) -> Self {
        let scalar_size_in_bits = console::Scalar::<A::Network>::size_in_bits();
        let compute_key_size_in_bits = console::ComputeKey::<A::Network>::size_in_bits();

        let (challenge_start, challenge_end) = (0, scalar_size_in_bits);
        let (response_start, response_end) = (challenge_end, challenge_end + scalar_size_in_bits);
        let (compute_key_start, compute_key_end) = (response_end, response_end + compute_key_size_in_bits);

        let Some(challenge_bits) = bits_be.get(challenge_start..challenge_end) else {
            A::halt("Unable to recover the signature challenge from (BE) bits")
        };
        let Some(response_bits) = bits_be.get(response_start..response_end) else {
            A::halt("Unable to recover the signature response from (BE) bits")
        };
        let Some(compute_key_bits) = bits_be.get(compute_key_start..compute_key_end) else {
            A::halt("Unable to recover the signature compute key from (BE) bits")
        };

        Self {
            challenge: Scalar::from_bits_be(challenge_bits),
            response: Scalar::from_bits_be(response_bits),
            compute_key: ComputeKey::from_bits_be(compute_key_bits),
        }
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
            // Sample a random signature.
            let expected = crate::helpers::generate_signature(i, rng);
            let candidate = Signature::<CurrentAleo>::new(mode, expected).to_bits_le();

            CurrentAleo::scope(&format!("{mode} {i}"), || {
                let candidate = Signature::<CurrentAleo>::from_bits_le(&candidate);
                assert_eq!(expected, candidate.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            CurrentAleo::reset();
        }
    }

    fn check_from_bits_be(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let rng = &mut TestRng::default();

        for i in 0..ITERATIONS {
            // Sample a random signature.
            let expected = crate::helpers::generate_signature(i, rng);
            let candidate = Signature::<CurrentAleo>::new(mode, expected).to_bits_be();

            CurrentAleo::scope(&format!("{mode} {i}"), || {
                let candidate = Signature::<CurrentAleo>::from_bits_be(&candidate);
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
        check_from_bits_le(Mode::Public, 9, 0, 1875, 1881);
    }

    #[test]
    fn test_from_bits_le_private() {
        check_from_bits_le(Mode::Private, 9, 0, 1875, 1881);
    }

    #[test]
    fn test_from_bits_be_constant() {
        check_from_bits_be(Mode::Constant, 272, 0, 0, 0);
    }

    #[test]
    fn test_from_bits_be_public() {
        check_from_bits_be(Mode::Public, 9, 0, 1875, 1881);
    }

    #[test]
    fn test_from_bits_be_private() {
        check_from_bits_be(Mode::Private, 9, 0, 1875, 1881);
    }
}
