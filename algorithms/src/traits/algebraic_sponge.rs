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

use smallvec::SmallVec;
use snarkvm_fields::{PrimeField, ToConstraintField};
use snarkvm_utilities::FromBits;

use core::fmt::Debug;

/// The interface for a cryptographic sponge.
/// A sponge can `absorb` or take in inputs and later `squeeze` or output bytes or field elements.
/// The outputs are dependent on previous `absorb` and `squeeze` calls.
pub trait AlgebraicSponge<F: PrimeField, const RATE: usize>: Clone + Debug {
    /// Parameters used by the sponge.
    type Parameters;

    fn sample_parameters() -> Self::Parameters;

    /// Initialize a new instance of the sponge.
    fn new_with_parameters(params: &Self::Parameters) -> Self;

    /// Initialize a new instance of the sponge.
    fn new() -> Self {
        let parameters = Self::sample_parameters();
        Self::new_with_parameters(&parameters)
    }

    /// Takes in field elements.
    fn absorb_native_field_elements<T: ToConstraintField<F>>(&mut self, elements: &[T]);

    /// Takes in field elements.
    fn absorb_nonnative_field_elements<Target: PrimeField>(&mut self, elements: impl IntoIterator<Item = Target>);

    /// Takes in bytes.
    fn absorb_bytes(&mut self, elements: &[u8]) {
        let capacity = F::size_in_bits() - 1;
        let mut bits = Vec::<bool>::with_capacity(elements.len() * 8);
        for elem in elements {
            bits.extend_from_slice(&[
                elem & 128 != 0,
                elem & 64 != 0,
                elem & 32 != 0,
                elem & 16 != 0,
                elem & 8 != 0,
                elem & 4 != 0,
                elem & 2 != 0,
                elem & 1 != 0,
            ]);
        }
        let elements = bits
            .chunks(capacity)
            .map(|bits| F::from_bigint(F::BigInteger::from_bits_be(bits).unwrap()).unwrap())
            .collect::<SmallVec<[F; 10]>>();

        self.absorb_native_field_elements(&elements);
    }

    /// Takes in field elements.
    fn squeeze_native_field_elements(&mut self, num: usize) -> SmallVec<[F; 10]>;

    /// Takes out field elements.
    fn squeeze_nonnative_field_elements<Target: PrimeField>(&mut self, num: usize) -> SmallVec<[Target; 10]>;

    /// Takes out field elements of 168 bits.
    fn squeeze_short_nonnative_field_elements<Target: PrimeField>(&mut self, num: usize) -> SmallVec<[Target; 10]>;

    /// Takes out a field element of 168 bits.
    fn squeeze_short_nonnative_field_element<Target: PrimeField>(&mut self) -> Target {
        self.squeeze_short_nonnative_field_elements(1)[0]
    }
}

/// The mode structure for duplex sponges
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum DuplexSpongeMode {
    /// The sponge is currently absorbing data.
    Absorbing {
        /// next position of the state to be XOR-ed when absorbing.
        next_absorb_index: usize,
    },
    /// The sponge is currently squeezing data out.
    Squeezing {
        /// next position of the state to be outputted when squeezing.
        next_squeeze_index: usize,
    },
}

pub(crate) mod nonnative_params {
    /// A macro for computing ceil(log2(x))+1 for a field element x. The num_bits
    /// param is expected to be a vector to which the BE bits can be written; it is
    /// not created here, as reusing it allows us to avoid a lot of allocations.
    #[macro_export]
    macro_rules! overhead {
        ($x:expr, $num_bits:expr) => {{
            use snarkvm_utilities::ToBits;
            let num = $x;
            let num_bits = $num_bits;
            num.to_bigint().write_bits_be(num_bits);
            let mut skipped_bits = 0;
            for b in num_bits.iter() {
                if *b == false {
                    skipped_bits += 1;
                } else {
                    break;
                }
            }

            let mut is_power_of_2 = true;
            for b in num_bits.iter().skip(skipped_bits + 1) {
                if *b == true {
                    is_power_of_2 = false;
                }
            }

            let result = if is_power_of_2 { num_bits.len() - skipped_bits } else { num_bits.len() - skipped_bits + 1 };

            // Clear the reusable vector for bits.
            num_bits.clear();

            result
        }};
    }

