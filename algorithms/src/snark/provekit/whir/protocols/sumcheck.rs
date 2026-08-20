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

//! Quadratic sumcheck protocol.

use std::fmt;

use serde::{Deserialize, Serialize};
use snarkvm_fields::Field;
#[cfg(any())]
use tracing::instrument;

use crate::snark::provekit::whir::{
    algebra::{
        dot,
        sumcheck::{compute_sumcheck_polynomial, fold, fold_and_compute_polynomial},
        univariate_evaluate,
    },
    protocols::proof_of_work,
    transcript::{
        Codec,
        Decoding,
        DuplexSpongeInterface,
        FieldElem,
        ProverState,
        VerificationResult,
        VerifierMessage,
        VerifierState,
        codecs::U64,
    },
    type_info::Type,
    utils::chunks_exact_or_empty,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Config<F>
where
    F: Field,
{
    pub field: Type<F>,
    pub initial_size: usize,
    pub round_pow: proof_of_work::Config,
    pub num_rounds: usize,
    pub mask_length: usize,
}

impl<F: Field> Config<F> {
    pub fn final_size(&self) -> usize {
        assert!(self.num_rounds == 0 || self.initial_size.next_power_of_two() >= 1 << self.num_rounds);
        if self.initial_size == 0 || self.num_rounds == 0 {
            self.initial_size
        } else {
            self.initial_size.next_power_of_two() >> self.num_rounds
        }
    }

    /// Runs the quadratic sumcheck protocol as configured.
    ///
    /// It reduces a claim of the form `dot(a, b) == sum` to an exponentially
    /// smaller claim `dot(a', b') == sum'` where `a'` is `a` folded in place
    /// and similarly for `b`.
    ///
    /// This function:
    /// - Samples random values to progressively reduce the polynomial.
    /// - Applies proof-of-work grinding if required.
    /// - Returns the sampled folding randomness values used in each reduction
    ///   step.
    #[cfg_attr(any(), instrument(skip_all))]
    pub fn prove<H>(
        &self,
        prover_state: &mut ProverState<H>,
        a: &mut Vec<F>,
        b: &mut Vec<F>,
        sum: &mut F,
        masks: &[F],
    ) -> (Vec<F>, F, F)
    where
        H: DuplexSpongeInterface,
        FieldElem<F>: Codec<[H::U]>,
        [u8; 32]: Decoding<[H::U]>,
        U64: Codec<[H::U]>,
    {
        assert!(self.num_rounds == 0 || self.initial_size.next_power_of_two() >= 1 << self.num_rounds);
        assert!(self.mask_length == 0 || self.mask_length >= 3);
        assert_eq!(a.len(), self.initial_size);
        assert_eq!(b.len(), self.initial_size);
        debug_assert_eq!(dot(a, b), *sum);
        assert_eq!(masks.len(), self.num_rounds * self.mask_length);
        let half = F::from(2u64).inverse().unwrap();

        // Send mask sum and get combination randomness.
        let mut mask_sum = F::zero();
        let mut mask_rlc = F::one();
        if !masks.is_empty() {
            let sum_multiple = F::from((1u64) << self.num_rounds.saturating_sub(1));
            mask_sum = masks
                .chunks_exact(self.mask_length)
                .map(eval_01) // s(0) + s(1)
                .sum::<F>()
                * sum_multiple;
            prover_state.prover_field(&mask_sum);
            mask_rlc = prover_state.verifier_field();
        }

        // We do a staggered Sumcheck loop so we can merge the inner fold+compute loops.
        let mut univariate = Vec::new();
        let mut res = Vec::with_capacity(self.num_rounds);
        let mut folding_randomness = None;
        for (round, mask) in chunks_exact_or_empty(masks, self.mask_length, self.num_rounds).enumerate() {
            // Fold and compute sumcheck polynomial in one pass.
            let (c0, c2) = if let Some(w) = folding_randomness {
                fold_and_compute_polynomial(a, b, w)
            } else {
                compute_sumcheck_polynomial(a, b)
            };
            let c1 = *sum - c0.double() - c2;

            // Optionally mask with univariate
            if mask.is_empty() {
                prover_state.prover_fields(&[c0, c2]);
            } else {
                // Initialize to round masking univariate polynomial.
                univariate.clear();
                let sum_multiple = F::from((1u64) << self.num_rounds.saturating_sub(round + 1));
                univariate.extend(mask.iter().map(|m| sum_multiple * *m));

                // Add constant term from previous and future masks.
                univariate[0] += (mask_sum - sum_multiple * eval_01(mask)) * half;

                // Add plain sumcheck polynomial
                univariate[0] += mask_rlc * c0;
                univariate[1] += mask_rlc * c1;
                univariate[2] += mask_rlc * c2;

                prover_state.prover_field(&univariate[0]);
                prover_state.prover_fields(&univariate[2..]);
            }

            // Receive the random evaluation point and update the sum
            self.round_pow.prove(prover_state);
            let r = prover_state.verifier_field::<F>();
            res.push(r);
            *sum = (c2 * r + c1) * r + c0;
            if !masks.is_empty() {
                let masked_sum = univariate_evaluate(&univariate, r);
                mask_sum = masked_sum - mask_rlc * *sum;
            }
            folding_randomness = Some(r);
        }
        if let Some(w) = folding_randomness {
            // Final fold of the inputs (no polynomial computation)
            fold(a, w);
            fold(b, w);
        }

        *sum = mask_sum + mask_rlc * *sum;
        (res, mask_sum, mask_rlc)
    }

    #[cfg_attr(any(), instrument(skip_all))]
    pub fn verify<H>(&self, verifier_state: &mut VerifierState<H>, sum: &mut F) -> VerificationResult<(Vec<F>, F)>
    where
        H: DuplexSpongeInterface,
        FieldElem<F>: Codec<[H::U]>,
        [u8; 32]: Decoding<[H::U]>,
        U64: Codec<[H::U]>,
    {
        assert!(self.num_rounds == 0 || self.initial_size.next_power_of_two() >= 1 << self.num_rounds);
        assert!(self.mask_length == 0 || self.mask_length >= 3);

        let mut mask_rlc = F::one();
        if self.mask_length > 0 && self.num_rounds > 0 {
            let mask_sum: F = verifier_state.prover_field()?;
            mask_rlc = verifier_state.verifier_field();
            *sum = mask_sum + mask_rlc * *sum;
        }

        let mut univariate = vec![F::zero(); self.mask_length.max(3)];
        let mut res = Vec::with_capacity(self.num_rounds);
        for _ in 0..self.num_rounds {
            // Receive all but linear coefficient.
            univariate[0] = verifier_state.prover_field()?;
            for c in &mut univariate[2..] {
                *c = verifier_state.prover_field()?;
            }

            // Derive linear coefficient from relation `univariate(0) + univariate(1) = sum`
            univariate[1] = *sum - univariate[0].double() - univariate[2..].iter().sum::<F>();

            // Check proof of work (if any)
            self.round_pow.verify(verifier_state)?;

            // Receive the random evaluation point
            let folding_randomness = verifier_state.verifier_field::<F>();
            res.push(folding_randomness);

            // Update the sum
            *sum = univariate_evaluate(&univariate, folding_randomness);
        }
        Ok((res, mask_rlc))
    }
}

impl<F: Field> fmt::Display for Config<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "size {} rounds {} pow {:.2} ℓ_zk {}",
            self.initial_size,
            self.num_rounds,
            self.round_pow.difficulty(),
            self.mask_length
        )
    }
}

