// Copyright (c) 2019-2026 Provable Inc.
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

//! Times `prepare_advance_to_next_quorum_block`, `check_next_block`, and `advance_to_next_block`
//! against one full block of pregenerated `transfer_public` executions.
//!
//! A full block is `BatchHeader::MAX_TRANSMISSIONS_PER_BATCH * PREGENERATED_NUM_VALIDATORS`
//! transmissions (default 4 validators).
//!
//! Load executions from `PREGENERATED_TX_DIR` (default `./transaction_files`). Each
//! `executions-*.txt` file is one transaction string per line.
//!
//! `PREGENERATED_TX_LIMIT` can load fewer executions (`0` means one full block).

use std::{
    collections::HashSet,
    env,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::Context;
use snarkvm_console::{
    network::{TEST_CONSENSUS_VERSION_HEIGHTS, TestnetV0},
    prelude::*,
    program::{Identifier, Literal, Plaintext, ProgramID, Value},
    types::U64,
};
use snarkvm_ledger::{
    narwhal::BatchHeader,
    test_helpers::{
        TestChainBuilder,
        chain_builder::{GenerateBlockOptions, GenerateBlocksOptions},
    },
};
use snarkvm_ledger_block::Transaction;
use snarkvm_synthesizer::program::FinalizeStoreTrait;
use snarkvm_utilities::PrettyUnwrap;

type CurrentNetwork = TestnetV0;

fn print_bencher(name: &str, elapsed: Duration) {
    println!("test {name} ... bench: {} ns/iter (+/- 0)", elapsed.as_nanos());
}

fn visit_execution_files(dir: &Path, lines: &mut Vec<String>, limit: usize) -> Result<()> {
    if lines.len() >= limit {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            visit_execution_files(&path, lines, limit)?;
            if lines.len() >= limit {
                return Ok(());
            }
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("executions-") || !name.ends_with(".txt") {
            continue;
        }
        let content = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        for line in content.lines() {
            if lines.len() >= limit {
                return Ok(());
            }
            let line = line.trim();
            if !line.is_empty() {
                lines.push(line.to_string());
            }
        }
    }
    Ok(())
}

fn load_executions(dir: &Path, limit: usize) -> Result<Vec<Transaction<CurrentNetwork>>> {
    ensure!(limit > 0, "execution limit must be positive");
    let mut lines = Vec::new();
    visit_execution_files(dir, &mut lines, limit)?;
    ensure!(!lines.is_empty(), "no executions-*.txt files under {}", dir.display());
    lines.truncate(limit);

    let mut txs = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let tx = Transaction::<CurrentNetwork>::from_str(line)
            .with_context(|| format!("parsing execution {i} from {}", dir.display()))?;
        txs.push(tx);
    }
    Ok(txs)
}

fn fund_payers(builder: &TestChainBuilder<CurrentNetwork>, txs: &[Transaction<CurrentNetwork>]) -> Result<()> {
    let program_id = ProgramID::from_str("credits.aleo")?;
    let mapping = Identifier::from_str("account")?;
    let balance = Value::from(Literal::U64(U64::new(1_000_000_000_000_000)));
    let store = builder.ledger().vm().finalize_store();
    let mut funded = HashSet::new();
    for tx in txs {
        let Some(fee) = tx.fee_transition() else {
            continue;
        };
        let Some(payer) = fee.payer() else {
            continue;
        };
        if !funded.insert(payer) {
            continue;
        }
        store.update_key_value(program_id, mapping, Plaintext::from(Literal::Address(payer)), balance.clone())?;
    }
    Ok(())
}

fn main() {
    let tx_dir =
        env::var("PREGENERATED_TX_DIR").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("transaction_files"));
    let num_validators = env::var("PREGENERATED_NUM_VALIDATORS").ok().and_then(|s| s.parse().ok()).unwrap_or(4usize);
    let chunk_size = BatchHeader::<CurrentNetwork>::MAX_TRANSMISSIONS_PER_BATCH.saturating_mul(num_validators).max(1);
    let tx_limit = env::var("PREGENERATED_TX_LIMIT").ok().and_then(|s| s.parse().ok()).unwrap_or(0usize);
    let load_limit = if tx_limit == 0 { chunk_size } else { tx_limit.min(chunk_size) };

    println!("Loading executions from {} (limit={load_limit}, full block={chunk_size})", tx_dir.display());
    let txs = load_executions(&tx_dir, load_limit).pretty_expect("Failed to load pregenerated executions");
    println!("Loaded {} executions", txs.len());

    let rng = &mut TestRng::default();
    let mut builder = TestChainBuilder::<CurrentNetwork>::new_with_quorum_size(num_validators, rng)
        .pretty_expect("Failed to initialize the test chain");

    let warmup_height = TEST_CONSENSUS_VERSION_HEIGHTS.last().unwrap().1 as usize;
    builder
        .generate_blocks_with_opts(
            warmup_height,
            GenerateBlocksOptions { skip_to_current_version: true, num_validators, ..Default::default() },
            rng,
        )
        .pretty_expect("Failed to skip to the current consensus version");

    fund_payers(&builder, &txs).pretty_expect("Failed to fund execution fee payers");

    let loaded = txs.len();
    let (subdag, transmissions, leader_certificate) = builder
        .build_quorum_subdag_and_transmissions_for_next_block(
            GenerateBlockOptions { transactions: txs, ..Default::default() },
            rng,
        )
        .pretty_expect("Failed to build the quorum subdag");

    let start = Instant::now();
    let block = builder
        .ledger()
        .prepare_advance_to_next_quorum_block(subdag, transmissions, rng)
        .unwrap_or_else(|err| panic!("prepare_advance_to_next_quorum_block failed: {err}"));
    let prepare_total = start.elapsed();

    let start = Instant::now();
    builder.ledger().check_next_block(&block, rng).unwrap_or_else(|err| panic!("check_next_block failed: {err}"));
    let check_total = start.elapsed();

    let start = Instant::now();
    builder.apply_prepared_quorum_block(&block, leader_certificate).pretty_expect("advance_to_next_block failed");
    let advance_total = start.elapsed();

    let accepted = block.transactions().num_accepted();
    ensure_accepted(accepted, loaded);
    println!("Processed 1 block; accepted {accepted} of {loaded} executions");
    print_bencher("pregenerated/prepare_advance_to_next_quorum_block", prepare_total);
    print_bencher("pregenerated/check_next_block", check_total);
    print_bencher("pregenerated/advance_to_next_block", advance_total);
}

fn ensure_accepted(accepted: usize, loaded: usize) {
    if accepted == 0 {
        panic!("pregenerated advance accepted 0 of {loaded} executions");
    }
}
