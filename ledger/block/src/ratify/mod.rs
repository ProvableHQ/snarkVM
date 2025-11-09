// Copyright (c) 2019-2025 Provable Inc.
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

use console::{
    account::{PrivateKey, Signature},
    network::prelude::*,
    types::{Address, Field},
};
use snarkvm_ledger_committee::Committee;

use indexmap::IndexMap;

type Variant = u8;
/// A helper type to represent the public balances.
type PublicBalances<N> = IndexMap<Address<N>, u64>;
/// A helper type to represent the bonded balances, as a
/// mapping of `staker_address` to `(validator_address, withdrawal_address, amount)`.
type BondedBalances<N> = IndexMap<Address<N>, (Address<N>, Address<N>, u64)>;

// Note: The size of the `Ratify` object is 32 bytes.
#[derive(Clone, PartialEq, Eq)]
pub enum Ratify<N: Network> {
    /// The genesis.
    Genesis(Box<Committee<N>>, Box<PublicBalances<N>>, Box<BondedBalances<N>>),
    /// The block reward.
    BlockReward(u64),
    /// The puzzle reward.
    PuzzleReward(u64),
    /// The blob ratification as (Hash of the blob, signature of the id, blob data).
    Blob(Field<N>, Signature<N>, Option<Vec<u8>>),
}

impl<N: Network> Ratify<N> {
    /// Returns the ratification ID.
    pub fn to_id(&self) -> Result<N::RatificationID> {
        match self {
            Ratify::Blob(_blob_id, _signature, _) => {
                // TODO (raychu86): Blob - Determine the ID for this.
                todo!()
            }
            _ => Ok(N::hash_bhp1024(&self.to_bytes_le()?.to_bits_le())?.into()),
        }
    }

    /// Creates a new blob ratification.
    pub fn new_blob<R: Rng + CryptoRng>(private_key: &PrivateKey<N>, blob_data: Vec<u8>, rng: &mut R) -> Result<Self> {
        // Calculate the blob ID.
        let blob_id = N::hash_bhp1024(&blob_data.to_bits_le())?;

        // Sign the blob ID.
        let signature = private_key.sign(&[blob_id], rng)?;

        // Return the blob ratification.
        Ok(Ratify::Blob(blob_id, signature, Some(blob_data)))
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    pub(crate) fn sample_ratifications(rng: &mut TestRng) -> Vec<Ratify<CurrentNetwork>> {
        let committee = snarkvm_ledger_committee::test_helpers::sample_committee(rng);
        let mut public_balances = PublicBalances::new();
        for (address, _) in committee.members().iter() {
            public_balances.insert(*address, rng.r#gen());
        }
        let bonded_balances = committee
            .members()
            .iter()
            .map(|(address, (amount, _, _))| (*address, (*address, *address, *amount)))
            .collect();

        let blob_id = Field::<CurrentNetwork>::rand(rng);
        let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
        let signature = private_key.sign(&[blob_id], rng).unwrap();
        let blob_data: Vec<u8> = (0..256).map(|_| rng.r#gen::<u8>()).collect();

        vec![
            Ratify::Genesis(Box::new(committee), Box::new(public_balances), Box::new(bonded_balances)),
            Ratify::BlockReward(rng.r#gen()),
            Ratify::PuzzleReward(rng.r#gen()),
            Ratify::Blob(blob_id, signature, Some(blob_data)),
            Ratify::Blob(blob_id, signature, None),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_ratify_size() {
        assert_eq!(std::mem::size_of::<Ratify<console::network::MainnetV0>>(), 32);
    }
}
