// Copyright (c) 2019-2025 Provable Inc.
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

mod bytes;
mod encrypt;
mod equal;
mod find;
mod from_bits;
mod from_fields;
mod num_randomizers;
mod parse;
mod serialize;
mod size_in_fields;
mod to_bits;
mod to_fields;

use crate::{Access, Ciphertext, Identifier, Literal};
use snarkvm_console_network::Network;
use snarkvm_console_types::prelude::*;

use indexmap::IndexMap;
use std::sync::OnceLock;

#[derive(Clone)]
pub enum Plaintext<N: Network> {
    /// A literal.
    Literal(Literal<N>, OnceLock<Vec<bool>>),
    /// A struct.
    Struct(IndexMap<Identifier<N>, Plaintext<N>>, OnceLock<Vec<bool>>),
    /// An array.
    Array(Vec<Plaintext<N>>, OnceLock<Vec<bool>>),
}

impl<N: Network> From<Literal<N>> for Plaintext<N> {
    /// Returns a new `Plaintext` from a `Literal`.
    fn from(literal: Literal<N>) -> Self {
        Self::Literal(literal, OnceLock::new())
    }
}

impl<N: Network> From<&Literal<N>> for Plaintext<N> {
    /// Returns a new `Plaintext` from a `&Literal`.
    fn from(literal: &Literal<N>) -> Self {
        Self::Literal(literal.clone(), OnceLock::new())
    }
}

// A macro that derives implementations of `From` for arrays of a plaintext literals of various sizes.
macro_rules! impl_plaintext_from_array {
    ($element:ident, $($size:literal),+) => {
        $(
            impl<N: Network> From<[$element<N>; $size]> for Plaintext<N> {
                fn from(value: [$element<N>; $size]) -> Self {
                    Self::Array(
                        value
                            .into_iter()
                            .map(|element| Plaintext::from(Literal::$element(element)))
                            .collect(),
                        OnceLock::new(),
                    )
                }
            }
        )+
    };
}

