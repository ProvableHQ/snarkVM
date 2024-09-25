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

impl<N: Network> ProgramID<N> {
    /// Returns the program address for this program ID.
    pub fn to_address(&self) -> Result<Address<N>> {
        // Compute the program address as `HashToGroup(program_id)`.
        let group = N::hash_to_group_psd4(&[self.name().to_field()?, self.network().to_field()?])?;
        // Return the program address.
        Ok(Address::new(group))
    }
}
