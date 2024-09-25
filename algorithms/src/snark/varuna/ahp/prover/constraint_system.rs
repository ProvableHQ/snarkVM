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

use crate::r1cs::{errors::SynthesisError, ConstraintSystem as CS, Index as VarIndex, LinearCombination, Variable};
use snarkvm_fields::Field;

pub(crate) struct ConstraintSystem<F: Field> {
    pub(crate) public_variables: Vec<F>,
    pub(crate) private_variables: Vec<F>,
    pub(crate) num_public_variables: usize,
    pub(crate) num_private_variables: usize,
    pub(crate) num_constraints: usize,
}

impl<F: Field> ConstraintSystem<F> {
    pub(crate) fn new() -> Self {
        Self {
            public_variables: vec![F::one()],
            private_variables: Vec::new(),
            num_public_variables: 1usize,
            num_private_variables: 0usize,
            num_constraints: 0usize,
        }
    }

    /// Formats the public input according to the requirements of the constraint
    /// system
    pub(crate) fn format_public_input(public_input: &[F]) -> Vec<F> {
        let mut input = Vec::with_capacity(1 + public_input.len());
        input.push(F::one());
        input.extend_from_slice(public_input);
        input
    }

    /// Takes in a previously formatted public input and removes the formatting
    /// imposed by the constraint system.
    pub(crate) fn unformat_public_input(input: &[F]) -> Vec<F> {
        input[1..].to_vec()
    }
}

impl<F: Field> CS<F> for ConstraintSystem<F> {
    type Root = Self;

    #[inline]
    fn alloc<Fn, A, AR>(&mut self, _: A, f: Fn) -> Result<Variable, SynthesisError>
    where
        Fn: FnOnce() -> Result<F, SynthesisError>,
        A: FnOnce() -> AR,
        AR: AsRef<str>,
    {
        let index = self.num_private_variables;
        self.num_private_variables += 1;

        self.private_variables.push(f()?);
        Ok(Variable::new_unchecked(VarIndex::Private(index)))
    }

    #[inline]
    fn alloc_input<Fn, A, AR>(&mut self, _: A, f: Fn) -> Result<Variable, SynthesisError>
    where
        Fn: FnOnce() -> Result<F, SynthesisError>,
        A: FnOnce() -> AR,
        AR: AsRef<str>,
    {
        let index = self.num_public_variables;
        self.num_public_variables += 1;

        self.public_variables.push(f()?);
        Ok(Variable::new_unchecked(VarIndex::Public(index)))
    }

    #[inline]
    fn enforce<A, AR, LA, LB, LC>(&mut self, _: A, _: LA, _: LB, _: LC)
    where
        A: FnOnce() -> AR,
        AR: AsRef<str>,
        LA: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
        LB: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
        LC: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
    {
        self.num_constraints += 1;
    }

    fn push_namespace<NR, N>(&mut self, _: N)
    where
        NR: AsRef<str>,
        N: FnOnce() -> NR,
    {
        // Do nothing; we don't care about namespaces in this context.
    }

    fn pop_namespace(&mut self) {
        // Do nothing; we don't care about namespaces in this context.
    }

    fn get_root(&mut self) -> &mut Self::Root {
        self
    }

    fn num_constraints(&self) -> usize {
        self.num_constraints
    }

    fn num_public_variables(&self) -> usize {
        self.num_public_variables
    }

    fn num_private_variables(&self) -> usize {
        self.num_private_variables
    }

    fn is_in_setup_mode(&self) -> bool {
        false
    }
}
