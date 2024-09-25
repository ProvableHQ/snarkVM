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

use crate::{Ciphertext, Entry, Literal, Plaintext};
use snarkvm_console_network::prelude::*;
use snarkvm_console_types::{Address, Boolean, Field};

/// A value stored in program data.
#[derive(Clone)]
pub enum Owner<N: Network, Private: Visibility> {
    /// A publicly-visible value.
    Public(Address<N>),
    /// A private value is encrypted under the account owner's address.
    Private(Private),
}

impl<N: Network> Deref for Owner<N, Plaintext<N>> {
    type Target = Address<N>;

    /// Returns the address of the owner.
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Public(public) => public,
            Self::Private(Plaintext::Literal(Literal::Address(address), ..)) => address,
            _ => N::halt("Internal error: plaintext deref corrupted in record owner"),
        }
    }
}

impl<N: Network, Private: Visibility> Owner<N, Private> {
    /// Returns `true` if `self` is public.
    pub const fn is_public(&self) -> bool {
        matches!(self, Self::Public(..))
    }

    /// Returns `true` if `self` is private.
    pub const fn is_private(&self) -> bool {
        matches!(self, Self::Private(..))
    }
}

impl<N: Network> Owner<N, Plaintext<N>> {
    /// Returns the owner as an `Entry`.
    pub fn to_entry(&self) -> Entry<N, Plaintext<N>> {
        match self {
            Self::Public(owner) => Entry::Public(Plaintext::from(Literal::Address(*owner))),
            Self::Private(plaintext, ..) => Entry::Private(plaintext.clone()),
        }
    }
}

impl<N: Network, Private: Visibility<Boolean = Boolean<N>>> Eq for Owner<N, Private> {}

impl<N: Network, Private: Visibility<Boolean = Boolean<N>>> PartialEq for Owner<N, Private> {
    /// Returns `true` if `self` and `other` are equal.
    fn eq(&self, other: &Self) -> bool {
        *self.is_equal(other)
    }
}

impl<N: Network, Private: Visibility<Boolean = Boolean<N>>> Equal<Self> for Owner<N, Private> {
    type Output = Boolean<N>;

    /// Returns `true` if `self` and `other` are equal.
    fn is_equal(&self, other: &Self) -> Self::Output {
        match (self, other) {
            (Self::Public(a), Self::Public(b)) => a.is_equal(b),
            (Self::Private(a), Self::Private(b)) => a.is_equal(b),
            (Self::Public(_), _) | (Self::Private(_), _) => Boolean::new(false),
        }
    }

    /// Returns `true` if `self` and `other` are *not* equal.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        match (self, other) {
            (Self::Public(a), Self::Public(b)) => a.is_not_equal(b),
            (Self::Private(a), Self::Private(b)) => a.is_not_equal(b),
            (Self::Public(_), _) | (Self::Private(_), _) => Boolean::new(true),
        }
    }
}

impl<N: Network> Owner<N, Plaintext<N>> {
    /// Encrypts `self` under the given randomizer.
    pub fn encrypt_with_randomizer(&self, randomizer: &[Field<N>]) -> Result<Owner<N, Ciphertext<N>>> {
        match self {
            Self::Public(public) => {
                // Ensure there is exactly zero randomizers.
                ensure!(randomizer.is_empty(), "Expected 0 randomizers, found {}", randomizer.len());
                // Return the owner.
                Ok(Owner::Public(*public))
            }
            Self::Private(Plaintext::Literal(Literal::Address(address), ..)) => {
                // Ensure there is exactly one randomizer.
                ensure!(randomizer.len() == 1, "Expected 1 randomizer, found {}", randomizer.len());
                // Encrypt the owner.
                let ciphertext = address.to_field()? + randomizer[0];
                // Return the encrypted owner.
                Ok(Owner::Private(Ciphertext::from_fields(&[ciphertext])?))
            }
            _ => bail!("Internal error: plaintext encryption corrupted in record owner"),
        }
    }
}

