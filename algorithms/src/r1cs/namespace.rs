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

use crate::r1cs::{errors::SynthesisError, ConstraintSystem, LinearCombination, Variable};
use snarkvm_fields::Field;

use std::marker::PhantomData;

/// This is a "namespaced" constraint system which borrows a constraint system
/// (pushing a namespace context) and, when dropped, pops out of the namespace context.
pub struct Namespace<'a, F: Field, CS: ConstraintSystem<F>>(pub(super) &'a mut CS, pub(super) PhantomData<F>);

impl<F: Field, CS: ConstraintSystem<F>> ConstraintSystem<F> for Namespace<'_, F, CS> {
    type Root = CS::Root;

    #[inline]
    fn one() -> Variable {
        CS::one()
    }

    #[inline]
    fn alloc<FN, A, AR>(&mut self, annotation: A, f: FN) -> Result<Variable, SynthesisError>
    where
        FN: FnOnce() -> Result<F, SynthesisError>,
        A: FnOnce() -> AR,
        AR: AsRef<str>,
    {
        self.0.alloc(annotation, f)
    }

    #[inline]
    fn alloc_input<FN, A, AR>(&mut self, annotation: A, f: FN) -> Result<Variable, SynthesisError>
    where
        FN: FnOnce() -> Result<F, SynthesisError>,
        A: FnOnce() -> AR,
        AR: AsRef<str>,
    {
        self.0.alloc_input(annotation, f)
    }

    #[inline]
    fn enforce<A, AR, LA, LB, LC>(&mut self, annotation: A, a: LA, b: LB, c: LC)
    where
        A: FnOnce() -> AR,
        AR: AsRef<str>,
        LA: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
        LB: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
        LC: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
    {
        self.0.enforce(annotation, a, b, c)
    }

    // Downstream users who use `namespace` will never interact with these
    // functions and they will never be invoked because the namespace is
    // never a root constraint system.

    #[inline]
    fn push_namespace<NR, N>(&mut self, _: N)
    where
        NR: AsRef<str>,
        N: FnOnce() -> NR,
    {
        panic!("only the root's push_namespace should be called");
    }

    #[inline]
    fn pop_namespace(&mut self) {
        panic!("only the root's pop_namespace should be called");
    }

    #[inline]
    fn get_root(&mut self) -> &mut Self::Root {
        self.0.get_root()
    }

    #[inline]
    fn num_constraints(&self) -> usize {
        self.0.num_constraints()
    }

    #[inline]
    fn num_public_variables(&self) -> usize {
        self.0.num_public_variables()
    }

    #[inline]
    fn num_private_variables(&self) -> usize {
        self.0.num_private_variables()
    }

    #[inline]
    fn is_in_setup_mode(&self) -> bool {
        self.0.is_in_setup_mode()
    }
}

impl<F: Field, CS: ConstraintSystem<F>> Drop for Namespace<'_, F, CS> {
    #[inline]
    fn drop(&mut self) {
        self.get_root().pop_namespace()
    }
}
