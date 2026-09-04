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

//! Reports whether a ledger's historical mapping data can be repaired in place.
//!
//! Opens the database **read-only** and writes nothing, so it is safe to run against a copy of a
//! production ledger without committing to a migration. It answers the question an operator
//! actually has -- "will my node come up, and how long will it take?" -- without the alternative,
//! which is to attempt the repair and find out.
//!
//! ```text
//! snarkvm-history-check ~/.aleo/storage/ledger-0
//! ```

use anyhow::{Result, bail};
use snarkvm_ledger_store::helpers::rocksdb::{PREFIX_LEN, inspect};

/// Entries per second, from measurements on the repair itself. Only used to turn an entry count
/// into a figure an operator can plan around, so it is deliberately conservative.
const ENTRIES_PER_SECOND: f64 = 300_000.0;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        bail!(
            "usage: snarkvm-history-check <ledger-dir> [network-id]\n\nExample: snarkvm-history-check ~/.aleo/storage/ledger-0"
        );
    };

    // The ledger directory is named `ledger-<network id>`, so the network is usually inferable.
    let network_id = match args.next() {
        Some(explicit) => explicit.parse()?,
        None => match std::path::Path::new(&path).file_name().and_then(|name| name.to_str()) {
            Some(name) => match name.rsplit_once('-').and_then(|(_, id)| id.parse().ok()) {
                Some(id) => id,
                None => bail!("Could not infer the network from {name:?}; pass it as the second argument"),
            },
            None => bail!("Could not infer the network from {path:?}; pass it as the second argument"),
        },
    };

    // Read-only, and configured exactly as the node configures it: the fixed-prefix extractor
    // changes how iteration behaves, so inspecting under different options would not be inspecting
    // the same database.
    let mut options = rocksdb::Options::default();
    options.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(PREFIX_LEN));
    let database = rocksdb::DB::open_for_read_only(&options, &path, false)?;

    println!("Inspecting {path} (network {network_id}), read-only...\n");
    let report = inspect(&database, network_id)?;

    if report.schema_version > 0 {
        println!("  schema version   v{} -- already repaired, nothing to do", report.schema_version);
        return Ok(());
    }

    let total = report.little_endian + report.big_endian;
    println!("  schema version   v0 -- repair needed");
    println!("  mapping keys     {}", report.keys);
    println!("  entries          {total} ({} little-endian, {} big-endian)", report.little_endian, report.big_endian);

    match report.undecidable.is_empty() {
        true => {
            let seconds = report.little_endian as f64 / ENTRIES_PER_SECOND;
            println!("  undecidable      0");
            println!("\nThis ledger can be repaired in place. Estimated {:.1} minutes.", seconds / 60.0);
            println!("The repair runs automatically at startup; the node will not serve until it finishes.");
        }
        false => {
            println!("  undecidable      {}", report.undecidable.len());
            println!("\nThis ledger CANNOT be repaired in place and must be resynced from genesis.");
            println!(
                "\n{} entries cannot be attributed to a block height from the database alone.",
                report.undecidable.len()
            );
            println!("Each reads as a plausible height under either encoding, on a key holding both:");
            for (little, big) in report.undecidable.iter().take(10) {
                println!("    height {little} (little-endian) or {big} (big-endian)");
            }
            if report.undecidable.len() > 10 {
                println!("    ... and {} more", report.undecidable.len() - 10);
            }
        }
    }
    Ok(())
}
