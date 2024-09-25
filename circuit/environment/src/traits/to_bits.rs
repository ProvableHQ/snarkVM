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

use crate::BooleanTrait;

/// Unary operator for converting to bits.
pub trait ToBits {
    type Boolean: BooleanTrait;

    /// Returns the little-endian bits of the circuit.
    fn to_bits_le(&self) -> Vec<Self::Boolean> {
        let mut bits = vec![];
        self.write_bits_le(&mut bits);
        bits
    }

    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>);

    /// Returns the big-endian bits of the circuit.
    fn to_bits_be(&self) -> Vec<Self::Boolean> {
        let mut bits = vec![];
        self.write_bits_be(&mut bits);
        bits
    }

    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>);
}

/********************/
/****** Arrays ******/
/********************/

impl<C: ToBits<Boolean = B>, B: BooleanTrait> ToBits for Vec<C> {
    type Boolean = B;

    /// A helper method to return a concatenated list of little-endian bits from the circuits.
    #[inline]
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        // The vector is order-preserving, meaning the first circuit in is the first circuit bits out.
        self.as_slice().write_bits_le(vec);
    }

    /// A helper method to return a concatenated list of big-endian bits from the circuits.
    #[inline]
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        // The vector is order-preserving, meaning the first circuit in is the first circuit bits out.
        self.as_slice().write_bits_be(vec);
    }
}

impl<C: ToBits<Boolean = B>, B: BooleanTrait, const N: usize> ToBits for [C; N] {
    type Boolean = B;

    /// A helper method to return a concatenated list of little-endian bits from the circuits.
    #[inline]
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        // The slice is order-preserving, meaning the first circuit in is the first circuit bits out.
        self.as_slice().write_bits_le(vec);
    }

    /// A helper method to return a concatenated list of big-endian bits from the circuits.
    #[inline]
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        // The slice is order-preserving, meaning the first circuit in is the first circuit bits out.
        self.as_slice().write_bits_be(vec);
    }
}

impl<C: ToBits<Boolean = B>, B: BooleanTrait> ToBits for &[C] {
    type Boolean = B;

    /// A helper method to return a concatenated list of little-endian bits from the circuits.
    #[inline]
    fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
        // The slice is order-preserving, meaning the first circuit in is the first circuit bits out.
        for elem in self.iter() {
            elem.write_bits_le(vec);
        }
    }

    /// A helper method to return a concatenated list of big-endian bits from the circuits.
    #[inline]
    fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
        // The slice is order-preserving, meaning the first circuit in is the first circuit bits out.
        for elem in self.iter() {
            elem.write_bits_be(vec);
        }
    }
}

/********************/
/****** Tuples ******/
/********************/

/// A helper macro to implement `ToBits` for a tuple of `ToBits` circuits.
macro_rules! to_bits_tuple {
    (($t0:ident, $i0:tt), $(($ty:ident, $idx:tt)),+) => {
        impl<B: BooleanTrait, $t0: ToBits<Boolean = B>, $($ty: ToBits<Boolean = B>),+> ToBits for ($t0, $($ty),+) {
            type Boolean = B;

            /// A helper method to return a concatenated list of little-endian bits from the circuits.
            #[inline]
            fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
                // The tuple is order-preserving, meaning the first circuit in is the first circuit bits out.
                (&self).write_bits_le(vec);
            }

            /// A helper method to return a concatenated list of big-endian bits from the circuits.
            #[inline]
            fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
                // The tuple is order-preserving, meaning the first circuit in is the first circuit bits out.
                (&self).write_bits_be(vec);
            }
        }

        impl<'a, B: BooleanTrait, $t0: ToBits<Boolean = B>, $($ty: ToBits<Boolean = B>),+> ToBits for &'a ($t0, $($ty),+) {
            type Boolean = B;

            /// A helper method to return a concatenated list of little-endian bits from the circuits.
            #[inline]
            fn write_bits_le(&self, vec: &mut Vec<Self::Boolean>) {
                // The tuple is order-preserving, meaning the first circuit in is the first circuit bits out.
                self.$i0.write_bits_le(vec);
                $(self.$idx.write_bits_le(vec);)+
            }

            /// A helper method to return a concatenated list of big-endian bits from the circuits.
            #[inline]
            fn write_bits_be(&self, vec: &mut Vec<Self::Boolean>) {
                // The tuple is order-preserving, meaning the first circuit in is the first circuit bits out.
                self.$i0.write_bits_be(vec);
                $(self.$idx.write_bits_be(vec);)+
            }
        }
    }
}

to_bits_tuple!((C0, 0), (C1, 1));
to_bits_tuple!((C0, 0), (C1, 1), (C2, 2));
to_bits_tuple!((C0, 0), (C1, 1), (C2, 2), (C3, 3));
to_bits_tuple!((C0, 0), (C1, 1), (C2, 2), (C3, 3), (C4, 4));
to_bits_tuple!((C0, 0), (C1, 1), (C2, 2), (C3, 3), (C4, 4), (C5, 5));
to_bits_tuple!((C0, 0), (C1, 1), (C2, 2), (C3, 3), (C4, 4), (C5, 5), (C6, 6));
to_bits_tuple!((C0, 0), (C1, 1), (C2, 2), (C3, 3), (C4, 4), (C5, 5), (C6, 6), (C7, 7));
to_bits_tuple!((C0, 0), (C1, 1), (C2, 2), (C3, 3), (C4, 4), (C5, 5), (C6, 6), (C7, 7), (C8, 8));
to_bits_tuple!((C0, 0), (C1, 1), (C2, 2), (C3, 3), (C4, 4), (C5, 5), (C6, 6), (C7, 7), (C8, 8), (C9, 9));
to_bits_tuple!((C0, 0), (C1, 1), (C2, 2), (C3, 3), (C4, 4), (C5, 5), (C6, 6), (C7, 7), (C8, 8), (C9, 9), (C10, 10));
