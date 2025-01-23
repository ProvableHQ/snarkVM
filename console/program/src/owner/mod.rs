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
mod serialize;
mod string;

use snarkvm_console_account::{Address, PrivateKey, Signature};
use snarkvm_console_network::Network;
use snarkvm_console_types::prelude::*;

/// Metadata regarding an owner of a program.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum ProgramOwner<N: Network> {
    /// A V1 program owner.
    V1(ProgramOwnerV1<N>),
    /// A V2 program owner.
    V2(ProgramOwnerV2<N>),
}

impl<N: Network> ProgramOwner<N> {
    /// Initializes a new V1 program owner.
    pub fn new_v1<R: Rng + CryptoRng>(
        private_key: &PrivateKey<N>,
        deployment_id: Field<N>,
        rng: &mut R,
    ) -> Result<Self> {
        Ok(Self::V1(ProgramOwnerV1::new(private_key, deployment_id, rng)?))
    }

    /// Initializes a new V2 program owner.
    pub fn new_v2<R: Rng + CryptoRng>(
        private_key: &PrivateKey<N>,
        authority: Address<N>,
        deployment_id: Field<N>,
        rng: &mut R,
    ) -> Result<Self> {
        Ok(Self::V2(ProgramOwnerV2::new(private_key, authority, deployment_id, rng)?))
    }

    /// Returns the program owner as a V1 owner.
    pub fn as_v1(&self) -> Option<&ProgramOwnerV1<N>> {
        match self {
            Self::V1(owner) => Some(owner),
            _ => None,
        }
    }

    /// Returns the program owner as a V2 owner.
    pub fn as_v2(&self) -> Option<&ProgramOwnerV2<N>> {
        match self {
            Self::V2(owner) => Some(owner),
            _ => None,
        }
    }

    /// Returns whether the program owner is a V1 owner.
    pub const fn is_v1(&self) -> bool {
        matches!(self, Self::V1(_))
    }

    /// Returns whether the program owner is a V2 owner.
    pub const fn is_v2(&self) -> bool {
        matches!(self, Self::V2(_))
    }
}

impl<N: Network> ProgramOwner<N> {
    /// Returns the address of the program owner.
    pub const fn address(&self) -> &Address<N> {
        match self {
            Self::V1(owner) => owner.address(),
            Self::V2(owner) => owner.address(),
        }
    }

    /// Returns the authority of the program owner.
    pub const fn authority(&self) -> Option<&Address<N>> {
        match self {
            Self::V1(_) => None,
            Self::V2(owner) => Some(&owner.authority()),
        }
    }

    /// Returns the signature of the program owner.
    pub const fn signature(&self) -> &Signature<N> {
        match self {
            Self::V1(owner) => owner.signature(),
            Self::V2(owner) => owner.signature(),
        }
    }

    /// Verify that the signature is valid for the given deployment ID.
    pub fn verify(&self, deployment_id: Field<N>) -> bool {
        match self {
            Self::V1(owner) => owner.verify(deployment_id),
            Self::V2(owner) => owner.verify(deployment_id),
        }
    }
}

/// A V1 program owner.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ProgramOwnerV1<N: Network> {
    /// The address of the program owner.
    address: Address<N>,
    /// The signature of the program owner, over the deployment transaction ID.
    signature: Signature<N>,
}

impl<N: Network> ProgramOwnerV1<N> {
    /// Initializes a new V1 program owner.
    pub fn new<R: Rng + CryptoRng>(private_key: &PrivateKey<N>, deployment_id: Field<N>, rng: &mut R) -> Result<Self> {
        // Derive the address.
        let address = Address::try_from(private_key)?;
        // Sign the transaction ID.
        let signature = private_key.sign(&[deployment_id], rng)?;
        // Return the V2 program owner.
        Ok(Self { address, signature })
    }

    /// Initializes a new V1 program owner from an address and signature.
    pub fn from(address: Address<N>, signature: Signature<N>) -> Self {
        Self { address, signature }
    }

    /// Returns the address of the V1 program owner.
    pub const fn address(&self) -> &Address<N> {
        &self.address
    }

    /// Returns the signature of the V1 program owner.
    pub const fn signature(&self) -> &Signature<N> {
        &self.signature
    }

    /// Verify that the signature is valid for the given deployment ID.
    pub fn verify(&self, deployment_id: Field<N>) -> bool {
        self.signature.verify(&self.address, &[deployment_id])
    }
}

