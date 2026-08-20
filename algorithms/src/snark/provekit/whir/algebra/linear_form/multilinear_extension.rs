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

use snarkvm_fields::Field;

use super::{Evaluate, LinearForm};
use crate::snark::provekit::whir::{
    algebra::{Embedding, eval_eq, mixed_multilinear_extend},
    utils::zip_strict,
};

/// Multilinear extension evaluation as a linear form $𝔽^n → 𝔽$.
///
/// Given a multilinear function $f ∈ 𝔽^(≤ 1)[X_0,…,X_(k-1)]$ represented by a
/// vector $v ∈ 𝔽^n$ with $n = 2^k$ using the boolean hypercube evaluation basis
/// such that $v_i = f( bits(i) )$ where $bits: ℕ → {0,1}^k$ is the
/// little-endian binary decomposition, then this linear form will evaluate to
/// $f(x)$ for some fixed point $x ∈ 𝔽^k$.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MultilinearExtension<F: Field> {
    pub point: Vec<F>,
}

impl<F: Field> MultilinearExtension<F> {
    pub const fn new(point: Vec<F>) -> Self {
        Self { point }
    }
}

impl<F: Field> LinearForm<F> for MultilinearExtension<F> {
    fn size(&self) -> usize {
        1 << self.point.len()
    }

    fn mle_evaluate(&self, point: &[F]) -> F {
        zip_strict(&self.point, point).fold(F::one(), |acc, (&l, &r)| acc * (l * r + (F::one() - l) * (F::one() - r)))
    }

    fn accumulate(&self, accumulator: &mut [F], scalar: F) {
        eval_eq(accumulator, &self.point, scalar);
    }
}

impl<M: Embedding> Evaluate<M> for MultilinearExtension<M::Target> {
    fn evaluate(&self, embedding: &M, vector: &[M::Source]) -> M::Target {
        mixed_multilinear_extend(embedding, vector, &self.point)
    }
}
