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

mod helpers;
use helpers::{BlockOptions, CurrentNetwork, LedgerType, TestChainBuilder};

use snarkvm_ledger::Ledger;

use aleo_std::StorageMode;
use snarkvm_utilities::TestRng;

#[test]
fn test_preprocess_block() {
    let rng = &mut TestRng::default();

    let mut builder = TestChainBuilder::new(4, rng);

    // Construct the ledger.
    let ledger = Ledger::<CurrentNetwork, LedgerType<CurrentNetwork>>::load(
        builder.genesis_block().clone(),
        StorageMode::new_test(None),
    )
    .unwrap();

    // Generate a bunch of blocks that do not contain votes
    let mut pending_blocks = vec![];

    for block in builder.generate_blocks_with_opts(5, &BlockOptions { skip_votes: true, ..Default::default() }, rng) {
        if !pending_blocks.is_empty() {
            // We shoud only be able to pre-process a pending block if the previous
            // blocks are applied to the ledger or in the pending set
            assert!(ledger.check_block_subdag(block.clone(), &[]).is_err());
        }

        let pending_block = ledger.check_block_subdag(block, &pending_blocks).unwrap();
        pending_blocks.push(pending_block);
    }

    // Now, create a "vote block" that contains sufficient votes to the previous leader block
    let vote_block = builder.generate_block(rng);
    assert!(ledger.check_next_block(&vote_block, rng).is_err());

    for pending in pending_blocks.into_iter() {
        let block = ledger.check_block_content(pending, rng).expect("Pending block should be accepted");
        assert!(ledger.advance_to_next_block(&block).is_ok());
    }

    // Now the commit block should be accepted
    assert!(ledger.check_next_block(&vote_block, rng).is_ok());
    assert!(ledger.advance_to_next_block(&vote_block).is_ok());
}