    /// Parameters for a specific `NonNativeFieldVar` instantiation
    #[derive(Clone, Debug)]
    pub struct NonNativeFieldParams {
        /// The number of limbs (`BaseField` elements) used to represent a `TargetField` element. Highest limb first.
        pub num_limbs: usize,

        /// The number of bits of the limb
        pub bits_per_limb: usize,
    }

    /// Obtain the parameters from a `ConstraintSystem`'s cache or generate a new one
    #[must_use]
    pub const fn get_params(
        target_field_size: usize,
        base_field_size: usize,
        optimization_type: OptimizationType,
    ) -> NonNativeFieldParams {
        let (num_of_limbs, limb_size) = find_parameters(base_field_size, target_field_size, optimization_type);
        NonNativeFieldParams { num_limbs: num_of_limbs, bits_per_limb: limb_size }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    /// The type of optimization target for the parameters searching
    pub enum OptimizationType {
        /// Optimized for constraints
        Constraints,
        /// Optimized for weight
        Weight,
    }

    /// A function to search for parameters for nonnative field gadgets
    pub const fn find_parameters(
        base_field_prime_length: usize,
        target_field_prime_bit_length: usize,
        optimization_type: OptimizationType,
    ) -> (usize, usize) {
        let mut found = false;
        let mut min_cost = 0usize;
        let mut min_cost_limb_size = 0usize;
        let mut min_cost_num_of_limbs = 0usize;

        let surfeit = 10;
        let mut max_limb_size = (base_field_prime_length - 1 - surfeit - 1) / 2 - 1;
        if max_limb_size > target_field_prime_bit_length {
            max_limb_size = target_field_prime_bit_length;
        }
        let mut limb_size = 1;

        while limb_size <= max_limb_size {
            let num_of_limbs = (target_field_prime_bit_length + limb_size - 1) / limb_size;

            let group_size = (base_field_prime_length - 1 - surfeit - 1 - 1 - limb_size + limb_size - 1) / limb_size;
            let num_of_groups = (2 * num_of_limbs - 1 + group_size - 1) / group_size;

            let mut this_cost = 0;

            match optimization_type {
                OptimizationType::Constraints => {
                    this_cost += 2 * num_of_limbs - 1;
                }
                OptimizationType::Weight => {
                    this_cost += 6 * num_of_limbs * num_of_limbs;
                }
            };

            match optimization_type {
                OptimizationType::Constraints => {
                    this_cost += target_field_prime_bit_length; // allocation of k
                    this_cost += target_field_prime_bit_length + num_of_limbs; // allocation of r
                    //this_cost += 2 * num_of_limbs - 1; // compute kp
                    this_cost += num_of_groups + (num_of_groups - 1) * (limb_size * 2 + surfeit) + 1;
                    // equality check
                }
                OptimizationType::Weight => {
                    this_cost += target_field_prime_bit_length * 3 + target_field_prime_bit_length; // allocation of k
                    this_cost += target_field_prime_bit_length * 3 + target_field_prime_bit_length + num_of_limbs; // allocation of r
                    this_cost += num_of_limbs * num_of_limbs + 2 * (2 * num_of_limbs - 1); // compute kp
                    this_cost += num_of_limbs
                        + num_of_groups
                        + 6 * num_of_groups
                        + (num_of_groups - 1) * (2 * limb_size + surfeit) * 4
                        + 2; // equality check
                }
            };

            if !found || this_cost < min_cost {
                found = true;
                min_cost = this_cost;
                min_cost_limb_size = limb_size;
                min_cost_num_of_limbs = num_of_limbs;
            }

            limb_size += 1;
        }

        (min_cost_num_of_limbs, min_cost_limb_size)
    }
}
