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

//! Argument handling and database opening, shared by the storage tools.
//!
//! Each binary includes this module separately, so a helper only one of them uses looks unused from
//! the other's point of view.
#![allow(dead_code)]

use anyhow::{Result, bail};
use snarkvm_ledger_store::helpers::rocksdb::PREFIX_LEN;

/// Returns the ledger path and network id from the command line.
///
/// The ledger directory is named `ledger-<network id>`, so the network is usually inferable; it can
/// be given explicitly as a second argument when it is not.
pub fn arguments(program: &str) -> Result<(String, u16)> {
    let mut args = std::env::args().skip(1);
    let path = match args.next() {
        Some(path) if !path.starts_with('-') => path,
        _ => bail!(
            "usage: {program} <ledger-dir> [network-id]\n\n  \
             {program} ~/.aleo/storage/ledger-0\n\n\
             The network is inferred from the directory name when it ends in `-<id>`\n\
             (0 = mainnet, 1 = testnet, 2 = canary)."
        ),
    };
    let network_id = match args.next() {
        Some(explicit) => explicit.parse()?,
        None => {
            let name = std::path::Path::new(&path).file_name().and_then(|name| name.to_str());
            match name.and_then(|name| name.rsplit_once('-')).and_then(|(_, id)| id.parse().ok()) {
                Some(id) => id,
                None => bail!(
                    "Could not infer the network from {path:?}. Pass it as the second argument \
                     (0 = mainnet, 1 = testnet, 2 = canary)."
                ),
            }
        }
    };
    Ok((path, network_id))
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

/// Opens the ledger for reading and writing, failing if a node still holds it.
pub fn open(path: &str) -> Result<rocksdb::DB> {
    Ok(rocksdb::DB::open(&options(), path)?)
}

/// Opens the ledger read-only, so that inspecting it cannot modify it.
///
/// Note this may read a database whose write-ahead log has not been replayed, so on a ledger left
/// by an unclean shutdown the very latest entries may not be visible.
pub fn open_read_only(path: &str) -> Result<rocksdb::DB> {
    Ok(rocksdb::DB::open_for_read_only(&options(), path, false)?)
}
