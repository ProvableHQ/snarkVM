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

/// An argument passed into a future.
#[derive(Clone)]
pub enum Argument<A: Aleo> {
    /// A plaintext value.
    Plaintext(Plaintext<A>),
    /// A future.
    Future(Future<A>),
}

impl<A: Aleo> Inject for Argument<A> {
    type Primitive = console::Argument<A::Network>;

    /// Initializes a circuit of the given mode and argument.
    fn new(mode: Mode, value: Self::Primitive) -> Self {
        match value {
            console::Argument::Plaintext(plaintext) => Self::Plaintext(Inject::new(mode, plaintext)),
            console::Argument::Future(future) => Self::Future(Inject::new(mode, future)),
        }
    }
}

impl<A: Aleo> Eject for Argument<A> {
    type Primitive = console::Argument<A::Network>;

    /// Ejects the mode of the circuit argument.
    fn eject_mode(&self) -> Mode {
        match self {
            Self::Plaintext(plaintext) => plaintext.eject_mode(),
            Self::Future(future) => future.eject_mode(),
        }
    }

    /// Ejects the circuit argument.
    fn eject_value(&self) -> Self::Primitive {
        match self {
            Self::Plaintext(plaintext) => Self::Primitive::Plaintext(plaintext.eject_value()),
            Self::Future(future) => Self::Primitive::Future(future.eject_value()),
        }
    }
}

impl<A: Aleo> Equal<Self> for Argument<A> {
    type Output = Boolean<A>;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Self) -> Self::Output {
        match (self, other) {
            (Self::Plaintext(plaintext_a), Self::Plaintext(plaintext_b)) => plaintext_a.is_equal(plaintext_b),
            (Self::Future(future_a), Self::Future(future_b)) => future_a.is_equal(future_b),
            (Self::Plaintext(..), _) | (Self::Future(..), _) => Boolean::constant(false),
        }
    }

    /// Returns `true` if `self` and `other` are *not* equal.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        match (self, other) {
            (Self::Plaintext(plaintext_a), Self::Plaintext(plaintext_b)) => plaintext_a.is_not_equal(plaintext_b),
            (Self::Future(future_a), Self::Future(future_b)) => future_a.is_not_equal(future_b),
            (Self::Plaintext(..), _) | (Self::Future(..), _) => Boolean::constant(true),
        }
    }
}

impl<A: Aleo> ToBits for Argument<A> {
    type Boolean = Boolean<A>;

    /// Returns the argument as a list of **little-endian** bits.
    #[inline]
    fn write_bits_le(&self, vec: &mut Vec<Boolean<A>>) {
        match self {
            Self::Plaintext(plaintext) => {
                vec.push(Boolean::constant(false));
                plaintext.write_bits_le(vec);
            }
            Self::Future(future) => {
                vec.push(Boolean::constant(true));
                future.write_bits_le(vec);
            }
        }
    }

    /// Returns the argument as a list of **big-endian** bits.
    #[inline]
    fn write_bits_be(&self, vec: &mut Vec<Boolean<A>>) {
        match self {
            Self::Plaintext(plaintext) => {
                vec.push(Boolean::constant(false));
                plaintext.write_bits_be(vec);
            }
            Self::Future(future) => {
                vec.push(Boolean::constant(true));
                future.write_bits_be(vec);
            }
        }
    }
}
