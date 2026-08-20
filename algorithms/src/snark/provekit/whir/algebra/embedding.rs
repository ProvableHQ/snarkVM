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

//! Traits and types for field morphisms.

use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use snarkvm_fields::Field;

use crate::snark::provekit::whir::type_info::{Type, TypeInfo, Typed};

/// Trait for a type representing a unital field homomorphism.
///
/// An `Embedding<Source = F, Target = G>` must satisfy
///
/// - `e.map(F::one()) == G::ONE`
/// - `e.map(a + b) == e.map(a) + e.map(b)`
/// - `e.map(a * b) == e.map(a) * e.map(b)`
/// - `e.mixed_add(a, b) == a + e.map(b)`
/// - `e.mixed_mul(a, b) == a * e.map(b)`
pub trait Embedding:
    Clone + PartialEq + Eq + Debug + TypeInfo + Serialize + for<'de> Deserialize<'de> + Send + Sync
{
    type Source: Field;
    type Target: Field;

    fn map(&self, dom: Self::Source) -> Self::Target;

    #[inline]
    fn mixed_mul(&self, cod: Self::Target, dom: Self::Source) -> Self::Target {
        cod * self.map(dom)
    }

    #[inline]
    fn mixed_add(&self, cod: Self::Target, dom: Self::Source) -> Self::Target {
        cod + self.map(dom)
    }

    fn map_vec(&self, source: Vec<Self::Source>) -> Vec<Self::Target> {
        source.into_iter().map(|e| self.map(e)).collect()
    }
}

/// The identity embedding of a field in into itself.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Identity<F: Field> {
    field: Type<F>,
}

/// The basefield embedding morphism.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Basefield<F: Field> {
    extension: Type<F>,
}

/// The Frobenius automorphism.
///
/// It demonstrates that embeddings are not uniquely defined by their source
/// and target fields alone. Hence they need to be objects in their own right.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Frobenius<F: Field> {
    field: Type<F>,
    power: u64,
}

/// The composition of two morphisms.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Compose<A: Embedding, B: Embedding<Source = A::Target>> {
    inner: Typed<A>,
    outer: Typed<B>,
}

impl<F: Field> Identity<F> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<F: Field> Basefield<F> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<F: Field> Frobenius<F> {
    pub fn new(power: u64) -> Self {
        Self { field: Type::new(), power }
    }
}

impl<A, B> Compose<A, B>
where
    A: Embedding,
    B: Embedding<Source = A::Target>,
{
    pub const fn new(inner: A, outer: B) -> Self {
        Self { inner: Typed::new(inner), outer: Typed::new(outer) }
    }

    pub fn inner(&self) -> &A {
        &self.inner
    }

    pub fn outer(&self) -> &B {
        &self.outer
    }
}

impl<F: Field> Embedding for Identity<F> {
    type Source = F;
    type Target = F;

    #[inline]
    fn map(&self, dom: Self::Source) -> Self::Target {
        dom
    }

    #[inline]
    fn map_vec(&self, source: Vec<F>) -> Vec<F> {
        source
    }
}

impl<F: Field> Embedding for Basefield<F> {
    type Source = F::BasePrimeField;
    type Target = F;

    #[inline]
    fn map(&self, dom: Self::Source) -> Self::Target {
        Self::Target::from_base_prime_field(dom)
    }
}

impl<F: Field> Embedding for Frobenius<F> {
    type Source = F;
    type Target = F;

    #[inline]
    fn map(&self, mut dom: Self::Source) -> Self::Target {
        dom.frobenius_map(self.power as usize);
        dom
    }
}

impl<A, B> Embedding for Compose<A, B>
where
    A: Embedding,
    B: Embedding<Source = A::Target>,
{
    type Source = A::Source;
    type Target = B::Target;

    #[inline]
    fn map(&self, dom: Self::Source) -> Self::Target {
        self.outer.map(self.inner.map(dom))
    }

    #[inline]
    fn mixed_add(&self, cod: Self::Target, dom: Self::Source) -> Self::Target {
        self.outer.mixed_add(cod, self.inner.map(dom))
    }

    #[inline]
    fn mixed_mul(&self, cod: Self::Target, dom: Self::Source) -> Self::Target {
        self.outer.mixed_mul(cod, self.inner.map(dom))
    }
}

impl<F: Field> TypeInfo for Identity<F> {
    type Info = String;

    fn type_info() -> Self::Info {
        "identity".into()
    }
}

impl<F: Field> TypeInfo for Basefield<F> {
    type Info = String;

    fn type_info() -> Self::Info {
        "basefield".into()
    }
}

impl<F: Field> TypeInfo for Frobenius<F> {
    type Info = String;

    fn type_info() -> Self::Info {
        "frobenius".into()
    }
}

impl<A: Embedding, B: Embedding<Source = A::Target>> TypeInfo for Compose<A, B> {
    type Info = String;

    fn type_info() -> Self::Info {
        "compose".into()
    }
}

#[cfg(any())]
pub(crate) mod tests {
    use proptest::{
        collection,
        prelude::{Just, Strategy, any},
        proptest,
        strategy,
    };
    use snarkvm_fields::{Field, One, PrimeField, Zero};

    use super::*;
    use crate::snark::provekit::whir::algebra::{embedding::Embedding, fields};

    pub fn arb_prime_field<F: PrimeField>() -> impl Strategy<Value = F> {
        let nbytes = F::MODULUS_BIT_SIZE.div_ceil(8) as usize;
        let rand = collection::vec(any::<u8>(), nbytes).prop_map(|bytes| F::from_le_bytes_mod_order(&bytes));
        strategy::Union::new_weighted(vec![
            (1, Just(F::zero()).boxed()),
            (1, Just(F::one()).boxed()),
            (1, Just(-F::one()).boxed()),
            (3, rand.boxed()),
        ])
    }

    pub fn arb_field<F: Field>() -> impl Strategy<Value = F> {
        collection::vec(arb_prime_field::<F::BasePrimeField>(), F::extension_degree() as usize)
            .prop_map(|elements| F::from_base_prime_field_elems(elements).unwrap())
    }

    pub fn test_embedding<E: Embedding>(e: &E) {
        assert_eq!(e.map(E::Source::ZERO), E::Target::ZERO);
        assert_eq!(e.map(E::Source::ONE), E::Target::ONE);
        proptest!(|(a in arb_field(), b in arb_field())| {
            assert_eq!(e.map(a) + e.map(b), e.map(a + b));
        });
        proptest!(|(a in arb_field(), b in arb_field())| {
            assert_eq!(e.map(a) * e.map(b), e.map(a * b));
        });
        proptest!(|(a in arb_field(), b in arb_field())| {
            assert_eq!(e.mixed_add(a, b), a + e.map(b));
        });
        proptest!(|(a in arb_field(), b in arb_field())| {
            assert_eq!(e.mixed_mul(a, b), a * e.map(b));
        });
    }

    #[test]
    fn test_field64_3() {
        test_embedding(&Identity::<fields::Field64_3>::new());
        test_embedding(&Basefield::<fields::Field64_3>::new());
        test_embedding(&Frobenius::<fields::Field64_3>::new(0));
        test_embedding(&Frobenius::<fields::Field64_3>::new(1));
        test_embedding(&Frobenius::<fields::Field64_3>::new(2));
    }
}
