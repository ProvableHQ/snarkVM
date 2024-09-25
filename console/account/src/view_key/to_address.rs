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

impl<N: Network> ViewKey<N> {
    /// Returns the address corresponding to the view key.
    pub fn to_address(&self) -> Address<N> {
        Address::new(N::g_scalar_multiply(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    const ITERATIONS: u64 = 1000;

    #[test]
    fn test_try_from() -> Result<()> {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a new view key and address.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let view_key = ViewKey::try_from(private_key)?;
            let address = Address::try_from(private_key)?;

            assert_eq!(address, view_key.to_address());
        }
        Ok(())
    }
}