// Evaluated a univariate as p(0) + p(1)
fn eval_01<F: Field>(coefficients: &[F]) -> F {
    if coefficients.is_empty() {
        return F::zero();
    }
    coefficients[0] + coefficients.iter().sum::<F>()
}

#[cfg(any())]
mod tests {
    // TODO: Proptest based tests checking invariants and post conditions.
    use proptest::{prelude::Just, prop_oneof, proptest, strategy::Strategy};
    use rand::{
        SeedableRng,
        distributions::{Distribution, Standard},
        rngs::StdRng,
    };
    #[cfg(any())]
    use tracing::instrument;

    use super::*;
    use crate::snark::provekit::whir::{
        algebra::{
            fields::{self, Field64},
            multilinear_extend,
            random_vector,
        },
        transcript::DomainSeparator,
        utils::zip_strict,
    };

    impl<F: Field> Config<F> {
        pub fn arbitrary() -> impl Strategy<Value = Self> {
            let mask_length = prop_oneof![
                3 => Just(0_usize),
                7 => 3_usize..100,
            ];
            (0_usize..(1 << 12), 0_usize..12, mask_length).prop_map(|(initial_size, num_rounds, mask_length)| {
                let num_rounds = num_rounds.min(initial_size.next_power_of_two().trailing_zeros() as usize);
                Self {
                    field: Type::new(),
                    initial_size,
                    num_rounds,
                    round_pow: proof_of_work::Config::none(),
                    mask_length,
                }
            })
        }
    }

