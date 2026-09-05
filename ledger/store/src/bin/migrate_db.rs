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

//! Brings a ledger's storage schema up to the version this build requires.
//!
//! Separate from the node deliberately. The work can rewrite billions of entries and take hours,
//! which is not something a node should do as a side effect of starting: an operator wants to run
//! it when they choose, against a copy first, watching it, able to stop and resume it. The node
//! only checks the version and refuses to run against a schema it does not understand.
//!
//! ```text
//! snarkvm-migrate-db --check ~/.aleo/storage/ledger-0   # read-only: what would this do?
//! snarkvm-migrate-db         ~/.aleo/storage/ledger-0   # do it
//! ```
//!
//! `--check` opens the database read-only and writes nothing, so it is safe against a copy of a
//! production ledger. Without it the node must be stopped: RocksDB permits a single writer, so a
//! migration will refuse to open a ledger a node still holds.

use anyhow::{Result, bail};
use snarkvm_ledger_store::helpers::rocksdb::{PREFIX_LEN, migrate, plan};

/// Entries per second, measured on the migration itself. Only used to turn a count into a figure an
/// operator can plan around, so it is deliberately conservative.
const ENTRIES_PER_SECOND: f64 = 300_000.0;

/// The ledger to act on, and whether to only report.
struct Args {
    path: String,
    network_id: u16,
    check: bool,
}

const USAGE: &str = "\
usage: snarkvm-migrate-db [--check] <ledger-dir> [network-id]

    snarkvm-migrate-db --check ~/.aleo/storage/ledger-0    report what a migration would do
    snarkvm-migrate-db         ~/.aleo/storage/ledger-0    perform it

--check opens the ledger read-only and writes nothing. Without it, stop the node first.
The network is inferred from a directory name ending in `-<id>` (0 = mainnet, 1 = testnet,
2 = canary), or given as the final argument.";

fn parse() -> Result<Args> {
    let mut check = false;
    let mut positional = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--check" | "-c" => check = true,
            // Asking for help is not an error.
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            flag if flag.starts_with('-') => bail!("Unrecognized option {flag:?}\n\n{USAGE}"),
            _ => positional.push(arg),
        }
    }

    let Some(path) = positional.first().cloned() else { bail!("{USAGE}") };
    let network_id = match positional.get(1) {
        Some(explicit) => explicit.parse()?,
        None => {
            let name = std::path::Path::new(&path).file_name().and_then(|name| name.to_str());
            match name.and_then(|name| name.rsplit_once('-')).and_then(|(_, id)| id.parse().ok()) {
                Some(id) => id,
                None => bail!("Could not infer the network from {path:?}.\n\n{USAGE}"),
            }
        }
    };
    Ok(Args { path, network_id, check })
}

/// The options a node opens the ledger with.
///
/// The prefix extractor matters: it puts iteration into prefix-seek mode, so inspecting or
/// migrating under different options would not be operating on the database as the node sees it.
fn options() -> rocksdb::Options {
    let mut options = rocksdb::Options::default();
    options.set_compression_type(rocksdb::DBCompressionType::Lz4);
    options.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(PREFIX_LEN));
    options
}

fn main() -> Result<()> {
    let args = parse()?;

    // Read-only for --check, so that inspecting a ledger cannot modify it. Note this may read a
    // database whose write-ahead log has not been replayed, so on a ledger left by an unclean
    // shutdown the very latest entries may not be visible.
    let database = match args.check {
        true => rocksdb::DB::open_for_read_only(&options(), &args.path, false)?,
        false => rocksdb::DB::open(&options(), &args.path)?,
    };

    let report = plan(&database, args.network_id)?;
    if report.schema_version > 0 {
        println!("{} is at storage schema v{}. Nothing to do.", args.path, report.schema_version);
        return Ok(());
    }

    let total = report.little_endian + report.big_endian;
    println!("{} (network {})\n", args.path, args.network_id);
    println!("  chain tip        {} (derived from the entries themselves)", report.tip);
    println!("  mapping keys     {}", report.keys);
    println!(
        "  entries          {total} ({} to migrate, {} already migrated)",
        report.little_endian, report.big_endian
    );
    println!("  undecidable      {}\n", report.undecidable.len());

    if !report.undecidable.is_empty() {
        println!("This ledger CANNOT be migrated and must be resynced from genesis.\n");
        println!("{} entries cannot be attributed to a block height. Each reads as a", report.undecidable.len());
        println!("plausible height under either encoding, on a key that holds both:");
        for entry in report.undecidable.iter().take(10) {
            println!("    height {} (little-endian) or {} (big-endian)", entry.little, entry.big);
        }
        if report.undecidable.len() > 10 {
            println!("    ... and {} more", report.undecidable.len() - 10);
        }
        // A refusal is a refusal in either mode; nothing has been modified.
        bail!("{} entries cannot be attributed to a block height", report.undecidable.len());
    }

    let minutes = report.little_endian as f64 / ENTRIES_PER_SECOND / 60.0;
    if args.check {
        println!("This ledger can be migrated in place. Estimated {minutes:.1} minutes.\n");
        println!("    snarkvm-migrate-db {}", args.path);
        return Ok(());
    }

    tracing_subscriber::fmt().with_env_filter("info").with_target(false).init();
    println!("Migrating. Estimated {minutes:.1} minutes; safe to interrupt and resume.\n");
    migrate(&database, args.network_id)?;
    println!("\nMigration complete. The node can now be started.");
    Ok(())
}
