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

impl<A: Aleo> Ciphertext<A> {
    /// Decrypts `self` into plaintext using the given plaintext view key.
    pub fn decrypt_symmetric(&self, plaintext_view_key: Field<A>) -> Plaintext<A> {
        // Determine the number of randomizers needed to encrypt the plaintext.
        let num_randomizers = self.num_randomizers();
        // Prepare a randomizer for each field element.
        let randomizers = A::hash_many_psd8(&[A::encryption_domain(), plaintext_view_key], num_randomizers);
        // Decrypt the plaintext.
        self.decrypt_with_randomizers(&randomizers)
    }

    /// Decrypts `self` into plaintext using the given randomizers.
    pub(crate) fn decrypt_with_randomizers(&self, randomizers: &[Field<A>]) -> Plaintext<A> {
        // Decrypt the ciphertext.
        Plaintext::from_fields(
            &self
                .iter()
                .zip_eq(randomizers)
                .map(|(ciphertext, randomizer)| ciphertext - randomizer)
                .collect::<Vec<_>>(),
        )
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use crate::{Circuit, Literal};
    use snarkvm_circuit_types::Field;
    use snarkvm_utilities::{TestRng, Uniform};

    use anyhow::Result;

    const ITERATIONS: u64 = 100;

    fn check_encrypt_and_decrypt<A: Aleo>() -> Result<()> {
        let mut rng = TestRng::default();

        // Prepare the plaintext.
        let plaintext = Plaintext::<A>::from(Literal::Field(Field::new(Mode::Private, Uniform::rand(&mut rng))));

        // Encrypt the plaintext.
        let plaintext_view_key = Field::new(Mode::Private, Uniform::rand(&mut rng));
        let ciphertext = plaintext.encrypt_symmetric(plaintext_view_key.clone());
        // Decrypt the plaintext.
        assert_eq!(plaintext.eject(), ciphertext.decrypt_symmetric(plaintext_view_key).eject());
        Ok(())
    }

    #[test]
    fn test_encrypt_and_decrypt() -> Result<()> {
        for _ in 0..ITERATIONS {
            check_encrypt_and_decrypt::<Circuit>()?;
        }
        Ok(())
    }
}
