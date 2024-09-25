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

#![forbid(unsafe_code)]
#![allow(clippy::type_complexity)]

extern crate snarkvm_circuit_environment_witness;

pub use snarkvm_circuit_environment_witness::rename_selfs;

pub mod canary_circuit;
pub use canary_circuit::*;

pub mod circuit;
pub use circuit::*;

pub mod environment;
pub use environment::*;

pub mod helpers;
pub use helpers::*;

pub mod macros;
#[allow(unused_imports)]
pub use macros::*;

pub mod testnet_circuit;
pub use testnet_circuit::*;

pub mod traits;
pub use traits::*;

pub mod prelude {
    pub use crate::{
        count,
        count_is,
        count_less_than,
        output_mode,
        rename_selfs,
        traits::*,
        witness,
        witness_mode,
        CircuitType,
        Count,
        Environment,
        LinearCombination,
        Mode,
        OutputMode,
        Variable,
    };
    pub use console::{
        prelude::{
            bail,
            ensure,
            fmt,
            has_duplicates,
            Debug,
            Display,
            Error,
            Formatter,
            FromStr,
            One as _,
            Result,
            Zero as _,
        },
        traits::{
            string_parser,
            types::{
                integer_magnitude::Magnitude,
                integer_type::{CheckedPow, IntegerProperties, IntegerType, WrappingDiv, WrappingPow, WrappingRem},
            },
            Double as _,
            FromBits as _,
            Inverse as _,
            Square as _,
            SquareRoot as _,
            ToBits as _,
        },
        Parser,
        ParserResult,
        TypeName,
    };
    pub use snarkvm_fields::{self, Field as _, PrimeField, Zero as _};

    #[cfg(debug_assertions)]
    pub use snarkvm_curves::AffineCurve as _;

    pub use core::ops::{
        Add,
        AddAssign,
        BitAnd,
        BitAndAssign,
        BitOr,
        BitOrAssign,
        BitXor,
        BitXorAssign,
        Deref,
        Div,
        DivAssign,
        Mul,
        MulAssign,
        Neg,
        Not,
        Rem,
        RemAssign,
        Shl,
        ShlAssign,
        Shr,
        ShrAssign,
        Sub,
        SubAssign,
    };
    pub use indexmap::IndexMap;
    pub use itertools::Itertools;
    pub use nom::{
        branch::alt,
        bytes::complete::tag,
        character::complete::{alpha1, alphanumeric1, char, one_of},
        combinator::{map, map_res, opt, recognize},
        multi::{many0, many1},
        sequence::{pair, terminated},
    };
    pub use num_traits::{self, Inv, One as NumOne, Pow, Unsigned};
    pub use once_cell::unsync::OnceCell;
}
