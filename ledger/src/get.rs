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

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Returns the committee for the given `block height`.
    pub fn get_committee(&self, block_height: u32) -> Result<Option<Committee<N>>> {
        self.vm.finalize_store().committee_store().get_committee(block_height)
    }

    /// Returns the committee for the given `round`.
    pub fn get_committee_for_round(&self, round: u64) -> Result<Option<Committee<N>>> {
        self.vm.finalize_store().committee_store().get_committee_for_round(round)
    }

    /// Returns the committee lookback for the given round.
    pub fn get_committee_lookback_for_round(&self, round: u64) -> Result<Option<Committee<N>>> {
        // Get the round number for the previous committee. Note, we subtract 2 from odd rounds,
        // because committees are updated in even rounds.
        let previous_round = match round % 2 == 0 {
            true => round.saturating_sub(1),
            false => round.saturating_sub(2),
        };

        // Get the committee lookback round.
        let committee_lookback_round = previous_round.saturating_sub(Committee::<N>::COMMITTEE_LOOKBACK_RANGE);

        // Retrieve the committee for the committee lookback round.
        self.get_committee_for_round(committee_lookback_round)
    }

    /// Returns the state root that contains the given `block height`.
    pub fn get_state_root(&self, block_height: u32) -> Result<Option<N::StateRoot>> {
        self.vm.block_store().get_state_root(block_height)
    }

    /// Returns a state path for the given commitment.
    pub fn get_state_path_for_commitment(&self, commitment: &Field<N>) -> Result<StatePath<N>> {
        self.vm.block_store().get_state_path_for_commitment(commitment)
    }

    /// Returns the epoch hash for the given block height.
    pub fn get_epoch_hash(&self, block_height: u32) -> Result<N::BlockHash> {
        // Compute the epoch number from the current block height.
        let epoch_number = block_height.saturating_div(N::NUM_BLOCKS_PER_EPOCH);
        // Compute the epoch starting height (a multiple of `NUM_BLOCKS_PER_EPOCH`).
        let epoch_starting_height = epoch_number.saturating_mul(N::NUM_BLOCKS_PER_EPOCH);
        // Retrieve the epoch hash, defined as the 'previous block hash' from the epoch starting height.
        let epoch_hash = self.get_previous_hash(epoch_starting_height)?;
        // Construct the epoch hash.
        Ok(epoch_hash)
    }

    /// Returns the block for the given block height.
    pub fn get_block(&self, height: u32) -> Result<Block<N>> {
        // If the height is 0, return the genesis block.
        if height == 0 {
            return Ok(self.genesis_block.clone());
        }
        // Retrieve the block hash.
        let block_hash = match self.vm.block_store().get_block_hash(height)? {
            Some(block_hash) => block_hash,
            None => bail!("Block {height} does not exist in storage"),
        };
        // Retrieve the block.
        match self.vm.block_store().get_block(&block_hash)? {
            Some(block) => Ok(block),
            None => bail!("Block {height} ('{block_hash}') does not exist in storage"),
        }
    }

    /// Returns the blocks in the given block range.
    /// The range is inclusive of the start and exclusive of the end.
    pub fn get_blocks(&self, heights: Range<u32>) -> Result<Vec<Block<N>>> {
        cfg_into_iter!(heights).map(|height| self.get_block(height)).collect()
    }

    /// Returns the block for the given block hash.
    pub fn get_block_by_hash(&self, block_hash: &N::BlockHash) -> Result<Block<N>> {
        // Retrieve the block.
        match self.vm.block_store().get_block(block_hash)? {
            Some(block) => Ok(block),
            None => bail!("Block '{block_hash}' does not exist in storage"),
        }
    }

    /// Returns the block height for the given block hash.
    pub fn get_height(&self, block_hash: &N::BlockHash) -> Result<u32> {
        match self.vm.block_store().get_block_height(block_hash)? {
            Some(height) => Ok(height),
            None => bail!("Missing block height for block '{block_hash}'"),
        }
    }

    /// Returns the block hash for the given block height.
    pub fn get_hash(&self, height: u32) -> Result<N::BlockHash> {
        // If the height is 0, return the genesis block hash.
        if height == 0 {
            return Ok(self.genesis_block.hash());
        }
        match self.vm.block_store().get_block_hash(height)? {
            Some(block_hash) => Ok(block_hash),
            None => bail!("Missing block hash for block {height}"),
        }
    }

    /// Returns the previous block hash for the given block height.
    pub fn get_previous_hash(&self, height: u32) -> Result<N::BlockHash> {
        // If the height is 0, return the default block hash.
        if height == 0 {
            return Ok(N::BlockHash::default());
        }
        match self.vm.block_store().get_previous_block_hash(height)? {
            Some(previous_hash) => Ok(previous_hash),
            None => bail!("Missing previous block hash for block {height}"),
        }
    }

    /// Returns the block header for the given block height.
    pub fn get_header(&self, height: u32) -> Result<Header<N>> {
        // If the height is 0, return the genesis block header.
        if height == 0 {
            return Ok(*self.genesis_block.header());
        }
        // Retrieve the block hash.
        let block_hash = match self.vm.block_store().get_block_hash(height)? {
            Some(block_hash) => block_hash,
            None => bail!("Block {height} does not exist in storage"),
        };
        // Retrieve the block header.
        match self.vm.block_store().get_block_header(&block_hash)? {
            Some(header) => Ok(header),
            None => bail!("Missing block header for block {height}"),
        }
    }

    /// Returns the block transactions for the given block height.
    pub fn get_transactions(&self, height: u32) -> Result<Transactions<N>> {
        // If the height is 0, return the genesis block transactions.
        if height == 0 {
            return Ok(self.genesis_block.transactions().clone());
        }
        // Retrieve the block hash.
        let Some(block_hash) = self.vm.block_store().get_block_hash(height)? else {
            bail!("Block {height} does not exist in storage");
        };
        // Retrieve the block transaction.
        match self.vm.block_store().get_block_transactions(&block_hash)? {
            Some(transactions) => Ok(transactions),
            None => bail!("Missing block transactions for block {height}"),
        }
    }

    /// Returns the aborted transaction IDs for the given block height.
    pub fn get_aborted_transaction_ids(&self, height: u32) -> Result<Vec<N::TransactionID>> {
        // If the height is 0, return the genesis block aborted transaction IDs.
        if height == 0 {
            return Ok(self.genesis_block.aborted_transaction_ids().clone());
        }
        // Retrieve the block hash.
        let Some(block_hash) = self.vm.block_store().get_block_hash(height)? else {
            bail!("Block {height} does not exist in storage");
        };
        // Retrieve the aborted transaction IDs.
        match self.vm.block_store().get_block_aborted_transaction_ids(&block_hash)? {
            Some(aborted_transaction_ids) => Ok(aborted_transaction_ids),
            None => bail!("Missing aborted transaction IDs for block {height}"),
        }
    }

    /// Returns the transaction for the given transaction ID.
    pub fn get_transaction(&self, transaction_id: N::TransactionID) -> Result<Transaction<N>> {
        // Retrieve the transaction.
        match self.vm.block_store().get_transaction(&transaction_id)? {
            Some(transaction) => Ok(transaction),
            None => bail!("Missing transaction for ID {transaction_id}"),
        }
    }

    /// Returns the confirmed transaction for the given transaction ID.
    pub fn get_confirmed_transaction(&self, transaction_id: N::TransactionID) -> Result<ConfirmedTransaction<N>> {
        // Retrieve the confirmed transaction.
        match self.vm.block_store().get_confirmed_transaction(&transaction_id)? {
            Some(confirmed_transaction) => Ok(confirmed_transaction),
            None => bail!("Missing confirmed transaction for ID {transaction_id}"),
        }
    }

    /// Returns the unconfirmed transaction for the given `transaction ID`.
    pub fn get_unconfirmed_transaction(&self, transaction_id: &N::TransactionID) -> Result<Transaction<N>> {
        // Retrieve the unconfirmed transaction.
        match self.vm.block_store().get_unconfirmed_transaction(transaction_id)? {
            Some(unconfirmed_transaction) => Ok(unconfirmed_transaction),
            None => bail!("Missing unconfirmed transaction for ID {transaction_id}"),
        }
    }

    /// Returns the program for the given program ID.
    pub fn get_program(&self, program_id: ProgramID<N>) -> Result<Program<N>> {
        match self.vm.block_store().get_program(&program_id)? {
            Some(program) => Ok(program),
            None => bail!("Missing program for ID {program_id}"),
        }
    }

    /// Returns the block solutions for the given block height.
    pub fn get_solutions(&self, height: u32) -> Result<Solutions<N>> {
        // If the height is 0, return the genesis block solutions.
        if height == 0 {
            return Ok(self.genesis_block.solutions().clone());
        }
        // Retrieve the block hash.
        let block_hash = match self.vm.block_store().get_block_hash(height)? {
            Some(block_hash) => block_hash,
            None => bail!("Block {height} does not exist in storage"),
        };
        // Retrieve the block solutions.
        self.vm.block_store().get_block_solutions(&block_hash)
    }

    /// Returns the solution for the given solution ID.
    pub fn get_solution(&self, solution_id: &SolutionID<N>) -> Result<Solution<N>> {
        self.vm.block_store().get_solution(solution_id)
    }

    /// Returns the block authority for the given block height.
    pub fn get_authority(&self, height: u32) -> Result<Authority<N>> {
        // If the height is 0, return the genesis block authority.
        if height == 0 {
            return Ok(self.genesis_block.authority().clone());
        }
        // Retrieve the block hash.
        let block_hash = match self.vm.block_store().get_block_hash(height)? {
            Some(block_hash) => block_hash,
            None => bail!("Block {height} does not exist in storage"),
        };
        // Retrieve the block authority.
        match self.vm.block_store().get_block_authority(&block_hash)? {
            Some(authority) => Ok(authority),
            None => bail!("Missing authority for block {height}"),
        }
    }

    /// Returns the batch certificate for the given `certificate ID`.
    pub fn get_batch_certificate(&self, certificate_id: &Field<N>) -> Result<Option<BatchCertificate<N>>> {
        self.vm.block_store().get_batch_certificate(certificate_id)
    }

    /// Returns the delegators for the given validator.
    pub fn get_delegators_for_validator(&self, validator: &Address<N>) -> Result<Vec<Address<N>>> {
        // Construct the credits.aleo program ID.
        let credits_program_id = ProgramID::from_str("credits.aleo")?;
        // Construct the bonded mapping name.
        let bonded_mapping = Identifier::from_str("bonded")?;
        // Construct the bonded mapping key name.
        let bonded_mapping_key = Identifier::from_str("validator")?;
        // Get the credits.aleo bonded mapping.
        let bonded = self.vm.finalize_store().get_mapping_confirmed(credits_program_id, bonded_mapping)?;
        // Select the delegators for the given validator.
        cfg_into_iter!(bonded)
            .filter_map(|(bonded_address, bond_state)| {
                let Plaintext::Literal(Literal::Address(bonded_address), _) = bonded_address else {
                    return Some(Err(anyhow!("Invalid delegator in finalize storage.")));
                };
                let Value::Plaintext(Plaintext::Struct(bond_state, _)) = bond_state else {
                    return Some(Err(anyhow!("Invalid bond_state in finalize storage.")));
                };
                let Some(mapping_validator) = bond_state.get(&bonded_mapping_key) else {
                    return Some(Err(anyhow!("Invalid bond_state validator in finalize storage.")));
                };
                let Plaintext::Literal(Literal::Address(mapping_validator), _) = mapping_validator else {
                    return Some(Err(anyhow!("Invalid validator in finalize storage.")));
                };
                // Select bonded addresses which:
                // 1. are bonded to the right validator.
                // 2. are not themselves the validator.
                (mapping_validator == validator && bonded_address != *validator).then_some(Ok(bonded_address))
            })
            .collect::<Result<_>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::CurrentLedger;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_get_block() {
        // Load the genesis block.
        let genesis = Block::from_bytes_le(CurrentNetwork::genesis_bytes()).unwrap();

        // Initialize a new ledger.
        let ledger = CurrentLedger::load(genesis.clone(), StorageMode::Production).unwrap();
        // Retrieve the genesis block.
        let candidate = ledger.get_block(0).unwrap();
        // Ensure the genesis block matches.
        assert_eq!(genesis, candidate);
    }
}