// Implement for `[U8<N>, SIZE]` for sizes 1 through 32.
impl_plaintext_from_array!(
    U8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30,
    31, 32
);

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;
    use snarkvm_console_types::Field;

    use core::str::FromStr;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_plaintext() -> Result<()> {
        let run_test = |value: Plaintext<CurrentNetwork>| {
            assert_eq!(
                value.to_bits_le(),
                Plaintext::<CurrentNetwork>::from_bits_le(&value.to_bits_le()).unwrap().to_bits_le()
            );
            assert_eq!(value, Plaintext::<CurrentNetwork>::from_fields(&value.to_fields().unwrap()).unwrap());
            assert_eq!(value, Plaintext::<CurrentNetwork>::from_str(&value.to_string()).unwrap());
            assert!(*value.is_equal(&value));
            assert!(*!value.is_not_equal(&value));
        };

        let mut rng = TestRng::default();

        // Test booleans.
        run_test(Plaintext::<CurrentNetwork>::from_str("true")?);
        run_test(Plaintext::<CurrentNetwork>::from_str("false")?);

        // Test a random field element.
        run_test(Plaintext::<CurrentNetwork>::Literal(
            Literal::Field(Field::new(Uniform::rand(&mut rng))),
            OnceLock::new(),
        ));

        // Test a random struct with literal members.
        run_test(Plaintext::<CurrentNetwork>::Struct(
            IndexMap::from_iter(vec![
                (Identifier::from_str("a")?, Plaintext::<CurrentNetwork>::from_str("true")?),
                (
                    Identifier::from_str("b")?,
                    Plaintext::<CurrentNetwork>::Literal(
                        Literal::Field(Field::new(Uniform::rand(&mut rng))),
                        OnceLock::new(),
                    ),
                ),
            ]),
            OnceLock::new(),
        ));

        // Test a random struct with array members.
        run_test(Plaintext::<CurrentNetwork>::Struct(
            IndexMap::from_iter(vec![
                (Identifier::from_str("a")?, Plaintext::<CurrentNetwork>::from_str("true")?),
                (
                    Identifier::from_str("b")?,
                    Plaintext::<CurrentNetwork>::Array(
                        vec![
                            Plaintext::<CurrentNetwork>::from_str("true")?,
                            Plaintext::<CurrentNetwork>::from_str("false")?,
                        ],
                        OnceLock::new(),
                    ),
                ),
            ]),
            OnceLock::new(),
        ));

        // Test random deeply-nested struct.
        run_test(Plaintext::<CurrentNetwork>::Struct(
            IndexMap::from_iter(vec![
                (Identifier::from_str("a")?, Plaintext::<CurrentNetwork>::from_str("true")?),
                (
                    Identifier::from_str("b")?,
                    Plaintext::<CurrentNetwork>::Struct(
                        IndexMap::from_iter(vec![
                            (Identifier::from_str("c")?, Plaintext::<CurrentNetwork>::from_str("true")?),
                            (
                                Identifier::from_str("d")?,
                                Plaintext::<CurrentNetwork>::Struct(
                                    IndexMap::from_iter(vec![
                                        (Identifier::from_str("e")?, Plaintext::<CurrentNetwork>::from_str("true")?),
                                        (
                                            Identifier::from_str("f")?,
                                            Plaintext::<CurrentNetwork>::Literal(
                                                Literal::Field(Field::new(Uniform::rand(&mut rng))),
                                                OnceLock::new(),
                                            ),
                                        ),
                                    ]),
                                    OnceLock::new(),
                                ),
                            ),
                            (
                                Identifier::from_str("g")?,
                                Plaintext::Array(
                                    vec![
                                        Plaintext::<CurrentNetwork>::from_str("true")?,
                                        Plaintext::<CurrentNetwork>::from_str("false")?,
                                    ],
                                    OnceLock::new(),
                                ),
                            ),
                        ]),
                        OnceLock::new(),
                    ),
                ),
                (
                    Identifier::from_str("h")?,
                    Plaintext::<CurrentNetwork>::Literal(
                        Literal::Field(Field::new(Uniform::rand(&mut rng))),
                        OnceLock::new(),
                    ),
                ),
            ]),
            OnceLock::new(),
        ));

        // Test an array of literals.
        run_test(Plaintext::<CurrentNetwork>::Array(
            vec![
                Plaintext::<CurrentNetwork>::from_str("0field")?,
                Plaintext::<CurrentNetwork>::from_str("1field")?,
                Plaintext::<CurrentNetwork>::from_str("2field")?,
                Plaintext::<CurrentNetwork>::from_str("3field")?,
                Plaintext::<CurrentNetwork>::from_str("4field")?,
            ],
            OnceLock::new(),
        ));

        // Test an array of structs.
        run_test(Plaintext::<CurrentNetwork>::Array(
            vec![
                Plaintext::<CurrentNetwork>::from_str("{ x: 0field, y: 1field }")?,
                Plaintext::<CurrentNetwork>::from_str("{ x: 2field, y: 3field }")?,
                Plaintext::<CurrentNetwork>::from_str("{ x: 4field, y: 5field }")?,
                Plaintext::<CurrentNetwork>::from_str("{ x: 6field, y: 7field }")?,
                Plaintext::<CurrentNetwork>::from_str("{ x: 8field, y: 9field }")?,
            ],
            OnceLock::new(),
        ));

        // Test a non-uniform array.
        run_test(Plaintext::<CurrentNetwork>::Array(
            vec![
                Plaintext::<CurrentNetwork>::from_str("true")?,
                Plaintext::<CurrentNetwork>::from_str("1field")?,
                Plaintext::<CurrentNetwork>::from_str("{ x: 4field, y: 1u8 }")?,
            ],
            OnceLock::new(),
        ));

        Ok(())
    }
}
