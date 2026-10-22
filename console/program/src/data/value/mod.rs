// Copyright (c) 2019-2026 Provable Inc.
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
mod equal;
mod find;
mod parse;
mod serialize;
mod to_bits;
mod to_bits_raw;
mod to_fields;
mod to_fields_raw;

use crate::{Access, Argument, DynamicFuture, DynamicRecord, Entry, Future, Literal, Plaintext, Record};
use snarkvm_console_network::Network;
use snarkvm_console_types::prelude::*;

#[derive(Clone)]
pub enum Value<N: Network> {
    /// A plaintext value.
    Plaintext(Plaintext<N>),
    /// A record value.
    Record(Record<N, Plaintext<N>>),
    /// A future.
    Future(Future<N>),
    /// A dynamic record.
    DynamicRecord(DynamicRecord<N>),
    /// A dynamic future.
    DynamicFuture(DynamicFuture<N>),
}

impl<N: Network> From<Literal<N>> for Value<N> {
    /// Initializes the value from a literal.
    fn from(literal: Literal<N>) -> Self {
        Self::Plaintext(Plaintext::from(literal))
    }
}

impl<N: Network> From<&Literal<N>> for Value<N> {
    /// Initializes the value from a literal.
    fn from(literal: &Literal<N>) -> Self {
        Self::from(Plaintext::from(literal))
    }
}

impl<N: Network> From<Plaintext<N>> for Value<N> {
    /// Initializes the value from a plaintext.
    fn from(plaintext: Plaintext<N>) -> Self {
        Self::Plaintext(plaintext)
    }
}

impl<N: Network> From<&Plaintext<N>> for Value<N> {
    /// Initializes the value from a plaintext.
    fn from(plaintext: &Plaintext<N>) -> Self {
        Self::from(plaintext.clone())
    }
}

impl<N: Network> From<Record<N, Plaintext<N>>> for Value<N> {
    /// Initializes the value from a record.
    fn from(record: Record<N, Plaintext<N>>) -> Self {
        Self::Record(record)
    }
}

impl<N: Network> From<&Record<N, Plaintext<N>>> for Value<N> {
    /// Initializes the value from a record.
    fn from(record: &Record<N, Plaintext<N>>) -> Self {
        Self::from(record.clone())
    }
}

impl<N: Network> From<Future<N>> for Value<N> {
    /// Initializes the value from a future.
    fn from(future: Future<N>) -> Self {
        Self::Future(future)
    }
}

impl<N: Network> From<&Future<N>> for Value<N> {
    /// Initializes the value from a future.
    fn from(future: &Future<N>) -> Self {
        Self::from(future.clone())
    }
}

impl<N: Network> From<DynamicRecord<N>> for Value<N> {
    /// Initializes the value from a dynamic record.
    fn from(dynamic_record: DynamicRecord<N>) -> Self {
        Self::DynamicRecord(dynamic_record)
    }
}

impl<N: Network> From<&DynamicRecord<N>> for Value<N> {
    /// Initializes the value from a dynamic record.
    fn from(dynamic_record: &DynamicRecord<N>) -> Self {
        Self::from(dynamic_record.clone())
    }
}

impl<N: Network> From<DynamicFuture<N>> for Value<N> {
    /// Initializes the value from a dynamic future.
    fn from(dynamic_future: DynamicFuture<N>) -> Self {
        Self::DynamicFuture(dynamic_future)
    }
}

impl<N: Network> From<&DynamicFuture<N>> for Value<N> {
    /// Initializes the value from a dynamic future.
    fn from(dynamic_future: &DynamicFuture<N>) -> Self {
        Self::from(dynamic_future.clone())
    }
}

impl<N: Network> From<Argument<N>> for Value<N> {
    /// Initializes the value from an argument.
    fn from(argument: Argument<N>) -> Self {
        match argument {
            Argument::Plaintext(plaintext) => Self::Plaintext(plaintext),
            Argument::Future(future) => Self::Future(future),
            Argument::DynamicFuture(future) => Self::DynamicFuture(future),
        }
    }
}

impl<N: Network> From<&Argument<N>> for Value<N> {
    /// Initializes the value from an argument.
    fn from(argument: &Argument<N>) -> Self {
        Self::from(argument.clone())
    }
}

impl<N: Network> From<&Value<N>> for Value<N> {
    /// Returns a clone of the value.
    fn from(value: &Value<N>) -> Self {
        value.clone()
    }
}

impl<N: Network> TryFrom<Result<Value<N>>> for Value<N> {
    type Error = Error;

    /// Initializes a value from a result.
    fn try_from(value: Result<Value<N>>) -> Result<Self> {
        value
    }
}

impl<N: Network> TryFrom<String> for Value<N> {
    type Error = Error;

