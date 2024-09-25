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

impl<N: Network> Record<N, Plaintext<N>> {
    /// Encrypts `self` for the record owner under the given randomizer.
    pub fn encrypt(&self, randomizer: Scalar<N>) -> Result<Record<N, Ciphertext<N>>> {
        // Ensure the randomizer corresponds to the record nonce.
        if self.nonce == N::g_scalar_multiply(&randomizer) {
            // Compute the record view key.
            let record_view_key = (**self.owner * randomizer).to_x_coordinate();
            // Encrypt the record.
            self.encrypt_symmetric_unchecked(&record_view_key)
        } else {
            bail!("Illegal operation: Record::encrypt() randomizer does not correspond to the record nonce.")
        }
    }

    /// Encrypts `self` under the given record view key.
    /// Note: This method does not check that the record view key corresponds to the record owner.
    /// Use `Self::encrypt` for the checked variant.
    pub fn encrypt_symmetric_unchecked(&self, record_view_key: &Field<N>) -> Result<Record<N, Ciphertext<N>>> {
        // Determine the number of randomizers needed to encrypt the record.
        let num_randomizers = self.num_randomizers()?;
        // Prepare a randomizer for each field element.
        let randomizers = N::hash_many_psd8(&[N::encryption_domain(), *record_view_key], num_randomizers);
        // Encrypt the record.
        self.encrypt_with_randomizers(&randomizers)
    }

    /// Encrypts `self` under the given randomizers.
    fn encrypt_with_randomizers(&self, randomizers: &[Field<N>]) -> Result<Record<N, Ciphertext<N>>> {
        // Initialize an index to keep track of the randomizer index.
        let mut index: usize = 0;

        // Encrypt the owner.
        let owner = match self.owner.is_public() {
            true => self.owner.encrypt_with_randomizer(&[])?,
            false => self.owner.encrypt_with_randomizer(&[randomizers[index]])?,
        };

        // Increment the index if the owner is private.
        if owner.is_private() {
            index += 1;
        }

        // Encrypt the data.
        let mut encrypted_data = IndexMap::with_capacity(self.data.len());
        for (id, entry, num_randomizers) in self.data.iter().map(|(id, entry)| (id, entry, entry.num_randomizers())) {
            // Retrieve the result for `num_randomizers`.
            let num_randomizers = num_randomizers? as usize;
            // Retrieve the randomizers for this entry.
            let randomizers = &randomizers[index..index + num_randomizers];
            // Encrypt the entry.
            let entry = match entry {
                // Constant entries do not need to be encrypted.
                Entry::Constant(plaintext) => Entry::Constant(plaintext.clone()),
                // Public entries do not need to be encrypted.
                Entry::Public(plaintext) => Entry::Public(plaintext.clone()),
                // Private entries are encrypted with the given randomizers.
                Entry::Private(private) => Entry::Private(Ciphertext::try_from(
                    private
                        .to_fields()?
                        .iter()
                        .zip_eq(randomizers)
                        .map(|(plaintext, randomizer)| *plaintext + randomizer)
                        .collect::<Vec<_>>(),
                )?),
            };
            // Insert the encrypted entry.
            if encrypted_data.insert(*id, entry).is_some() {
                bail!("Duplicate identifier in record: {}", id);
            }
            // Increment the index.
            index += num_randomizers;
        }

        // Return the encrypted record.
        Self::from_ciphertext(owner, encrypted_data, self.nonce)
    }
}
