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

use crate::prelude::*;

/// Representation of a base field element.
pub trait FieldTrait:
    Add<Output = Self>
    + AddAssign
    + Clone
    + Div<Output = Self>
    + DivAssign
    + Double<Output = Self>
    + Eject
    + Equal
    + FromBits
    + FromBoolean
    + Inject
    + Inv
    + Mul<Output = Self>
    + MulAssign
    + Neg<Output = Self>
    + One
    + Parser
    + Pow<Self, Output = Self>
    + Square<Output = Self>
    + Sub<Output = Self>
    + SubAssign
    + Ternary
    + ToBits
    + TypeName
    + Zero
{
}