    /// Initializes a value from a string.
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl<N: Network> TryFrom<&String> for Value<N> {
    type Error = Error;

    /// Initializes a value from a string.
    fn try_from(value: &String) -> Result<Self> {
        Self::from_str(value)
    }
}

impl<N: Network> TryFrom<&str> for Value<N> {
    type Error = Error;

    /// Initializes a value from a string.
    fn try_from(value: &str) -> Result<Self> {
        Self::from_str(value)
    }
}

impl<N: Network> Value<N> {
    /// Checks that the bit encoding of this value fits its size prefixes.
    ///
    /// Plaintext bit encodings write a `u16` size in front of each struct member and each
    /// array element, and writing a larger value halts the process before signature or root
    /// validation. Callers must run this check on untrusted values before bit-encoding
    /// them, so that oversized values are rejected instead of halting.
    pub fn check_bit_lengths(&self) -> Result<()> {
        match self {
            // A plaintext is bit-encoded with a size prefix in front of each nested value.
            Self::Plaintext(plaintext) => plaintext.check_bit_lengths(),
            // A record is bit-encoded entry by entry.
            Self::Record(record) => record.check_bit_lengths(),
            // A future is bit-encoded argument by argument.
            Self::Future(future) => future.check_bit_lengths(),
            // A dynamic record and a dynamic future have a fixed-size bit encoding.
            Self::DynamicRecord(..) | Self::DynamicFuture(..) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Identifier, Owner, ProgramID};
    use snarkvm_console_network::MainnetV0;
    use snarkvm_utilities::{TestRng, Uniform};

    use core::str::FromStr;

    type CurrentNetwork = MainnetV0;

    /// Returns an outer array whose sole element is an array of the given number of u64s.
    /// The bit encoding of the inner array exceeds the `u16` size prefix when the number of
    /// elements is large enough.
    fn nested_array(num_elements: usize, rng: &mut TestRng) -> Plaintext<CurrentNetwork> {
        let inner = Plaintext::Array(
            core::iter::repeat_n(Plaintext::from(Literal::U64(U64::rand(rng))), num_elements).collect(),
            Default::default(),
        );
        Plaintext::Array(vec![inner], Default::default())
    }

    #[test]
    fn test_check_bit_lengths_accepts_valid_values() {
        let rng = &mut TestRng::default();

        // A literal.
        let literal = Value::<CurrentNetwork>::from(Literal::U64(U64::rand(rng)));
        assert!(literal.check_bit_lengths().is_ok());

        // An array of 512 u64s stays under the u16 size prefix.
        let valid = Value::Plaintext(nested_array(512, rng));
        assert!(valid.check_bit_lengths().is_ok());

        // A record with one entry.
        let data = indexmap::indexmap! {
            Identifier::from_str("amount").unwrap() => Entry::Private(Plaintext::from(Literal::U64(U64::rand(rng)))),
        };
        let record = Record::<CurrentNetwork, Plaintext<CurrentNetwork>>::from_plaintext(
            Owner::Public(Address::rand(rng)),
            data,
            Group::rand(rng),
            U8::new(0),
        )
        .unwrap();
        assert!(Value::from(record).check_bit_lengths().is_ok());
    }

    #[test]
    fn test_check_bit_lengths_rejects_oversized_plaintext() {
        let rng = &mut TestRng::default();

        // An outer array whose sole element is an array of 1024 u64s has a bit encoding
        // that exceeds the u16 size prefix the outer array writes before each element.
        let oversized = Value::Plaintext(nested_array(1024, rng));
        let error = oversized.check_bit_lengths().unwrap_err();
        assert!(error.to_string().contains("exceeds u16::MAX bits"));
    }

    #[test]
    fn test_check_bit_lengths_rejects_oversized_record_entry() {
        let rng = &mut TestRng::default();

        // A record with an oversized entry.
        let data = indexmap::indexmap! {
            Identifier::from_str("amount").unwrap() => Entry::Private(nested_array(1024, rng)),
        };
        let record = Record::<CurrentNetwork, Plaintext<CurrentNetwork>>::from_plaintext(
            Owner::Public(Address::rand(rng)),
            data,
            Group::rand(rng),
            U8::new(0),
        )
        .unwrap();
        let error = Value::from(record).check_bit_lengths().unwrap_err();
        assert!(error.to_string().contains("exceeds u16::MAX bits"));
    }

    #[test]
    fn test_check_bit_lengths_checks_future_arguments() {
        let rng = &mut TestRng::default();

        // A future with an oversized plaintext argument.
        let future = Future::<CurrentNetwork>::new(
            ProgramID::from_str("token.aleo").unwrap(),
            Identifier::from_str("transfer").unwrap(),
            vec![Argument::Plaintext(nested_array(1024, rng))],
        );
        let error = Value::Future(future).check_bit_lengths().unwrap_err();
        assert!(error.to_string().contains("exceeds u16::MAX bits"));
    }
}
