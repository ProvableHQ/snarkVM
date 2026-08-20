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

//! Base Case Linear Opening Protocol
//!
//! It support honest verifier zero-knowledge (HVZK), but is not succinct.
//!
//! <https://eprint.iacr.org/2026/391.pdf> § 7.

use serde::{Deserialize, Serialize};
use snarkvm_fields::Field;
use spongefish::{Decoding, VerificationResult};

use crate::snark::provekit::whir::{
    algebra::{dot, embedding::Identity, multilinear_extend, scalar_mul_add_new, univariate_evaluate},
    hash::Hash,
    protocols::{irs_commit, sumcheck},
    transcript::{
        Codec,
        DuplexSpongeInterface,
        FieldElem,
        ProverMessage,
        ProverState,
        VerifierMessage,
        VerifierState,
        codecs::U64,
    },
    utils::zip_strict,
    verify,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Config<F: Field> {
    pub commit: irs_commit::Config<Identity<F>>,
    pub sumcheck: sumcheck::Config<F>,

    /// Whether to mask the vectors, whichs adds HVZK.
    pub masked: bool,
}

impl<F: Field> Config<F> {
    pub const fn size(&self) -> usize {
        self.sumcheck.initial_size
    }

    pub fn prove<H>(
        &self,
        prover_state: &mut ProverState<H>,
        mut vector: Vec<F>,
        witness: &irs_commit::Witness<F>,
        mut covector: Vec<F>,
        mut sum: F,
    ) -> (Vec<F>, F)
    where
        H: DuplexSpongeInterface,
        FieldElem<F>: Codec<[H::U]>,
        u8: Decoding<[H::U]>,
        [u8; 32]: Decoding<[H::U]>,
        U64: Codec<[H::U]>,
        Hash: ProverMessage<[H::U]>,
    {
        assert_eq!(self.commit.interleaving_depth, 1);
        assert_eq!(self.commit.num_vectors, 1);
        assert_eq!(self.commit.vector_size, self.sumcheck.initial_size);
        assert_eq!(self.sumcheck.final_size(), 1.min(self.commit.vector_size));
        debug_assert_eq!(dot(&vector, &covector), sum);
        if self.size() == 0 {
            return (Vec::new(), F::zero());
        }

        // Even more trivial non-zk protocol: send f an r directly.
        if !self.masked {
            prover_state.prover_fields(&vector);
            prover_state.prover_fields(&witness.masks);
            let _ = self.commit.open(prover_state, &[witness]);
            let point = self.sumcheck.prove(prover_state, &mut vector, &mut covector, &mut sum, &[]).0;
            assert!(!vector[0].is_zero(), "Proof failed");
            return (point, covector[0]);
        }

        // Create masking vector.
        let mask = prover_state.random_field_vec(vector.len());

        // Commit to the masking vector.
        let mask_witness = self.commit.commit(prover_state, &[&mask]);

        // Compute and send linear form of mask (μ' in paper).
        let mask_sum = dot(&mask, &covector);
        prover_state.prover_field(&mask_sum);

        // RLC the mask with the vector
        let mask_rlc = prover_state.verifier_field::<F>();
        assert!(!mask_rlc.is_zero(), "Proof failed");
        let mut masked_vector = scalar_mul_add_new(&mask, mask_rlc, &vector);
        prover_state.prover_fields(&masked_vector);

        // Send combined IRS randomness. (r^* in paper)
        let masked_masks = scalar_mul_add_new(&mask_witness.masks, mask_rlc, &witness.masks);
        prover_state.prover_fields(&masked_masks);

        // Open the commitment and mask simultaneously.
        let _ = self.commit.open(prover_state, &[&mask_witness, witness]);

        // Run sumcheck to reduce linear form claim
        let mut masked_sum = mask_sum + mask_rlc * sum;
        let point = self.sumcheck.prove(prover_state, &mut masked_vector, &mut covector, &mut masked_sum, &[]).0;

        // If the MLE of `masked_vector` evaluates to zero, the verifier can not
        // proceed. Basically the sumcheck equation has degenerated to 0 * l(r)
        // = 0, which provides no constraints on l(r) that the verifier can
        // return. This event is cryptographically unlikely as `F` is challenge
        // sized.
        assert!(!masked_vector[0].is_zero(), "Proof failed");

        // Return evaluation point and value of the covector.
        (point, covector[0])
    }

    pub fn verify<H>(
        &self,
        verifier_state: &mut VerifierState<H>,
        commitment: &irs_commit::Commitment<F>,
        mut sum: F,
    ) -> VerificationResult<(Vec<F>, F)>
    where
        H: DuplexSpongeInterface,
        FieldElem<F>: Codec<[H::U]>,
        u8: Decoding<[H::U]>,
        [u8; 32]: Decoding<[H::U]>,
        U64: Codec<[H::U]>,
        Hash: ProverMessage<[H::U]>,
    {
        assert_eq!(self.commit.interleaving_depth, 1);
        assert_eq!(self.commit.num_vectors, 1);
        assert_eq!(self.commit.vector_size, self.sumcheck.initial_size);
        assert_eq!(self.sumcheck.final_size(), 1.min(self.commit.vector_size));
        if self.size() == 0 {
            return Ok((Vec::new(), F::zero()));
        }

        // Unmasked protocol
        if !self.masked {
            let vector = verifier_state.prover_fields_vec(self.commit.vector_size)?;
            let masks = verifier_state.prover_fields_vec(self.commit.mask_length * self.commit.num_messages())?;
            let evals = self.commit.verify(verifier_state, &[commitment])?;
            let point = self.sumcheck.verify(verifier_state, &mut sum)?.0;

            for (&point, value) in zip_strict(&evals.points, evals.values(&[F::one()])) {
                // We expected `f(x) + x^l · g(x)` where l = deg(f) + 1, f is the message and g
                // the mask.
                let expected = univariate_evaluate(&vector, point)
                    + point.pow([self.commit.message_length() as u64]) * univariate_evaluate(&masks, point);
                verify!(value == expected);
            }
            let mle = multilinear_extend(&vector, &point);
            verify!(!mle.is_zero());
            let linear_mle = sum / mle;
            return Ok((point, linear_mle));
        }

        let mask_commitment = self.commit.receive_commitment(verifier_state)?;
        let mask_sum: F = verifier_state.prover_field()?;
        let mask_rlc: F = verifier_state.verifier_field();
        verify!(!mask_rlc.is_zero());
        let masked_vector: Vec<F> = verifier_state.prover_fields_vec(self.commit.vector_size)?;
        let masked_masks: Vec<F> = verifier_state.prover_fields_vec(self.commit.mask_length)?;

        // Open the commitment and mask simultaneously.
        let evals = self.commit.verify(verifier_state, &[&mask_commitment, commitment])?;

        // Spot check evaluations.
        for (&point, value) in zip_strict(&evals.points, evals.values(&[F::one(), mask_rlc])) {
            // We expected `f(x) + x^l · g(x)` where l = deg(f) + 1, f is the message and g
            // the mask.
            let expected = univariate_evaluate(&masked_vector, point)
                + point.pow([self.commit.message_length() as u64]) * univariate_evaluate(&masked_masks, point);
            verify!(value == expected);
        }

        // Sumcheck on masked inner product
        let mut masked_sum = mask_sum + mask_rlc * sum;
        let point = self.sumcheck.verify(verifier_state, &mut masked_sum)?.0;

        // Compute implied MLE of the linear form
        // f*(r) · l(r) = sum  =>  l(r) = sum / f*(r)
        let masked_mle = multilinear_extend(&masked_vector, &point);
        verify!(!masked_mle.is_zero());
        let linear_mle = masked_sum / masked_mle;

        Ok((point, linear_mle))
    }
}

#[cfg(any())]
mod tests {
    use proptest::{bool, prelude::Strategy, proptest};
    use rand::{SeedableRng, rngs::StdRng};
    #[cfg(any())]
    use tracing::instrument;

    use super::*;
    use crate::snark::provekit::whir::{
        algebra::fields,
        protocols::proof_of_work,
        transcript::DomainSeparator,
        type_info::Type,
    };

    impl<F: Field> Config<F> {
        pub fn arbitrary(size: usize, mask_length: usize) -> impl Strategy<Value = Self> {
            let commit = irs_commit::Config::arbitrary(Identity::<F>::new(), 1, size, mask_length, 1);
            (commit, bool::weighted(0.8)).prop_map(move |(commit, masked)| Self {
                commit: irs_commit::Config { out_domain_samples: 0, ..commit },
                sumcheck: sumcheck::Config {
                    field: Type::new(),
                    initial_size: size,
                    round_pow: proof_of_work::Config::none(),
                    num_rounds: size.next_power_of_two().trailing_zeros() as usize,
                    mask_length: 0,
                },
                masked,
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
        let vector = random_vector(&mut rng, config.size());
        let covector = random_vector(&mut rng, config.size());
        let sum = dot(&vector, &covector);

        // Prover
        let mut prover_state = ProverState::new_std(&ds);
        let witness = config.commit.commit(&mut prover_state, &[&vector]);
        let (point, value) = config.prove(&mut prover_state, vector.clone(), &witness, covector.clone(), sum);
        assert_eq!(multilinear_extend(&covector, &point), value);
        let proof = prover_state.proof();

        // Verifier
        let mut verifier_state = VerifierState::new_std(&ds, &proof);
        let commitment = config.commit.receive_commitment(&mut verifier_state).unwrap();
        let (verifier_point, verifier_value) = config.verify(&mut verifier_state, &commitment, sum).unwrap();
        assert_eq!(verifier_point, point);
        assert_eq!(verifier_value, value);
        verifier_state.check_eof().unwrap();
    }

    fn test<F: Field + Codec>()
    where
        F: snarkvm_utilities::Uniform,
    {
        crate::snark::provekit::whir::tests::init();
        let configs = (0_usize..1 << 10, 0_usize..1 << 10)
            .prop_flat_map(|(size, mask_length)| Config::arbitrary(size, mask_length));
        proptest!(|(seed: u64, config in configs)| {
            test_config(seed, &config);
        });
    }

    #[test]
    fn test_field64_1() {
        test::<fields::Field64>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field64_2() {
        test::<fields::Field64_2>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field64_3() {
        test::<fields::Field64_3>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field128() {
        test::<fields::Field128>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field192() {
        test::<fields::Field192>();
    }

    #[test]
    #[ignore = "Somewhat expensive and redundant"]
    fn test_field256() {
        test::<fields::Field256>();
    }
}
