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

impl<N: Network> FromBytes for Ratify<N> {
    /// Reads the ratify object from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid ratify version"));
        }

        let variant = Variant::read_le(&mut reader)?;
        let ratify = match variant {
            0 => {
                // Read the committee.
                let committee: Committee<N> = FromBytes::read_le(&mut reader)?;
                // Read the number of public balances.
                let num_public_balances: u16 = FromBytes::read_le(&mut reader)?;
                // Read the public balances.
                let mut public_balances = PublicBalances::with_capacity(num_public_balances as usize);
                for _ in 0..num_public_balances {
                    // Read the address.
                    let address: Address<N> = FromBytes::read_le(&mut reader)?;
                    // Read the amount.
                    let amount: u64 = FromBytes::read_le(&mut reader)?;
                    // Insert the public balance.
                    public_balances.insert(address, amount);
                }
                // Read the number of bonded balances.
                let num_bonded_balances: u16 = FromBytes::read_le(&mut reader)?;
                // Read the bonded balances.
                let mut bonded_balances = BondedBalances::with_capacity(num_bonded_balances as usize);
                for _ in 0..num_bonded_balances {
                    // Read the address.
                    let address: Address<N> = FromBytes::read_le(&mut reader)?;
                    // Read the validator address.
                    let validator_address: Address<N> = FromBytes::read_le(&mut reader)?;
                    // Read the withdrawal address.
                    let withdrawal_address: Address<N> = FromBytes::read_le(&mut reader)?;
                    // Read the amount.
                    let amount: u64 = FromBytes::read_le(&mut reader)?;
                    // Insert the bonded balance.
                    bonded_balances.insert(address, (validator_address, withdrawal_address, amount));
                }
                // Return the ratify object.
                Self::Genesis(Box::new(committee), Box::new(public_balances), Box::new(bonded_balances))
            }
            1 => {
                // Read the amount.
                let amount: u64 = FromBytes::read_le(&mut reader)?;
                // Return the ratify object.
                Self::BlockReward(amount)
            }
            2 => {
                // Read the amount.
                let amount: u64 = FromBytes::read_le(&mut reader)?;
                // Return the ratify object.
                Self::PuzzleReward(amount)
            }
            3.. => return Err(error(format!("Failed to decode ratify object variant {variant}"))),
        };
        Ok(ratify)
    }
}

impl<N: Network> ToBytes for Ratify<N> {
    /// Writes the ratify object to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;

        match self {
            Self::Genesis(committee, public_balances, bonded_balances) => {
                (0 as Variant).write_le(&mut writer)?;
                committee.write_le(&mut writer)?;
                u16::try_from(public_balances.len()).map_err(|e| error(e.to_string()))?.write_le(&mut writer)?;
                for (address, amount) in public_balances.iter() {
                    address.write_le(&mut writer)?;
                    amount.write_le(&mut writer)?;
                }
                u16::try_from(bonded_balances.len()).map_err(|e| error(e.to_string()))?.write_le(&mut writer)?;
                for (address, (validator_address, withdrawal_address, amount)) in bonded_balances.iter() {
                    address.write_le(&mut writer)?;
                    validator_address.write_le(&mut writer)?;
                    withdrawal_address.write_le(&mut writer)?;
                    amount.write_le(&mut writer)?;
                }
                Ok(())
            }
            Self::BlockReward(amount) => {
                (1 as Variant).write_le(&mut writer)?;
                amount.write_le(&mut writer)
            }
            Self::PuzzleReward(amount) => {
                (2 as Variant).write_le(&mut writer)?;
                amount.write_le(&mut writer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() {
        let rng = &mut TestRng::default();

        for expected in crate::ratify::test_helpers::sample_ratifications(rng) {
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le().unwrap();
            assert_eq!(expected, Ratify::read_le(&expected_bytes[..]).unwrap());
        }
    }
}
