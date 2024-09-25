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

impl<N: Network> Committee<N> {
    /// Returns the committee ID.
    pub fn to_id(&self) -> Result<Field<N>> {
        Self::compute_committee_id(self.starting_round, &self.members, self.total_stake)
    }
}

impl<N: Network> Committee<N> {
    /// Returns the commmitee ID.
    pub fn compute_committee_id(
        starting_round: u64,
        members: &IndexMap<Address<N>, (u64, bool, u8)>,
        total_stake: u64,
    ) -> Result<Field<N>> {
        let mut preimage = Vec::new();
        // Insert the starting_round.
        starting_round.write_le(&mut preimage)?;
        // Write the number of members.
        u16::try_from(members.len())?.write_le(&mut preimage)?;
        // Write the members.
        for (address, (stake, is_open, commission)) in members {
            // Write the address.
            address.write_le(&mut preimage)?;
            // Write the stake.
            stake.write_le(&mut preimage)?;
            // Write the is_open flag.
            is_open.write_le(&mut preimage)?;
            // Write the commission.
            commission.write_le(&mut preimage)?;
        }
        // Insert the total stake.
        total_stake.write_le(&mut preimage)?;
        // Hash the preimage.
        N::hash_bhp1024(&preimage.to_bits_le())
    }
}
