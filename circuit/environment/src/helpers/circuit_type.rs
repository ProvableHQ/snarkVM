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

use crate::{Eject, Mode};

/// Helper enum used in the case where a circuit's output mode or counts are determined by
/// its mode and the actual value of the circuit.
#[derive(Debug, Clone)]
pub enum CircuitType<T: Eject> {
    Constant(T),
    Public,
    Private,
}

impl<T: Eject> CircuitType<T> {
    pub fn mode(&self) -> Mode {
        match self {
            CircuitType::Constant(constant) => constant.eject_mode(),
            CircuitType::Public => Mode::Public,
            CircuitType::Private => Mode::Private,
        }
    }
}

/// Initializes a new `CircuitType` from a circuit.
/// If the circuit is constant, the `CircuitType` will be `Constant(circuit)`.
/// Otherwise, the `CircuitType` will be `Public` or `Private`.
impl<T: Eject + Clone> From<T> for CircuitType<T> {
    fn from(circuit: T) -> Self {
        match circuit.eject_mode() {
            Mode::Constant => CircuitType::Constant(circuit),
            Mode::Public => CircuitType::Public,
            Mode::Private => CircuitType::Private,
        }
    }
}

/// Initializes a new `CircuitType` from a circuit.
/// If the circuit is constant, the `CircuitType` will be `Constant(circuit)`.
/// Otherwise, the `CircuitType` will be `Public` or `Private`.
impl<T: Eject + Clone> From<&T> for CircuitType<T> {
    fn from(circuit: &T) -> Self {
        match circuit.eject_mode() {
            Mode::Constant => CircuitType::Constant(circuit.clone()),
            Mode::Public => CircuitType::Public,
            Mode::Private => CircuitType::Private,
        }
    }
}
