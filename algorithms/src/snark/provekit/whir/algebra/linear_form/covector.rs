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
use crate::snark::provekit::whir::algebra::{Embedding, mixed_dot, multilinear_extend, scalar_mul_add};

/// Linear form as an explicit covector over the field.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Covector<F: Field> {
    pub vector: Vec<F>,
}

impl<F: Field> LinearForm<F> for Covector<F> {
    fn size(&self) -> usize {
        self.vector.len()
    }

    fn mle_evaluate(&self, point: &[F]) -> F {
        multilinear_extend(&self.vector, point)
    }

    fn accumulate(&self, accumulator: &mut [F], scalar: F) {
        scalar_mul_add(accumulator, scalar, &self.vector);
    }
}

impl<F: Field> Covector<F> {
    pub const fn new(vector: Vec<F>) -> Self {
        Self { vector }
    }

    /// Any [`LinearForm<F>`] can be converted to a [`Covector<F>`].
    pub fn from(linear_form: &dyn LinearForm<F>) -> Self {
        let mut vector = vec![F::zero(); linear_form.size()];
        linear_form.accumulate(&mut vector, F::one());
        Self { vector }
    }
}

impl<M: Embedding> Evaluate<M> for Covector<M::Target> {
    fn evaluate(&self, embedding: &M, vector: &[M::Source]) -> M::Target {
        assert_eq!(self.vector.len(), vector.len());
        mixed_dot(embedding, &self.vector, vector)
    }
}
