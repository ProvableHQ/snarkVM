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
#![allow(clippy::too_many_arguments)]

mod environment;
pub use environment::*;

mod helpers;
pub use helpers::*;

pub mod traits;
pub use traits::*;

pub mod prelude {
    pub use crate::{
        environment::*,
        helpers::*,
        traits::{
            algorithms::*,
            arithmetic::*,
            bitwise::*,
            from_bits::*,
            from_field::*,
            parse::*,
            parse_string::*,
            to_bits_le,
            to_field::*,
            type_name::*,
            types::{
                integer_magnitude::Magnitude,
                integer_type::{
                    CheckedPow,
                    CheckedShl,
                    IntegerProperties,
                    IntegerType,
                    WrappingDiv,
                    WrappingPow,
                    WrappingRem,
                },
                *,
            },
            visibility::*,
            ToBits,
        },
    };

    pub use snarkvm_curves::{AffineCurve, MontgomeryParameters, ProjectiveCurve, TwistedEdwardsParameters};
    pub use snarkvm_fields::{Field as _, PrimeField as _, SquareRootField as _, Zero as _};
    pub use snarkvm_utilities::{
        cfg_chunks,
        cfg_find,
        cfg_find_map,
        cfg_into_iter,
        cfg_iter,
        cfg_iter_mut,
        cfg_keys,
        cfg_reduce,
        cfg_values,
        error,
        has_duplicates,
        io::{Read, Result as IoResult, Write},
        DeserializeExt,
        FromBits as _,
        FromBytes,
        FromBytesDeserializer,
        LimitedWriter,
        TestRng,
        ToBits as _,
        ToBytes,
        ToBytesSerializer,
        Uniform,
    };

    pub use core::{
        cmp::Ordering,
        fmt::{self, Debug, Display, Formatter},
        hash::Hash as _,
        iter::{Product, Sum},
        ops::{
            Add,
            AddAssign,
            BitAnd,
            BitAndAssign,
            BitOr,
            BitOrAssign,
            BitXor,
            BitXorAssign,
            Deref,
            DerefMut,
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
        },
        str::{self, FromStr},
    };

    pub use anyhow::{anyhow, bail, ensure, Error, Result};
    pub use bech32::{self, FromBase32, ToBase32};
    pub use itertools::Itertools;
    pub use nom::{
        branch::alt,
        bytes::{complete::tag, streaming::take},
        character::complete::{alpha1, alphanumeric1, char, one_of},
        combinator::{complete, fail, map, map_res, opt, recognize},
        multi::{count, many0, many0_count, many1, separated_list0, separated_list1},
        sequence::{pair, terminated},
    };
    pub use num_traits::{AsPrimitive, One, Pow, Zero};
    pub use rand::{
        distributions::{Alphanumeric, Distribution, Standard},
        CryptoRng,
        Rng,
    };
    pub use serde::{
        de,
        de::{DeserializeOwned, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor},
        ser::{self, SerializeSeq, SerializeStruct},
        Deserialize,
        Deserializer,
        Serialize,
        Serializer,
    };
}
