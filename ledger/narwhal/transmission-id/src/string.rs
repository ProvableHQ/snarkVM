// Copyright (C) 2019-2023 Aleo Systems Inc.
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

impl<N: Network> FromStr for TransmissionID<N> {
    type Err = Error;

    /// Initializes the transmission ID from a string.
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        // Split the id and checksum.
        let (id, checksum) = input.split_once('.').ok_or_else(|| anyhow!("Invalid transmission ID: {input}"))?;
        // Parse the string.
        if id.starts_with(SOLUTION_ID_PREFIX) {
            Ok(Self::Solution(
                SolutionID::from_str(id)?,
                N::TransmissionChecksum::from_str(checksum)
                    .map_err(|_| anyhow!("Failed to parse checksum: {checksum}"))?,
            ))
        } else if id.starts_with(TRANSACTION_PREFIX) {
            Ok(Self::Transaction(
                N::TransactionID::from_str(id).map_err(|_| anyhow!("Failed to parse transaction ID: {id}"))?,
                N::TransmissionChecksum::from_str(checksum)
                    .map_err(|_| anyhow!("Failed to parse checksum: {checksum}"))?,
            ))
        } else {
            bail!("Invalid transmission ID: {input}")
        }
    }
}

impl<N: Network> Debug for TransmissionID<N> {
    /// Prints the transmission ID as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for TransmissionID<N> {
    /// Prints the transmission ID as a string.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ratification => write!(f, "ratification"),
            Self::Solution(id, checksum) => write!(f, "{}.{}", id, checksum),
            Self::Transaction(id, checksum) => write!(f, "{}.{}", id, checksum),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_string() {
        let rng = &mut TestRng::default();

        for expected in crate::test_helpers::sample_transmission_ids(rng) {
            // Check the string representation.
            let candidate = format!("{expected}");
            assert_eq!(expected, TransmissionID::from_str(&candidate).unwrap());
            assert!(TransmissionID::<CurrentNetwork>::from_str(&candidate[1..]).is_err());
        }
    }
}