/// A V2 program owner.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ProgramOwnerV2<N: Network> {
    /// The address of the program owner.
    address: Address<N>,
    /// The address of the authority allowed to update the program.
    authority: Address<N>,
    /// The signature of the program owner, over the deployment transaction ID.
    signature: Signature<N>,
}

impl<N: Network> ProgramOwnerV2<N> {
    /// Initializes a new V2 program owner.
    pub fn new<R: Rng + CryptoRng>(
        private_key: &PrivateKey<N>,
        authority: Address<N>,
        deployment_id: Field<N>,
        rng: &mut R,
    ) -> Result<Self> {
        // Derive the address.
        let address = Address::try_from(private_key)?;
        // Sign the transaction ID.
        let signature = private_key.sign(&[authority.to_x_coordinate(), deployment_id], rng)?;
        // Return the program owner.
        Ok(Self { address, authority, signature })
    }

    /// Initializes a new V2 program owner from an address, authority, and signature.
    pub fn from(address: Address<N>, authority: Address<N>, signature: Signature<N>) -> Self {
        Self { address, authority, signature }
    }

    /// Returns the address of the V2 program owner.
    pub const fn address(&self) -> &Address<N> {
        &self.address
    }

    /// Returns the authority of the V2 program owner.
    pub const fn authority(&self) -> &Address<N> {
        &self.authority
    }

    /// Returns the signature of the V2 program owner.
    pub const fn signature(&self) -> &Signature<N> {
        &self.signature
    }

    /// Verify that the signature is valid for the given deployment ID.
    pub fn verify(&self, deployment_id: Field<N>) -> bool {
        self.signature.verify(&self.address, &[self.authority.to_x_coordinate(), deployment_id])
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    use once_cell::sync::OnceCell;

    type CurrentNetwork = MainnetV0;

    pub(crate) fn sample_program_owner_v1() -> ProgramOwner<CurrentNetwork> {
        static V1_INSTANCE: OnceCell<ProgramOwner<CurrentNetwork>> = OnceCell::new();
        *V1_INSTANCE.get_or_init(|| {
            // Initialize the RNG.
            let rng = &mut TestRng::default();

            // Initialize a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();

            // Initialize a deployment ID.
            let deployment_id: Field<CurrentNetwork> = rng.gen();

            // Return the program owner.
            ProgramOwner::new_v1(&private_key, deployment_id, rng).unwrap()
        })
    }

    pub(crate) fn sample_program_owner_v2() -> ProgramOwner<CurrentNetwork> {
        static V2_INSTANCE: OnceCell<ProgramOwner<CurrentNetwork>> = OnceCell::new();
        *V2_INSTANCE.get_or_init(|| {
            // Initialize the RNG.
            let rng = &mut TestRng::default();

            // Initialize a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();

            // Initialize an authority.
            let authority = Address::<CurrentNetwork>::try_from(&private_key).unwrap();

            // Initialize a deployment ID.
            let deployment_id: Field<CurrentNetwork> = rng.gen();

            // Return the program owner.
            ProgramOwner::new_v2(&private_key, authority, deployment_id, rng).unwrap()
        })
    }

    #[test]
    fn test_verify_program_owner_v1() {
        // Initialize the RNG.
        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();

        // Initialize a deployment ID.
        let deployment_id: Field<CurrentNetwork> = rng.gen();

        // Construct the program owner.
        let owner = ProgramOwner::new_v1(&private_key, deployment_id, rng).unwrap();
        // Ensure that the program owner is verified for the given deployment ID.
        assert!(owner.verify(deployment_id));

        // Ensure that the program owner is not verified for a different deployment ID.
        let incorrect_deployment_id: Field<CurrentNetwork> = rng.gen();
        assert!(!owner.verify(incorrect_deployment_id));
    }

    #[test]
    fn test_verify_program_owner_v2() {
        // Initialize the RNG.
        let rng = &mut TestRng::default();

        // Initialize a private key.
        let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();

        // Initialize an authority.
        let authority = Address::<CurrentNetwork>::try_from(&private_key).unwrap();

        // Initialize a deployment ID.
        let deployment_id: Field<CurrentNetwork> = rng.gen();

        // Construct the program owner.
        let owner = ProgramOwner::new_v2(&private_key, authority, deployment_id, rng).unwrap();
        // Ensure that the program owner is verified for the given deployment ID.
        assert!(owner.verify(deployment_id));

        // Ensure that the program owner is not verified for a different deployment ID.
        let incorrect_deployment_id: Field<CurrentNetwork> = rng.gen();
        assert!(!owner.verify(incorrect_deployment_id));
    }
}
