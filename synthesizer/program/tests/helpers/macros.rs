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

/// Samples a random value for each literal type.
macro_rules! sample_literals {
    ($network:ident, $rng:expr) => {
        [
            console::program::Literal::<$network>::Address(console::types::Address::rand($rng)),
            console::program::Literal::Boolean(console::types::Boolean::rand($rng)),
            console::program::Literal::Field(console::types::Field::rand($rng)),
            console::program::Literal::Group(console::types::Group::rand($rng)),
            console::program::Literal::I8(console::types::I8::rand($rng)),
            console::program::Literal::I16(console::types::I16::rand($rng)),
            console::program::Literal::I32(console::types::I32::rand($rng)),
            console::program::Literal::I64(console::types::I64::rand($rng)),
            console::program::Literal::I128(console::types::I128::rand($rng)),
            console::program::Literal::U8(console::types::U8::rand($rng)),
            console::program::Literal::U16(console::types::U16::rand($rng)),
            console::program::Literal::U32(console::types::U32::rand($rng)),
            console::program::Literal::U64(console::types::U64::rand($rng)),
            console::program::Literal::U128(console::types::U128::rand($rng)),
            console::program::Literal::Scalar(console::types::Scalar::rand($rng)),
            console::program::Literal::sample(console::program::LiteralType::Signature, $rng),
            console::program::Literal::String(console::types::StringType::rand($rng)),
        ]
    };
}
