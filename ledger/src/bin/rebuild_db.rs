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
//! Must stay a binary, not an example: an example is built with this crate's dev-dependencies
//! unified into the graph, which compiles it with `snarkvm-synthesizer/test` -- the wrong `Process`
//! for the tip height, and confirmed transactions capped at eight.
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
usage: rebuild_db [--check] [--force] <ledger-dir> [network-id]

Discards the ledger's finalize state and rebuilds it by replaying every block already in
storage. The network is inferred from a directory name ending in `-<id>` (0 = mainnet,
1 = testnet, 2 = canary), or given as the final argument.

--force rebuilds a ledger that already records this schema version, discarding a finalize
state nothing has reported as wrong. Only for a ledger whose history is suspect for some
other reason.

--check runs every pre-flight the rebuild runs and stops before the first write, reporting
whether this ledger is a candidate and how much of it there is to replay. It still needs the
node stopped, since opening the ledger takes RocksDB's single writer lock.

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
fn rebuild<N: Network>(path: PathBuf, check: bool, force: bool) -> Result<()> {
    // Said before opening: the schema gate exists to keep a node out of exactly this database.
    allow_downlevel_open();

    let store = ConsensusStore::<N, ConsensusDB<N>>::open(StorageMode::Custom(path))?;
    // Without preloaded deployments: the replay adds each program as it reaches its deployment, so
    // a program revised later is checked against the edition the block carries.
    let vm = VM::from_without_deployments(store)?;
    match (check, force) {
        (true, _) => vm.check_rebuild(),
        (false, true) => vm.force_rebuild_finalize_state(),
        (false, false) => vm.rebuild_finalize_state(),
    }
}

fn main() -> Result<()> {
    let all = std::env::args().skip(1).collect::<Vec<_>>();
    if all.is_empty() || all.iter().any(|arg| arg == "-h" || arg == "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    let check = all.iter().any(|arg| arg == "--check" || arg == "-c");
    let force = all.iter().any(|arg| arg == "--force");
    let args = all.into_iter().filter(|arg| !arg.starts_with('-')).collect::<Vec<_>>();
    if args.is_empty() {
        bail!("No ledger directory given.\n\n{USAGE}");
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
    // print nothing at all. `RUST_LOG` overrides the default, so a stalled run can be turned up.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).with_target(false).init();

    match network_id {
        0 => rebuild::<MainnetV0>(path, check, force),
        1 => rebuild::<TestnetV0>(path, check, force),
        2 => rebuild::<CanaryV0>(path, check, force),
        other => bail!("Unknown network id {other}.\n\n{USAGE}"),
    }
}
