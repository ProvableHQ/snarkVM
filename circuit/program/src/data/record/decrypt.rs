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

impl<A: Aleo> Record<A, Ciphertext<A>> {
    /// Decrypts `self` into a plaintext record using the given view key & nonce.
    pub fn decrypt(&self, view_key: &ViewKey<A>) -> Record<A, Plaintext<A>> {
        // Compute the record view key.
        let record_view_key = (&**view_key * &self.nonce).to_x_coordinate();
        // Decrypt the record.
        let record = self.decrypt_symmetric_unchecked(record_view_key);
        // Ensure the view key corresponds to the record owner.
        A::assert_eq(view_key.to_address(), record.owner().deref());
        // Return the decrypted record.
        record
    }

    /// Decrypts `self` into a plaintext record using the given record view key.
    /// Note: This method does not check that the record view key corresponds to the record owner.
    /// Use `Self::decrypt` for the checked variant.
    pub fn decrypt_symmetric_unchecked(&self, record_view_key: Field<A>) -> Record<A, Plaintext<A>> {
        // Determine the number of randomizers needed to encrypt the record.
        let num_randomizers = self.num_randomizers();
        // Prepare a randomizer for each field element.
        let randomizers = A::hash_many_psd8(&[A::encryption_domain(), record_view_key], num_randomizers);
        // Decrypt the record.
        self.decrypt_with_randomizers(&randomizers)
    }

    /// Decrypts `self` into a plaintext record using the given randomizers.
    fn decrypt_with_randomizers(&self, randomizers: &[Field<A>]) -> Record<A, Plaintext<A>> {
        // Initialize an index to keep track of the randomizer index.
        let mut index: usize = 0;

        // Decrypt the owner.
        let owner = match self.owner.is_public().eject_value() {
            true => self.owner.decrypt(&[]),
            false => self.owner.decrypt(&[randomizers[index].clone()]),
        };

        // Increment the index if the owner is private.
        if owner.is_private().eject_value() {
            index += 1;
        }

        // Decrypt the program data.
        let mut decrypted_data = IndexMap::with_capacity(self.data.len());
        for (id, entry, num_randomizers) in self.data.iter().map(|(id, entry)| (id, entry, entry.num_randomizers())) {
            // Retrieve the randomizers for this entry.
            let randomizers = &randomizers[index..index + num_randomizers as usize];
            // Decrypt the entry.
            let entry = match entry {
                // Constant entries do not need to be decrypted.
                Entry::Constant(plaintext) => Entry::Constant(plaintext.clone()),
                // Public entries do not need to be decrypted.
                Entry::Public(plaintext) => Entry::Public(plaintext.clone()),
                // Private entries are decrypted with the given randomizers.
                Entry::Private(private) => Entry::Private(private.decrypt_with_randomizers(randomizers)),
            };
            // Insert the decrypted entry.
            if decrypted_data.insert(id.clone(), entry).is_some() {
                A::halt(format!("Duplicate identifier in record: {id}"))
            }
            // Increment the index.
            index += num_randomizers as usize;
        }

        // Return the decrypted record.
        Record { owner, data: decrypted_data, nonce: self.nonce.clone() }
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::{Circuit, Literal};
    use snarkvm_circuit_types::{Address, Field};
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const ITERATIONS: u64 = 100;

    fn check_encrypt_and_decrypt<A: Aleo>(
        view_key: &ViewKey<A>,
        owner: Owner<A, Plaintext<A>>,
        rng: &mut TestRng,
    ) -> Result<()> {
        // Prepare the record.
        let randomizer = Scalar::new(Mode::Private, Uniform::rand(rng));
        let record = Record {
            owner,
            data: IndexMap::from_iter(vec![
                (
                    Identifier::from_str("a")?,
                    Entry::Private(Plaintext::from(Literal::Field(Field::new(Mode::Private, Uniform::rand(rng))))),
                ),
                (
                    Identifier::from_str("b")?,
                    Entry::Private(Plaintext::from(Literal::Scalar(Scalar::new(Mode::Private, Uniform::rand(rng))))),
                ),
            ]),
            nonce: A::g_scalar_multiply(&randomizer),
        };

        // Encrypt the record.
        let ciphertext = record.encrypt(&randomizer);
        // Decrypt the record.
        assert_eq!(record.eject(), ciphertext.decrypt(view_key).eject());
        Ok(())
    }

    #[test]
    fn test_encrypt_and_decrypt() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Generate a private key, view key, and address.
            let private_key = snarkvm_console_account::PrivateKey::<<Circuit as Environment>::Network>::new(&mut rng)?;
            let view_key = snarkvm_console_account::ViewKey::try_from(private_key)?;
            let address = snarkvm_console_account::Address::try_from(private_key)?;

            // Initialize a view key and address.
            let view_key = ViewKey::<Circuit>::new(Mode::Private, view_key);
            let owner = address;

            // Public owner.
            {
                let owner = Owner::Public(Address::<Circuit>::new(Mode::Public, owner));
                check_encrypt_and_decrypt::<Circuit>(&view_key, owner, &mut rng)?;
            }

            // Private owner.
            {
                let owner =
                    Owner::Private(Plaintext::from(Literal::Address(Address::<Circuit>::new(Mode::Private, owner))));
                check_encrypt_and_decrypt::<Circuit>(&view_key, owner, &mut rng)?;
            }
        }
        Ok(())
    }
}
