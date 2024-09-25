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

#[cfg(test)]
use snarkvm_circuit_types::environment::assert_scope;

mod from_outputs;
mod process_outputs_from_callback;

use crate::{compute_function_id, Identifier, ProgramID, Value};
use snarkvm_circuit_network::Aleo;
use snarkvm_circuit_types::{environment::prelude::*, Field, U16};

pub enum OutputID<A: Aleo> {
    /// The hash of the constant output.
    Constant(Field<A>),
    /// The hash of the public output.
    Public(Field<A>),
    /// The ciphertext hash of the private output.
    Private(Field<A>),
    /// The `(commitment, checksum)` tuple of the record output.
    Record(Field<A>, Field<A>),
    /// The hash of the external record output.
    ExternalRecord(Field<A>),
    /// The hash of the future output.
    Future(Field<A>),
}

#[cfg(console)]
impl<A: Aleo> Inject for OutputID<A> {
    type Primitive = console::OutputID<A::Network>;

    /// Initializes the output ID from the given mode and console output ID.
    fn new(_: Mode, output: Self::Primitive) -> Self {
        match output {
            // Inject the expected hash as `Mode::Public`.
            console::OutputID::Constant(field) => Self::Constant(Field::new(Mode::Public, field)),
            // Inject the expected hash as `Mode::Public`.
            console::OutputID::Public(field) => Self::Public(Field::new(Mode::Public, field)),
            // Inject the ciphertext hash as `Mode::Public`.
            console::OutputID::Private(field) => Self::Private(Field::new(Mode::Public, field)),
            // Inject the expected commitment and checksum as `Mode::Public`.
            console::OutputID::Record(commitment, checksum) => {
                Self::Record(Field::new(Mode::Public, commitment), Field::new(Mode::Public, checksum))
            }
            // Inject the expected hash as `Mode::Public`.
            console::OutputID::ExternalRecord(hash) => Self::ExternalRecord(Field::new(Mode::Public, hash)),
            // Inject the expected hash as `Mode::Public`.
            console::OutputID::Future(hash) => Self::Future(Field::new(Mode::Public, hash)),
        }
    }
}

impl<A: Aleo> OutputID<A> {
    /// Initializes a constant output ID.
    fn constant(expected_hash: Field<A>) -> Self {
        // Inject the expected hash as `Mode::Public`.
        let output_hash = Field::new(Mode::Public, expected_hash.eject_value());
        // Ensure the injected hash matches the given hash.
        A::assert_eq(&output_hash, expected_hash);
        // Return the output ID.
        Self::Constant(output_hash)
    }

    /// Initializes a public output ID.
    fn public(expected_hash: Field<A>) -> Self {
        // Inject the expected hash as `Mode::Public`.
        let output_hash = Field::new(Mode::Public, expected_hash.eject_value());
        // Ensure the injected hash matches the given hash.
        A::assert_eq(&output_hash, expected_hash);
        // Return the output ID.
        Self::Public(output_hash)
    }

    /// Initializes a private output ID.
    fn private(expected_hash: Field<A>) -> Self {
        // Inject the ciphertext hash as `Mode::Public`.
        let output_hash = Field::new(Mode::Public, expected_hash.eject_value());
        // Ensure the injected hash matches the given hash.
        A::assert_eq(&output_hash, expected_hash);
        // Return the output ID.
        Self::Private(output_hash)
    }

    /// Initializes a record output ID.
    fn record(expected_commitment: Field<A>, expected_checksum: Field<A>) -> Self {
        // Inject the expected commitment and checksum as `Mode::Public`.
        let output_commitment = Field::new(Mode::Public, expected_commitment.eject_value());
        let output_checksum = Field::new(Mode::Public, expected_checksum.eject_value());
        // Ensure the injected commitment and checksum match the given commitment and checksum.
        A::assert_eq(&output_commitment, expected_commitment);
        A::assert_eq(&output_checksum, expected_checksum);
        // Return the output ID.
        Self::Record(output_commitment, output_checksum)
    }

    /// Initializes an external record output ID.
    fn external_record(expected_hash: Field<A>) -> Self {
        // Inject the expected hash as `Mode::Public`.
        let output_hash = Field::new(Mode::Public, expected_hash.eject_value());
        // Ensure the injected hash matches the given commitment.
        A::assert_eq(&output_hash, expected_hash);
        // Return the output ID.
        Self::ExternalRecord(output_hash)
    }

    /// Initializes a future output ID.
    fn future(expected_hash: Field<A>) -> Self {
        // Inject the expected hash as `Mode::Public`.
        let output_hash = Field::new(Mode::Public, expected_hash.eject_value());
        // Ensure the injected hash matches the given hash.
        A::assert_eq(&output_hash, expected_hash);
        // Return the output ID.
        Self::Future(output_hash)
    }
}

#[cfg(console)]
impl<A: Aleo> Eject for OutputID<A> {
    type Primitive = console::OutputID<A::Network>;

    /// Ejects the mode of the output ID.
    fn eject_mode(&self) -> Mode {
        match self {
            Self::Constant(field) => field.eject_mode(),
            Self::Public(field) => field.eject_mode(),
            Self::Private(field) => field.eject_mode(),
            Self::Record(commitment, checksum) => Mode::combine(commitment.eject_mode(), [checksum.eject_mode()]),
            Self::ExternalRecord(hash) => hash.eject_mode(),
            Self::Future(hash) => hash.eject_mode(),
        }
    }

    /// Ejects the output ID as a primitive.
    fn eject_value(&self) -> Self::Primitive {
        match self {
            Self::Constant(field) => console::OutputID::Constant(field.eject_value()),
            Self::Public(field) => console::OutputID::Public(field.eject_value()),
            Self::Private(field) => console::OutputID::Private(field.eject_value()),
            Self::Record(commitment, checksum) => {
                console::OutputID::Record(commitment.eject_value(), checksum.eject_value())
            }
            Self::ExternalRecord(hash) => console::OutputID::ExternalRecord(hash.eject_value()),
            Self::Future(hash) => console::OutputID::Future(hash.eject_value()),
        }
    }
}

pub struct Response<A: Aleo> {
    /// The function output IDs.
    output_ids: Vec<OutputID<A>>,
    /// The function outputs.
    outputs: Vec<Value<A>>,
}

impl<A: Aleo> Response<A> {
    /// Returns the output IDs for the transition.
    pub fn output_ids(&self) -> &[OutputID<A>] {
        &self.output_ids
    }

    /// Returns the function outputs.
    pub fn outputs(&self) -> &[Value<A>] {
        &self.outputs
    }
}

#[cfg(console)]
impl<A: Aleo> Eject for Response<A> {
    type Primitive = console::Response<A::Network>;

    /// Ejects the mode of the response.
    fn eject_mode(&self) -> Mode {
        Mode::combine(self.output_ids.eject_mode(), [self.outputs.eject_mode()])
    }

    /// Ejects the response as a primitive.
    fn eject_value(&self) -> Self::Primitive {
        Self::Primitive::from((
            self.output_ids.iter().map(|output_id| output_id.eject_value()).collect(),
            self.outputs.eject_value(),
        ))
    }
}
