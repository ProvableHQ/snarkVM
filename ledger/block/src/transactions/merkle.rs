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

impl<N: Network> Transactions<N> {
    /// Returns the finalize root of the transactions.
    pub fn to_finalize_root(&self, ratified_finalize_operations: Vec<FinalizeOperation<N>>) -> Result<Field<N>> {
        // Prepare the ratified finalize ID - a Merkle tree composed of the ratified finalize operations.
        let ratified_finalize_id = *N::merkle_tree_bhp::<FINALIZE_ID_DEPTH>(
            &ratified_finalize_operations.iter().map(ToBits::to_bits_le).collect::<Vec<_>>(),
        )?
        .root();

        // Prepare the leaves, composed of:
        // | transaction_0 finalize ID, ..., transaction_n finalize ID | ratified finalize ID |
        let leaves = self
            .iter()
            .map(|tx| tx.to_finalize_id().map(|id| id.to_bits_le()))
            .chain(std::iter::once(Ok(ratified_finalize_id.to_bits_le())))
            .collect::<Result<Vec<_>>>()?;

        // Compute the finalize root.
        // Note: This call will ensure the number of finalize operations is within the size of the Merkle tree.
        Ok(*N::merkle_tree_bhp::<FINALIZE_OPERATIONS_DEPTH>(&leaves)?.root())
    }
}

impl<N: Network> Transactions<N> {
    /// Returns the transactions root, by computing the root for a Merkle tree of the transaction IDs.
    pub fn to_transactions_root(&self) -> Result<Field<N>> {
        Ok(*self.to_tree()?.root())
    }

    /// Returns the Merkle path for the transactions leaf.
    pub fn to_path(&self, transaction_id: N::TransactionID) -> Result<TransactionsPath<N>> {
        match self.transactions.get_index_of(&transaction_id) {
            Some(transaction_index) => self.to_tree()?.prove(transaction_index, &transaction_id.to_bits_le()),
            None => bail!("The transaction '{transaction_id}' is not in the block transactions"),
        }
    }

    /// The Merkle tree of transaction IDs for the block.
    pub fn to_tree(&self) -> Result<TransactionsTree<N>> {
        Self::transactions_tree(&self.transactions)
    }

    /// Returns the Merkle tree for the given transactions.
    fn transactions_tree(
        transactions: &IndexMap<N::TransactionID, ConfirmedTransaction<N>>,
    ) -> Result<TransactionsTree<N>> {
        // Ensure the number of transactions is within the allowed range.
        ensure!(
            transactions.len() <= Self::MAX_TRANSACTIONS,
            "Block cannot exceed {} transactions, found {}",
            Self::MAX_TRANSACTIONS,
            transactions.len()
        );
        // Prepare the leaves.
        let leaves = transactions.values().map(|transaction| transaction.id().to_bits_le());
        // Compute the transactions tree.
        N::merkle_tree_bhp::<TRANSACTIONS_DEPTH>(&leaves.collect::<Vec<_>>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_transactions_depth() {
        // Ensure the log2 relationship between depth and the maximum number of transactions.
        // Note: This test uses 'checked_sub' to ensure the depth is not zero.
        assert_eq!(
            2usize.pow(TRANSACTIONS_DEPTH as u32).checked_sub(1).expect("Invalid depth"),
            Transactions::<CurrentNetwork>::MAX_TRANSACTIONS
        );
    }
}
