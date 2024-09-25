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

use crate::Mode;

/// Operations to inject from a primitive form into a circuit environment.
pub trait Inject {
    type Primitive;

    ///
    /// Initializes a circuit of the given mode and primitive value.
    ///
    fn new(mode: Mode, value: Self::Primitive) -> Self;

    ///
    /// Initializes a constant of the given primitive value.
    ///
    fn constant(value: Self::Primitive) -> Self
    where
        Self: Sized,
    {
        Self::new(Mode::Constant, value)
    }
}

/********************/
/***** IndexMap *****/
/********************/

impl<C0: Eq + core::hash::Hash + Inject<Primitive = P0>, C1: Inject<Primitive = P1>, P0, P1> Inject
    for indexmap::IndexMap<C0, C1>
{
    type Primitive = indexmap::IndexMap<P0, P1>;

    #[inline]
    fn new(mode: Mode, value: Self::Primitive) -> Self {
        value.into_iter().map(|(v0, v1)| (C0::new(mode, v0), C1::new(mode, v1))).collect()
    }
}

/********************/
/****** Arrays ******/
/********************/

impl<C: Inject<Primitive = P>, P> Inject for Vec<C> {
    type Primitive = Vec<P>;

    #[inline]
    fn new(mode: Mode, value: Self::Primitive) -> Self {
        value.into_iter().map(|v| C::new(mode, v)).collect()
    }
}

/********************/
/****** Tuples ******/
/********************/

/// A helper macro to implement `Inject` for a tuple of `Inject` circuits.
macro_rules! inject_tuple {
    (($t0:ident, 0), $(($ty:ident, $idx:tt)),*) => {
        impl<$t0: Inject, $($ty: Inject),*> Inject for ($t0, $($ty),*) {
            type Primitive = ($t0::Primitive, $( $ty::Primitive ),*);

            #[inline]
            fn new(mode: Mode, value: Self::Primitive) -> Self {
                ($t0::new(mode, value.0), $($ty::new(mode, value.$idx)),*)
            }
        }
    }
}

inject_tuple!((C0, 0), (C1, 1));
inject_tuple!((C0, 0), (C1, 1), (C2, 2));
inject_tuple!((C0, 0), (C1, 1), (C2, 2), (C3, 3));
inject_tuple!((C0, 0), (C1, 1), (C2, 2), (C3, 3), (C4, 4));
