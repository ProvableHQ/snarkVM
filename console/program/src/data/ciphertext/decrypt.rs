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

impl<N: Network> Ciphertext<N> {
    /// Decrypts `self` into plaintext using the given account view key & nonce.
    pub fn decrypt(&self, view_key: ViewKey<N>, nonce: Group<N>) -> Result<Plaintext<N>> {
        // Compute the plaintext view key.
        let plaintext_view_key = (nonce * *view_key).to_x_coordinate();
        // Decrypt the record.
        self.decrypt_symmetric(plaintext_view_key)
    }

    /// Decrypts `self` into plaintext using the given plaintext view key.
    pub fn decrypt_symmetric(&self, plaintext_view_key: Field<N>) -> Result<Plaintext<N>> {
        // Determine the number of randomizers needed to encrypt the plaintext.
        let num_randomizers = self.num_randomizers()?;
        // Prepare a randomizer for each field element.
        let randomizers = N::hash_many_psd8(&[N::encryption_domain(), plaintext_view_key], num_randomizers);
        // Decrypt the plaintext.
        self.decrypt_with_randomizers(&randomizers)
    }

    /// Decrypts `self` into plaintext using the given randomizers.
    pub(crate) fn decrypt_with_randomizers(&self, randomizers: &[Field<N>]) -> Result<Plaintext<N>> {
        // Decrypt the ciphertext.
        Plaintext::from_fields(
            &self
                .iter()
                .zip_eq(randomizers)
                .map(|(ciphertext, randomizer)| *ciphertext - randomizer)
                .collect::<Vec<_>>(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Literal;
    use snarkvm_console_account::{Address, PrivateKey};
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 100;

    fn check_encrypt_and_decrypt<N: Network>(rng: &mut TestRng) -> Result<()> {
        // Prepare the plaintext.
        let plaintext_string = r"{
  foo: 5u8,
  bar: {
    baz: 10field,
    qux: {
      quux: {
        corge: {
          grault: {
            garply: {
              waldo: {
                fred: {
                  plugh: {
                    xyzzy: {
                      thud: true
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
}";
        let plaintext = Plaintext::<N>::from_str(plaintext_string)?;

        // Sample a random address.
        let private_key = PrivateKey::<N>::new(rng)?;
        let view_key = ViewKey::<N>::try_from(private_key)?;
        let address = Address::<N>::try_from(view_key)?;

        // Encrypt the plaintext.
        let randomizer = Uniform::rand(rng);
        let ciphertext = plaintext.encrypt(&address, randomizer)?;

        // Decrypt the plaintext.
        let nonce = N::g_scalar_multiply(&randomizer);
        assert_eq!(plaintext, ciphertext.decrypt(view_key, nonce)?);
        Ok(())
    }

    fn check_encrypt_and_decrypt_symmetric<N: Network>(rng: &mut TestRng) -> Result<()> {
        // Prepare the plaintext.
        let plaintext = Plaintext::<N>::from(Literal::Field(Uniform::rand(rng)));

        // Encrypt the plaintext.
        let plaintext_view_key = Uniform::rand(rng);
        let ciphertext = plaintext.encrypt_symmetric(plaintext_view_key)?;
        // Decrypt the plaintext.
        assert_eq!(plaintext, ciphertext.decrypt_symmetric(plaintext_view_key)?);
        Ok(())
    }

    #[test]
    fn test_encrypt_and_decrypt() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            check_encrypt_and_decrypt::<CurrentNetwork>(&mut rng)?;
            check_encrypt_and_decrypt_symmetric::<CurrentNetwork>(&mut rng)?;
        }
        Ok(())
    }
}
