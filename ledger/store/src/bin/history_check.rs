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
//! actually has -- "will this work, and how long will it take?" -- rather than the only alternative,
//! which is to attempt the repair and find out.
//!
//! ```text
//! snarkvm-history-check ~/.aleo/storage/ledger-0
//! ```

use anyhow::Result;
use snarkvm_ledger_store::helpers::rocksdb::plan;

mod common;

/// Entries per second, measured on the migration itself. Only used to turn a count into a figure an
/// operator can plan around, so it is deliberately conservative.
const ENTRIES_PER_SECOND: f64 = 300_000.0;

fn main() -> Result<()> {
    let (path, network_id) = common::arguments("snarkvm-history-check")?;
    let database = common::open_read_only(&path)?;

    println!("Inspecting {path} (network {network_id}), read-only...\n");
    let report = plan(&database, network_id)?;

    if report.schema_version > 0 {
        println!("  schema version   v{} -- already migrated, nothing to do", report.schema_version);
        return Ok(());
    }

    let total = report.little_endian + report.big_endian;
    println!("  schema version   v0 -- migration needed");
    println!("  chain tip        {} (derived from the entries themselves)", report.tip);
    println!("  mapping keys     {}", report.keys);
    println!(
        "  entries          {total} ({} to migrate, {} already migrated)",
        report.little_endian, report.big_endian
    );

    match report.undecidable.is_empty() {
        true => {
            let minutes = report.little_endian as f64 / ENTRIES_PER_SECOND / 60.0;
            println!("  undecidable      0\n");
            println!("This ledger can be migrated in place. Estimated {minutes:.1} minutes.\n");
            println!("    snarkvm-migrate-db {path}");
        }
        false => {
            println!("  undecidable      {}\n", report.undecidable.len());
            println!("This ledger CANNOT be migrated and must be resynced from genesis.\n");
            println!(
                "{} entries cannot be attributed to a block height. Each reads as a plausible",
                report.undecidable.len()
            );
            println!("height under either encoding, on a key that holds both:");
            for entry in report.undecidable.iter().take(10) {
                println!("    height {} (little-endian) or {} (big-endian)", entry.little, entry.big);
            }
            if report.undecidable.len() > 10 {
                println!("    ... and {} more", report.undecidable.len() - 10);
            }
        }
    }
    Ok(())
}
