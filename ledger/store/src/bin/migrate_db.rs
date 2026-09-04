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
//! snarkvm-history-check ~/.aleo/storage/ledger-0   # read-only, reports what this would do
//! snarkvm-migrate-db    ~/.aleo/storage/ledger-0   # performs it
//! ```
//!
//! **The node must be stopped.** RocksDB permits a single writer, so this will refuse to open a
//! ledger a node still holds.

use anyhow::{Result, bail};
use snarkvm_ledger_store::helpers::rocksdb::{migrate, plan};

mod common;

fn main() -> Result<()> {
    let (path, network_id) = common::arguments("snarkvm-migrate-db")?;

    tracing_subscriber::fmt().with_env_filter("info").with_target(false).init();

    // Opened read-write, which also serves as the check that no node holds the ledger.
    let database = common::open(&path)?;

    let report = plan(&database, network_id)?;
    if report.schema_version > 0 {
        println!("{path} is already at storage schema v{}. Nothing to do.", report.schema_version);
        return Ok(());
    }
    if !report.undecidable.is_empty() {
        let first = &report.undecidable[0];
        bail!(
            "{} historical mapping entries cannot be attributed to a block height -- the first \
             reads as {} little-endian and {} big-endian, and its key holds entries in both \
             encodings. Nothing has been modified. Run snarkvm-history-check for the full list. \
             This ledger must be resynced from genesis.",
            report.undecidable.len(),
            first.little,
            first.big
        );
    }

    println!("Migrating {path} (network {network_id})");
    println!("  {} entries to migrate across {} keys", report.little_endian, report.keys);
    println!("  {} entries are already migrated\n", report.big_endian);
    println!("This may take hours on an archive node. It is safe to interrupt: the migration");
    println!("records its progress and resumes from where it stopped.\n");

    migrate(&database, network_id)?;

    println!("\nMigration complete. The node can now be started.");
    Ok(())
}
