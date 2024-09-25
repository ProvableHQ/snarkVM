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

mod bytes;
mod decrypt;
mod equal;
mod from_bits;
mod from_fields;
mod num_randomizers;
mod parse;
mod serialize;
mod size_in_fields;
mod to_bits;
mod to_fields;

use crate::Plaintext;
use snarkvm_console_account::ViewKey;
use snarkvm_console_network::prelude::*;
use snarkvm_console_types::{Boolean, Field, Group};

use core::ops::Deref;

#[derive(Clone)]
pub struct Ciphertext<N: Network>(Vec<Field<N>>);

impl<N: Network> Deref for Ciphertext<N> {
    type Target = [Field<N>];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
