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

impl<N: Network> Ratifications<N> {
    /// Returns the ratifications root, by computing the root for a Merkle tree of the ratification IDs.
    pub fn to_ratifications_root(&self) -> Result<Field<N>> {
        Ok(*self.to_tree()?.root())
    }

    /// Returns the Merkle path for the ratifications leaf.
    pub fn to_path(&self, ratification_id: N::RatificationID) -> Result<RatificationsPath<N>> {
        match self.ratifications.get_index_of(&ratification_id) {
            Some(ratification_index) => self.to_tree()?.prove(ratification_index, &ratification_id.to_bits_le()),
            None => bail!("The ratification '{ratification_id}' is not in the block ratifications"),
        }
    }

    /// The Merkle tree of ratification IDs for the block.
    pub fn to_tree(&self) -> Result<RatificationsTree<N>> {
        Self::ratifications_tree(self.ratifications.keys())
    }

    /// Returns the Merkle tree for the given ratifications.
    fn ratifications_tree<'a>(
        ratifications: impl ExactSizeIterator<Item = &'a N::RatificationID>,
    ) -> Result<RatificationsTree<N>> {
        // Ensure the number of ratifications is within the allowed range.
        ensure!(
            ratifications.len() <= Self::MAX_RATIFICATIONS,
            "Block cannot exceed {} ratifications, found {}",
            Self::MAX_RATIFICATIONS,
            ratifications.len()
        );
        // Prepare the leaves.
        let leaves = ratifications.map(|id| id.to_bits_le());
        // Compute the ratifications tree.
        N::merkle_tree_bhp::<RATIFICATIONS_DEPTH>(&leaves.collect::<Vec<_>>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_ratifications_depth() {
        // Ensure the log2 relationship between depth and the maximum number of ratifications.
        assert_eq!(2usize.pow(RATIFICATIONS_DEPTH as u32), Ratifications::<CurrentNetwork>::MAX_RATIFICATIONS);
    }
}
