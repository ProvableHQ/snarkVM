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

use crate::r1cs::{errors::SynthesisError, ConstraintSystem, Index, LinearCombination, Variable};
use snarkvm_fields::Field;

/// Constraint counter for testing purposes.
#[derive(Default)]
pub struct ConstraintCounter {
    pub num_public_variables: usize,
    pub num_private_variables: usize,
    pub num_constraints: usize,
}

impl<ConstraintF: Field> ConstraintSystem<ConstraintF> for ConstraintCounter {
    type Root = Self;

    fn alloc<F, A, AR>(&mut self, _: A, _: F) -> Result<Variable, SynthesisError>
    where
        F: FnOnce() -> Result<ConstraintF, SynthesisError>,
        A: FnOnce() -> AR,
        AR: AsRef<str>,
    {
        let var = Variable::new_unchecked(Index::Private(self.num_private_variables));
        self.num_private_variables += 1;
        Ok(var)
    }

    fn alloc_input<F, A, AR>(&mut self, _: A, _: F) -> Result<Variable, SynthesisError>
    where
        F: FnOnce() -> Result<ConstraintF, SynthesisError>,
        A: FnOnce() -> AR,
        AR: AsRef<str>,
    {
        let var = Variable::new_unchecked(Index::Public(self.num_public_variables));
        self.num_public_variables += 1;

        Ok(var)
    }

    fn enforce<A, AR, LA, LB, LC>(&mut self, _: A, _: LA, _: LB, _: LC)
    where
        A: FnOnce() -> AR,
        AR: AsRef<str>,
        LA: FnOnce(LinearCombination<ConstraintF>) -> LinearCombination<ConstraintF>,
        LB: FnOnce(LinearCombination<ConstraintF>) -> LinearCombination<ConstraintF>,
        LC: FnOnce(LinearCombination<ConstraintF>) -> LinearCombination<ConstraintF>,
    {
        self.num_constraints += 1;
    }

    fn push_namespace<NR, N>(&mut self, _: N)
    where
        NR: AsRef<str>,
        N: FnOnce() -> NR,
    {
    }

    fn pop_namespace(&mut self) {}

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
        true
    }
}
