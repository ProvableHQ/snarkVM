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

use super::*;

impl<N: Network> Literal<N> {
    /// Returns the variant of the literal.
    pub fn variant(&self) -> u8 {
        match self {
            Self::Address(..) => 0,
            Self::Boolean(..) => 1,
            Self::Field(..) => 2,
            Self::Group(..) => 3,
            Self::I8(..) => 4,
            Self::I16(..) => 5,
            Self::I32(..) => 6,
            Self::I64(..) => 7,
            Self::I128(..) => 8,
            Self::U8(..) => 9,
            Self::U16(..) => 10,
            Self::U32(..) => 11,
            Self::U64(..) => 12,
            Self::U128(..) => 13,
            Self::Scalar(..) => 14,
            Self::Signature(..) => 15,
            Self::String(..) => 16,
        }
    }
}
