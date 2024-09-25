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

impl<A: Aleo> Record<A, Plaintext<A>> {
    /// Encrypts `self` for the record owner under the given randomizer.
    pub fn encrypt(&self, randomizer: &Scalar<A>) -> Record<A, Ciphertext<A>> {
        // Ensure the randomizer corresponds to the record nonce.
        A::assert_eq(&self.nonce, A::g_scalar_multiply(randomizer));
        // Compute the record view key.
        let record_view_key = ((*self.owner).to_group() * randomizer).to_x_coordinate();
        // Encrypt the record.
        self.encrypt_symmetric_unchecked(record_view_key)
    }

    /// Encrypts `self` under the given record view key.
    /// Note: This method does not check that the record view key corresponds to the record owner.
    /// Use `Self::encrypt` for the checked variant.
    pub fn encrypt_symmetric_unchecked(&self, record_view_key: Field<A>) -> Record<A, Ciphertext<A>> {
        // Determine the number of randomizers needed to encrypt the record.
        let num_randomizers = self.num_randomizers();
        // Prepare a randomizer for each field element.
        let randomizers = A::hash_many_psd8(&[A::encryption_domain(), record_view_key], num_randomizers);
        // Encrypt the record.
        self.encrypt_with_randomizers(&randomizers)
    }

    /// Encrypts `self` under the given randomizers.
    fn encrypt_with_randomizers(&self, randomizers: &[Field<A>]) -> Record<A, Ciphertext<A>> {
        // Initialize an index to keep track of the randomizer index.
        let mut index: usize = 0;

        // Encrypt the owner.
        let owner = match self.owner.is_public().eject_value() {
            true => self.owner.encrypt(&[]),
            false => self.owner.encrypt(&[randomizers[index].clone()]),
        };

        // Increment the index if the owner is private.
        if owner.is_private().eject_value() {
            index += 1;
        }

        // Encrypt the data.
        let mut encrypted_data = IndexMap::with_capacity(self.data.len());
        for (id, entry, num_randomizers) in self.data.iter().map(|(id, entry)| (id, entry, entry.num_randomizers())) {
            // Retrieve the randomizers for this entry.
            let randomizers = &randomizers[index..index + num_randomizers as usize];
            // Encrypt the entry.
            let entry = match entry {
                // Constant entries do not need to be encrypted.
                Entry::Constant(plaintext) => Entry::Constant(plaintext.clone()),
                // Public entries do not need to be encrypted.
                Entry::Public(plaintext) => Entry::Public(plaintext.clone()),
                // Private entries are encrypted with the given randomizers.
                Entry::Private(private) => Entry::Private(private.encrypt_with_randomizers(randomizers)),
            };
            // Insert the encrypted entry.
            if encrypted_data.insert(id.clone(), entry).is_some() {
                A::halt(format!("Duplicate identifier in record: {id}"))
            }
            // Increment the index.
            index += num_randomizers as usize;
        }

        // Return the encrypted record.
        Record { owner, data: encrypted_data, nonce: self.nonce.clone() }
    }
}
