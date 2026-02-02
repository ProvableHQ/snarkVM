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

mod entry;
pub use entry::Entry;

mod helpers;
pub use helpers::Owner;

mod bytes;
mod decrypt;
mod encrypt;
mod equal;
mod find;
mod is_owner;
mod num_randomizers;
mod parse_ciphertext;
mod parse_plaintext;
mod serial_number;
mod serialize;
mod tag;
mod to_bits;
mod to_commitment;
mod to_fields;

use crate::{Access, Ciphertext, Identifier, Literal, Plaintext, ProgramID};
use snarkvm_console_account::{Address, PrivateKey, ViewKey};
use snarkvm_console_network::prelude::*;
use snarkvm_console_types::{Boolean, Field, Group, Scalar, U8};

use indexmap::IndexMap;

/// A value stored in program record.
#[derive(Clone)]
pub struct Record<N: Network, Private: Visibility> {
    /// The owner of the program record.
    owner: Owner<N, Private>,
    /// The program data.
    data: IndexMap<Identifier<N>, Entry<N, Private>>,
    /// The nonce of the program record.
    nonce: Group<N>,
    /// The version of the program record.
    ///   - Version 0 uses a BHP hash to derive the record commitment.
    ///   - Version 1 uses a BHP commitment to derive the record commitment.
    version: U8<N>,
}

impl<N: Network, Private: Visibility> Record<N, Private> {
    /// Initializes a new record plaintext.
    pub fn from_plaintext(
        owner: Owner<N, Plaintext<N>>,
        data: IndexMap<Identifier<N>, Entry<N, Plaintext<N>>>,
        nonce: Group<N>,
        version: U8<N>,
    ) -> Result<Record<N, Plaintext<N>>> {
        let reserved = [Identifier::from_str("owner")?];
        // Ensure the members has no duplicate names.
        ensure!(!has_duplicates(data.keys().chain(reserved.iter())), "Found a duplicate entry name in a record");
        // Ensure the number of entries is within the maximum limit.
        ensure!(data.len() <= N::MAX_DATA_ENTRIES, "Found a record that exceeds size ({})", data.len());
        // Return the record.
        Ok(Record { owner, data, nonce, version })
    }

    /// Initializes a new record ciphertext.
    pub fn from_ciphertext(
        owner: Owner<N, Ciphertext<N>>,
        data: IndexMap<Identifier<N>, Entry<N, Ciphertext<N>>>,
        nonce: Group<N>,
        version: U8<N>,
    ) -> Result<Record<N, Ciphertext<N>>> {
        let reserved = [Identifier::from_str("owner")?];
        // Ensure the members has no duplicate names.
        ensure!(!has_duplicates(data.keys().chain(reserved.iter())), "Found a duplicate entry name in a record");
        // Ensure the number of entries is within the maximum limit.
        ensure!(data.len() <= N::MAX_DATA_ENTRIES, "Found a record that exceeds size ({})", data.len());
        // Return the record.
        Ok(Record { owner, data, nonce, version })
    }
}

impl<N: Network, Private: Visibility> Record<N, Private> {
    /// Returns the owner of the program record.
    pub const fn owner(&self) -> &Owner<N, Private> {
        &self.owner
    }

    /// Returns the program data.
    pub const fn data(&self) -> &IndexMap<Identifier<N>, Entry<N, Private>> {
        &self.data
    }

    /// Returns the nonce of the program record.
    pub const fn nonce(&self) -> &Group<N> {
        &self.nonce
    }

    /// Returns the version of the program record.
    pub const fn version(&self) -> &U8<N> {
        &self.version
    }

    /// Returns `true` if the program record is a hiding variant.
    pub fn is_hiding(&self) -> bool {
        !self.version.is_zero()
    }
}

impl<N: Network, Private: Visibility> Record<N, Private> {
    /// Returns the owner of the program record, and consumes `self`.
    pub fn into_owner(self) -> Owner<N, Private> {
        self.owner
    }

    /// Returns the program data, and consumes `self`.
    pub fn into_data(self) -> IndexMap<Identifier<N>, Entry<N, Private>> {
        self.data
    }

    /// Returns the nonce of the program record, and consumes `self`.
    pub fn into_nonce(self) -> Group<N> {
        self.nonce
    }
}
