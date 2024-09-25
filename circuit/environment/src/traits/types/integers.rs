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

/// Representation of an integer.
pub trait IntegerTrait<I: IntegerType, U8: IntegerCore<u8>, U16: IntegerCore<u16>, U32: IntegerCore<u32>>:
    IntegerCore<I>
    + PowChecked<U8, Output = Self>
    + PowWrapped<U8, Output = Self>
    + Shl<U8, Output = Self>
    + ShlAssign<U8>
    + ShlChecked<U8, Output = Self>
    + ShlWrapped<U8, Output = Self>
    + Shr<U8, Output = Self>
    + ShrAssign<U8>
    + ShrChecked<U8, Output = Self>
    + ShrWrapped<U8, Output = Self>
    + PowChecked<U16, Output = Self>
    + PowWrapped<U16, Output = Self>
    + Shl<U16, Output = Self>
    + ShlAssign<U16>
    + ShlChecked<U16, Output = Self>
    + ShlWrapped<U16, Output = Self>
    + Shr<U16, Output = Self>
    + ShrAssign<U16>
    + ShrChecked<U16, Output = Self>
    + ShrWrapped<U16, Output = Self>
    + PowChecked<U32, Output = Self>
    + PowWrapped<U32, Output = Self>
    + Shl<U32, Output = Self>
    + ShlAssign<U32>
    + ShlChecked<U32, Output = Self>
    + ShlWrapped<U32, Output = Self>
    + Shr<U32, Output = Self>
    + ShrAssign<U32>
    + ShrChecked<U32, Output = Self>
    + ShrWrapped<U32, Output = Self>
{
}

pub trait IntegerCore<I: IntegerType>:
    AbsChecked
    + AbsWrapped
    + AddAssign
    + Add<Output = Self>
    + AddChecked<Output = Self>
    + AddWrapped<Output = Self>
    + BitAndAssign
    + BitAnd<Output = Self>
    + BitOrAssign
    + BitOr<Output = Self>
    + BitXorAssign
    + BitXor<Output = Self>
    + Clone
    + Compare
    + DivAssign
    + Div<Output = Self>
    + DivChecked<Output = Self>
    + DivWrapped<Output = Self>
    + Eject
    + Equal
    + FromBits
    + Inject
    + MulAssign
    + Mul<Output = Self>
    + MulChecked<Output = Self>
    + MulWrapped<Output = Self>
    + Neg<Output = Self>
    + Not<Output = Self>
    + One
    + Parser
    + RemAssign
    + Rem<Output = Self>
    + RemChecked<Output = Self>
    + RemWrapped<Output = Self>
    + SubAssign
    + Sub<Output = Self>
    + SubChecked<Output = Self>
    + SubWrapped<Output = Self>
    + Ternary
    + ToBits
    + TypeName
    + Zero
{
}