impl<N: Network> Owner<N, Ciphertext<N>> {
    /// Decrypts the owner under the given randomizer.
    pub fn decrypt_with_randomizer(&self, randomizer: &[Field<N>]) -> Result<Owner<N, Plaintext<N>>> {
        match self {
            Self::Public(public) => {
                // Ensure there is exactly zero randomizers.
                ensure!(randomizer.is_empty(), "Expected 0 randomizers, found {}", randomizer.len());
                // Return the owner.
                Ok(Owner::Public(*public))
            }
            Self::Private(ciphertext) => {
                // Ensure there is exactly one randomizer.
                ensure!(randomizer.len() == 1, "Expected 1 randomizer, found {}", randomizer.len());
                // Ensure there is exactly one field element in the ciphertext.
                ensure!(ciphertext.len() == 1, "Expected 1 ciphertext, found {}", ciphertext.len());
                // Decrypt the owner.
                let owner = Address::from_field(&(ciphertext[0] - randomizer[0]))?;
                // Return the owner.
                Ok(Owner::Private(Plaintext::from(Literal::Address(owner))))
            }
        }
    }
}

impl<N: Network> ToBits for Owner<N, Plaintext<N>> {
    /// Returns `self` as a boolean vector in little-endian order.
    fn write_bits_le(&self, vec: &mut Vec<bool>) {
        vec.push(self.is_private());
        match self {
            Self::Public(public) => public.write_bits_le(vec),
            Self::Private(Plaintext::Literal(Literal::Address(address), ..)) => address.write_bits_le(vec),
            _ => N::halt("Internal error: plaintext to_bits_le corrupted in record owner"),
        };
    }

    /// Returns `self` as a boolean vector in big-endian order.
    fn write_bits_be(&self, vec: &mut Vec<bool>) {
        vec.push(self.is_private());
        match self {
            Self::Public(public) => public.write_bits_be(vec),
            Self::Private(Plaintext::Literal(Literal::Address(address), ..)) => address.write_bits_be(vec),
            _ => N::halt("Internal error: plaintext to_bits_be corrupted in record owner"),
        };
    }
}

impl<N: Network> ToBits for Owner<N, Ciphertext<N>> {
    /// Returns `self` as a boolean vector in little-endian order.
    fn write_bits_le(&self, vec: &mut Vec<bool>) {
        vec.push(self.is_private());
        match self {
            Self::Public(public) => public.write_bits_le(vec),
            Self::Private(ciphertext) => {
                // Ensure there is exactly one field element in the ciphertext.
                match ciphertext.len() == 1 {
                    true => ciphertext[0].write_bits_le(vec),
                    false => N::halt("Internal error: ciphertext to_bits_le corrupted in record owner"),
                }
            }
        };
    }

    /// Returns `self` as a boolean vector in big-endian order.
    fn write_bits_be(&self, vec: &mut Vec<bool>) {
        vec.push(self.is_private());
        match self {
            Self::Public(public) => public.write_bits_be(vec),
            Self::Private(ciphertext) => {
                // Ensure there is exactly one field element in the ciphertext.
                match ciphertext.len() == 1 {
                    true => ciphertext[0].write_bits_be(vec),
                    false => N::halt("Internal error: ciphertext to_bits_be corrupted in record owner"),
                }
            }
        };
    }
}

impl<N: Network> Debug for Owner<N, Plaintext<N>> {
    /// Prints the owner as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for Owner<N, Plaintext<N>> {
    /// Prints the owner as a string, i.e. `aleo1xxx.public`.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Public(owner) => write!(f, "{owner}.public"),
            Self::Private(Plaintext::Literal(Literal::Address(owner), ..)) => write!(f, "{owner}.private"),
            _ => N::halt("Internal error: plaintext fmt corrupted in record owner"),
        }
    }
}

impl<N: Network, Private: Visibility> FromBytes for Owner<N, Private> {
    /// Reads the owner from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the index.
        let index = u8::read_le(&mut reader)?;
        // Read the owner.
        let owner = match index {
            0 => Self::Public(Address::read_le(&mut reader)?),
            1 => Self::Private(Private::read_le(&mut reader)?),
            2.. => return Err(error(format!("Failed to decode owner variant {index}"))),
        };
        Ok(owner)
    }
}

impl<N: Network, Private: Visibility> ToBytes for Owner<N, Private> {
    /// Writes the owner to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::Public(owner) => {
                0u8.write_le(&mut writer)?;
                owner.write_le(&mut writer)
            }
            Self::Private(owner) => {
                1u8.write_le(&mut writer)?;
                owner.write_le(&mut writer)
            }
        }
    }
}
