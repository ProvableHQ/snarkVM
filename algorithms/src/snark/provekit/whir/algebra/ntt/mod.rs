// Copyright (c) 2019-2026 Provable Inc.
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

// Originally derived from WHIR (https://github.com/WizardOfMenlo/whir),
// licensed under Apache-2.0 OR MIT.

//! NTT and related algorithms.

mod cooley_tukey;
mod matrix;
mod transpose;
mod utils;
mod wavelet;

use std::{fmt::Debug, sync::LazyLock};

use static_assertions::assert_obj_safe;

use self::matrix::MatrixMut;
pub use self::{
    cooley_tukey::NttEngine,
    transpose::transpose,
    wavelet::{inverse_wavelet_transform, wavelet_transform},
};
use crate::snark::provekit::whir::type_map::{self, TypeMap};

/// Global NTT registry. BLS12-377 `Fr` is inserted by
/// [`crate::snark::provekit::bls12_377::register`].
pub static NTT: LazyLock<TypeMap<NttFamily>> = LazyLock::new(TypeMap::new);

#[derive(Default)]
pub struct NttFamily;

impl type_map::Family for NttFamily {
    type Dyn<F: 'static> = dyn ReedSolomon<F>;
}

/// Trait for a Reed-Solomon encoder implementation for a given field `F`.
pub trait ReedSolomon<F>: Debug + Send + Sync {
    /// Returns the next supported order equal or larger than `size`.
    ///
    /// The result will be an NTT-smooth number suitable for `codeword_length`.
    ///
    /// Returns `None` if `size` exceeds the largest supported order.
    fn next_order(&self, size: usize) -> Option<usize>;

    fn generator(&self, codeword_length: usize) -> F;

    /// Returns the `index`th evaluation point.
    ///
    /// `masked_message_length`: the total message length including any mask
    /// values.
    ///
    /// # Panics
    ///
    /// Panics if any of the indices are `>= codeword_length` or `order` is not
    /// supported.
    fn evaluation_points(&self, masked_message_length: usize, codeword_length: usize, indices: &[usize]) -> Vec<F>;

    /// Compute a masked interleaved Reed-Solomon encoding.
    ///
    /// `messages` are `num_messages` slices of `message_length` elements.
    /// `masks` is a `num_messages` × `mask_length` matrix of blinding
    /// coefficients. `codeword_length` must be an NTT-smooth number >=
    /// `message_length + mask_length`. returns an `codeword_length ×
    /// num_messages` matrix.
    ///
    /// Each output value is the univariate polynomial evaluation in the
    /// evaluation point corresponding with the index of a coefficient list
    /// formed by concatenating message and mask.
    fn interleaved_encode(&self, messages: &[&[F]], masks: &[F], codeword_length: usize) -> Vec<F>;
}

assert_obj_safe!(ReedSolomon<snarkvm_curves::bls12_377::Fr>);

pub fn next_order<F: 'static>(size: usize) -> Option<usize> {
    NTT.get::<F>().expect("Unsupported NTT field.").next_order(size)
}

pub fn evaluation_points<F: 'static>(
    masked_message_length: usize,
    codeword_length: usize,
    indices: &[usize],
) -> Vec<F> {
    NTT.get::<F>().expect("Unsupported NTT field.").evaluation_points(masked_message_length, codeword_length, indices)
}

pub fn interleaved_rs_encode<F: 'static>(messages: &[&[F]], masks: &[F], codeword_length: usize) -> Vec<F> {
    NTT.get::<F>().expect("Unsupported NTT field.").interleaved_encode(messages, masks, codeword_length)
}

pub fn generator<F: 'static>(codeword_length: usize) -> F {
    NTT.get::<F>().expect("Unsupported NTT field.").generator(codeword_length)
}

#[cfg(any())]
mod tests {
    use std::iter;

    use proptest::{collection, prelude::Just, proptest, sample::select, strategy::Strategy};
    use rand::{SeedableRng, distributions::Standard, prelude::Distribution, rngs::StdRng};

    use super::*;
    use crate::snark::provekit::whir::{
        algebra::{random_vector, univariate_evaluate},
        utils::{chunks_exact_or_empty, zip_strict},
    };

    fn valid_codeword_lengths<F: 'static>(size: usize, count: usize) -> Vec<usize> {
        let ntt = NTT.get::<F>().expect("No NTT engine for field.");
        iter::successors(ntt.next_order(size), |size| ntt.next_order(*size + 1)).take(count).collect()
    }

    fn test<F: Field>(ntt: &dyn ReedSolomon<F>)
    where
        F: snarkvm_utilities::Uniform,
    {
        let cases = (0_usize..10, 0_usize..(1 << 10), 0_usize..(1 << 10), 1_usize..=32).prop_flat_map(
            |(num_messages, message_length, mask_length, sample_size)| {
                let valid_codeword_lengths = valid_codeword_lengths::<F>(message_length + mask_length, 6);
                select(valid_codeword_lengths).prop_flat_map(move |codeword_length| {
                    let sample_size = sample_size.min(codeword_length.max(1));
                    (
                        Just(num_messages),
                        Just(message_length),
                        Just(mask_length),
                        Just(codeword_length),
                        collection::vec(0..codeword_length, sample_size),
                    )
                })
            },
        );
        proptest!(|(
            seed: u64,
            (num_messages, message_length, mask_length, codeword_length, sampled_indices) in cases
        )| {
            let mut rng = StdRng::seed_from_u64(seed);
            let messages = (0..num_messages)
                .map(|_| random_vector(&mut rng, message_length))
                .collect::<Vec<_>>();
            let masks = random_vector(&mut rng, mask_length * num_messages);
            let message_refs = messages.iter().map(|v| v.as_slice()).collect::<Vec<_>>();
            let codeword = ntt.interleaved_encode(
                &message_refs,
                &masks,
                codeword_length,
            );

            // Output must be the right size.
            assert_eq!(codeword.len(), codeword_length * num_messages);

            // Output values are polynomial evaluations in the evaluation points.
            let mut evaluation_points = ntt.evaluation_points(message_length + mask_length, codeword_length, &sampled_indices);
            for (&index, &evaluation_point) in zip_strict(&sampled_indices, &evaluation_points) {
                let evaluations = &codeword[index * num_messages.. (index + 1) * num_messages];
                let masks = chunks_exact_or_empty(&masks, mask_length, num_messages);
                for ((message, mask), value) in zip_strict(zip_strict(&messages, masks), evaluations) {
                    assert_eq!(*value,
                        univariate_evaluate(message, evaluation_point)
                        + evaluation_point.pow([message_length as u64])
                        * univariate_evaluate(mask, evaluation_point));
                }
            }

            // Evaluation points are unique.
            let mut sample_indices = sampled_indices;
            sample_indices.sort_unstable();
            sample_indices.dedup();
            evaluation_points.sort_unstable();
            evaluation_points.dedup();
            assert_eq!(sample_indices.len(), evaluation_points.len());
        });
    }

    #[test]
    fn test_field64_1() {
        test::<fields::Field64>(NTT.get().unwrap().as_ref());
    }
}
