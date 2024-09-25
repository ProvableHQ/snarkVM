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

mod equal;
mod find;
mod to_bits;
mod to_fields;

use crate::{Access, Entry, Future, Plaintext, Record};
use snarkvm_circuit_network::Aleo;
use snarkvm_circuit_types::{environment::prelude::*, Boolean, Field};

#[derive(Clone)]
pub enum Value<A: Aleo> {
    /// A plaintext value.
    Plaintext(Plaintext<A>),
    /// A record value.
    Record(Record<A, Plaintext<A>>),
    /// A future.
    Future(Future<A>),
}

impl<A: Aleo> Inject for Value<A> {
    type Primitive = console::Value<A::Network>;

    /// Initializes a circuit of the given mode and value.
    fn new(mode: Mode, value: Self::Primitive) -> Self {
        match value {
            console::Value::Plaintext(plaintext) => Value::Plaintext(Plaintext::new(mode, plaintext)),
            console::Value::Record(record) => Value::Record(Record::new(Mode::Private, record)),
            console::Value::Future(future) => Value::Future(Future::new(mode, future)),
        }
    }
}

impl<A: Aleo> Eject for Value<A> {
    type Primitive = console::Value<A::Network>;

    /// Ejects the mode of the circuit value.
    fn eject_mode(&self) -> Mode {
        match self {
            Value::Plaintext(plaintext) => plaintext.eject_mode(),
            Value::Record(record) => record.eject_mode(),
            Value::Future(future) => future.eject_mode(),
        }
    }

    /// Ejects the circuit value.
    fn eject_value(&self) -> Self::Primitive {
        match self {
            Value::Plaintext(plaintext) => console::Value::Plaintext(plaintext.eject_value()),
            Value::Record(record) => console::Value::Record(record.eject_value()),
            Value::Future(future) => console::Value::Future(future.eject_value()),
        }
    }
}
