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

impl<N: Network> Record<N, Ciphertext<N>> {
    /// Decrypts `self` into plaintext using the given view key.
    pub fn is_owner(&self, view_key: &ViewKey<N>) -> bool {
        // Compute the address.
        let address = view_key.to_address();
        // Check if the address is the owner.
        self.is_owner_with_address_x_coordinate(view_key, &address.to_x_coordinate())
    }

    /// Decrypts `self` into plaintext using the x-coordinate of the address corresponding to the given view key.
    pub fn is_owner_with_address_x_coordinate(&self, view_key: &ViewKey<N>, address_x_coordinate: &Field<N>) -> bool {
        // In debug mode, check that the address corresponds to the given view key.
        debug_assert_eq!(
            &view_key.to_address().to_x_coordinate(),
            address_x_coordinate,
            "Failed to check record - view key and address do not match"
        );

        match &self.owner {
            // If the owner is public, check if the address is the owner.
            Owner::Public(owner) => &owner.to_x_coordinate() == address_x_coordinate,
            // If the owner is private, decrypt the owner to check if it matches the address.
            Owner::Private(ciphertext) => {
                // Compute the record view key.
                let record_view_key = (self.nonce * **view_key).to_x_coordinate();
                // Compute the 0th randomizer.
                let randomizer = N::hash_many_psd8(&[N::encryption_domain(), record_view_key], 1);
                // Decrypt the owner.
                let owner_x = ciphertext[0] - randomizer[0];
                // Compare the x coordinates of computed and supplied addresses.
                // We can skip recomputing the address from `owner_x` due to the following reasoning.
                // First, the transaction SNARK that generated the ciphertext would have checked that the ciphertext encrypts a valid address.
                // Now, since a valid address is an element of the prime-order subgroup of the curve, we know that the encrypted x-coordinate corresponds to a prime-order element.
                // Finally, since the SNARK + hybrid encryption
                // together are an authenticated encryption scheme, we know that the ciphertext has not been malleated.
                // Thus overall we know that if the x-coordinate matches that of `address`, then the underlying `address`es must also match.
                // Therefore we can skip recomputing the address from `owner_x` and instead compare the x-coordinates directly.
                &owner_x == address_x_coordinate
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Literal;
    use snarkvm_console_account::PrivateKey;
    use snarkvm_console_network::MainnetV0;
    use snarkvm_console_types::Field;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 1_000;

    fn check_is_owner<N: Network>(
        view_key: ViewKey<N>,
        owner: Owner<N, Plaintext<N>>,
        rng: &mut TestRng,
    ) -> Result<()> {
        // Prepare the record.
        let randomizer = Scalar::rand(rng);
        let record = Record {
            owner,
            data: IndexMap::from_iter(vec![
                (Identifier::from_str("a")?, Entry::Private(Plaintext::from(Literal::Field(Field::rand(rng))))),
                (Identifier::from_str("b")?, Entry::Private(Plaintext::from(Literal::Scalar(Scalar::rand(rng))))),
            ]),
            nonce: N::g_scalar_multiply(&randomizer),
        };

        // Encrypt the record.
        let ciphertext = record.encrypt(randomizer)?;

        // Ensure the record belongs to the owner.
        assert!(ciphertext.is_owner(&view_key));

        // Sample a random view key and address.
        let private_key = PrivateKey::<N>::new(rng)?;
        let view_key = ViewKey::try_from(&private_key)?;

        // Ensure the random address is not the owner.
        assert!(!ciphertext.is_owner(&view_key));

        Ok(())
    }

    #[test]
    fn test_is_owner() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a view key and address.
            let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng)?;
            let view_key = ViewKey::try_from(&private_key)?;
            let address = Address::try_from(&private_key)?;

            // Public owner.
            let owner = Owner::Public(address);
            check_is_owner::<CurrentNetwork>(view_key, owner, &mut rng)?;

            // Private owner.
            let owner = Owner::Private(Plaintext::from(Literal::Address(address)));
            check_is_owner::<CurrentNetwork>(view_key, owner, &mut rng)?;
        }
        Ok(())
    }
}