    #[cfg_attr(any(), instrument)]
    fn test_config<F>(seed: u64, config: &Config<F>)
    where
        F: Field + Codec,
        F: snarkvm_utilities::Uniform,
    {
        // Pseudo-random Instance
        let instance = U64(seed);
        let ds =
            DomainSeparator::protocol(config).session(&format!("Test at {}:{}", file!(), line!())).instance(&instance);
        let mut rng = StdRng::seed_from_u64(seed);
        let initial_vector = random_vector(&mut rng, config.initial_size);
        let initial_covector = random_vector(&mut rng, config.initial_size);
        let initial_sum = dot(&initial_vector, &initial_covector);
        let masks = random_vector(&mut rng, config.mask_length * config.num_rounds);

        // Prover
        let mut vector = initial_vector.clone();
        let mut covector = initial_covector.clone();
        let mut sum = initial_sum;
        let mut prover_state = ProverState::new_std(&ds);
        let (point, mask_sum, mask_rlc) = config.prove(&mut prover_state, &mut vector, &mut covector, &mut sum, &masks);
        let expected_mask_sum =
            zip_strict(chunks_exact_or_empty(&masks, config.mask_length, config.num_rounds), &point)
                .map(|(m, x)| univariate_evaluate(m, *x))
                .sum::<F>();
        assert_eq!(vector.len(), config.final_size());
        assert_eq!(covector.len(), config.final_size());
        assert_eq!(mask_sum, expected_mask_sum);
        assert_eq!(mask_sum + mask_rlc * dot(&vector, &covector), sum);
        if config.final_size() == 1 {
            assert_eq!(multilinear_extend(&initial_vector, &point), vector[0]);
            assert_eq!(multilinear_extend(&initial_covector, &point), covector[0]);
        } else {
            // TODO: Check correct folding.
        }
        let proof = prover_state.proof();

        // Verifier
        let mut verifier_sum = initial_sum;
        let mut verifier_state = VerifierState::new_std(&ds, &proof);
        let (verifier_point, verifier_mask_rlc) = config.verify(&mut verifier_state, &mut verifier_sum).unwrap();
        assert_eq!(verifier_point, point);
        assert_eq!(verifier_mask_rlc, mask_rlc);
        assert_eq!(verifier_sum, sum);
        verifier_state.check_eof().unwrap();
    }

    fn test_sumcheck<F>()
    where
        F: Field + Codec,
        F: snarkvm_utilities::Uniform,
    {
        crate::snark::provekit::whir::tests::init();
        proptest!(|(seed: u64, config in Config::arbitrary())| {
            test_config(seed, &config);
        });
    }

    #[test]
    fn test_single_round() {
        test_config(0, &Config::<Field64> {
            field: Type::new(),
            initial_size: 2,
            round_pow: proof_of_work::Config::none(),
            num_rounds: 1,
            mask_length: 3,
        });
    }

    #[test]
    fn test_two_rounds() {
        test_config(0, &Config::<Field64> {
            field: Type::new(),
            initial_size: 3,
            round_pow: proof_of_work::Config::none(),
            num_rounds: 2,
            mask_length: 3,
        });
    }

    #[test]
    fn test_three_rounds() {
        test_config(0, &Config::<Field64> {
            field: Type::new(),
            initial_size: 5,
            round_pow: proof_of_work::Config::none(),
            num_rounds: 3,
            mask_length: 3,
        });
    }

    #[test]
    fn test_field64_1() {
        test_sumcheck::<fields::Field64>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field64_2() {
        test_sumcheck::<fields::Field64_2>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field64_3() {
        test_sumcheck::<fields::Field64_3>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field128() {
        test_sumcheck::<fields::Field128>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field192() {
        test_sumcheck::<fields::Field192>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field256() {
        test_sumcheck::<fields::Field256>();
    }
}
