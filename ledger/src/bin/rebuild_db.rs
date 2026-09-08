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

//! Rebuilds a ledger's finalize state by replaying its blocks.
//!
//! ```text
//! cargo build --release --bin rebuild_db --features rebuild,history
//! ./target/release/rebuild_db ~/.aleo/storage/ledger-0
//! ```
//!
//! Deliberately a binary rather than an example: an example is built with the crate's
//! dev-dependencies unified into the graph, which would hand an operator a tool compiled with
//! `snarkvm-synthesizer/test` -- one that loads the wrong `Process` and caps confirmed transactions
//! at eight. Temporary either way; the operator-facing form is `snarkos developer rebuild-history`,
//! which calls the same `VM::rebuild_finalize_state`. Nothing here is more than argument parsing
//! and a log subscriber.
//!
//! The node must be stopped. RocksDB permits a single writer, so this refuses to open a ledger a
//! node still holds, and the rebuild rewrites the state a running node would be reading.

use snarkvm_console::network::{CanaryV0, MainnetV0, Network, TestnetV0};
use snarkvm_ledger_store::{
    ConsensusStore,
    helpers::rocksdb::{ConsensusDB, allow_downlevel_open},
};
use snarkvm_synthesizer::vm::VM;

use aleo_std::StorageMode;
use anyhow::{Result, bail};
use std::path::PathBuf;

const USAGE: &str = "\
usage: rebuild_db <ledger-dir> [network-id]

Discards the ledger's finalize state and rebuilds it by replaying every block already in
storage. The network is inferred from a directory name ending in `-<id>` (0 = mainnet,
1 = testnet, 2 = canary), or given as the final argument.

Stop the node first. Progress is reported as it runs, and it can be interrupted and resumed.";

/// Returns the network id encoded in a ledger directory name, if it has one.
///
/// Production ledgers are `ledger-{network}`; development ones are `.ledger-{network}-{id}`, so a
/// naive split from the right yields the dev id instead. An id outside the known range is refused
/// rather than assumed, because guessing it wrong would rebuild under the wrong prefix.
fn network_from_name(path: &std::path::Path) -> Option<u16> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix('.').unwrap_or(name).strip_prefix("ledger-")?;
    let network = rest.split('-').next()?.parse().ok()?;
    // 0 = mainnet, 1 = testnet, 2 = canary. Anything else is a misread name.
    (network <= 2).then_some(network)
}

/// Opens the ledger at `path` and rebuilds its finalize state.
fn rebuild<N: Network>(path: PathBuf) -> Result<()> {
    // Said before opening: the schema gate exists to keep a node out of exactly this database.
    allow_downlevel_open();

    let store = ConsensusStore::<N, ConsensusDB<N>>::open(StorageMode::Custom(path))?;
    VM::from(store)?.rebuild_finalize_state()
}

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args.iter().any(|arg| arg == "-h" || arg == "--help") {
        println!("{USAGE}");
        return Ok(());
    }

    let path = PathBuf::from(&args[0]);
    let network_id = match args.get(1) {
        Some(explicit) => explicit.parse()?,
        None => match network_from_name(&path) {
            Some(id) => id,
            None => bail!("Could not infer the network from {path:?}.\n\n{USAGE}"),
        },
    };

    // The rebuild reports progress through `tracing`; without a subscriber a multi-hour run would
    // print nothing at all.
    tracing_subscriber::fmt().with_env_filter("info").with_target(false).init();

    match network_id {
        0 => rebuild::<MainnetV0>(path),
        1 => rebuild::<TestnetV0>(path),
        2 => rebuild::<CanaryV0>(path),
        other => bail!("Unknown network id {other}.\n\n{USAGE}"),
    }
}
