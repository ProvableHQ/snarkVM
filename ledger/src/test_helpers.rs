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

pub mod chain_builder;
pub use chain_builder::TestChainBuilder;

use crate::Ledger;
use aleo_std::StorageMode;
use console::{
    account::{Address, PrivateKey, ViewKey},
    network::MainnetV0,
    prelude::*,
};

use snarkvm_circuit::AleoV0;
use snarkvm_ledger_store::ConsensusStore;
use snarkvm_synthesizer::vm::VM;

use once_cell::sync::Lazy;
use snarkvm_ledger_block::Block;

use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use snarkvm_utilities::{FromBytes, ToBytes};

pub use snarkvm_ledger_test_helpers::*;

pub type CurrentNetwork = MainnetV0;
pub type CurrentAleo = AleoV0;

#[cfg(not(feature = "rocks"))]
pub type CurrentLedger = Ledger<CurrentNetwork, snarkvm_ledger_store::helpers::memory::ConsensusMemory<CurrentNetwork>>;
#[cfg(feature = "rocks")]
pub type CurrentLedger = Ledger<CurrentNetwork, snarkvm_ledger_store::helpers::rocksdb::ConsensusDB<CurrentNetwork>>;

#[cfg(not(feature = "rocks"))]
pub type LedgerType = snarkvm_ledger_store::helpers::memory::ConsensusMemory<CurrentNetwork>;
#[cfg(feature = "rocks")]
pub type LedgerType = snarkvm_ledger_store::helpers::rocksdb::ConsensusDB<CurrentNetwork>;

#[cfg(not(feature = "rocks"))]
pub type CurrentConsensusStore =
    ConsensusStore<CurrentNetwork, snarkvm_ledger_store::helpers::memory::ConsensusMemory<CurrentNetwork>>;
#[cfg(feature = "rocks")]
pub type CurrentConsensusStore =
    ConsensusStore<CurrentNetwork, snarkvm_ledger_store::helpers::rocksdb::ConsensusDB<CurrentNetwork>>;

#[cfg(not(feature = "rocks"))]
pub type CurrentConsensusStorage = snarkvm_ledger_store::helpers::memory::ConsensusMemory<CurrentNetwork>;
#[cfg(feature = "rocks")]
pub type CurrentConsensusStorage = snarkvm_ledger_store::helpers::rocksdb::ConsensusDB<CurrentNetwork>;

pub struct TestEnv {
    pub ledger: CurrentLedger,
    pub private_key: PrivateKey<CurrentNetwork>,
    pub view_key: ViewKey<CurrentNetwork>,
    pub address: Address<CurrentNetwork>,
}

pub struct SharedGenesis {
    pub private_key: PrivateKey<CurrentNetwork>,
    pub genesis: Block<CurrentNetwork>,
}

fn genesis_cache_path() -> PathBuf {
    // Prefer the workspace target dir. If CARGO_TARGET_DIR is set, use it.
    // Otherwise, assume snarkvm-ledger is at <workspace>/ledger and target is at <workspace>/target.
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("target"));

    let dir = target_dir.join("test-cache");
    let _ = fs::create_dir_all(&dir);

    // Versioned so you don’t reuse an incompatible cache after upgrades.
    dir.join(format!("snarkvm-ledger_shared_genesis_{}.bin", env!("CARGO_PKG_VERSION")))
}

fn read_shared_genesis_from_file() -> Option<SharedGenesis> {
    let path = genesis_cache_path();
    let mut f = fs::File::open(&path).ok()?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).ok()?;

    let mut slice = bytes.as_slice();
    let private_key = PrivateKey::<CurrentNetwork>::read_le(&mut slice).ok()?;
    let genesis = Block::<CurrentNetwork>::read_le(&mut slice).ok()?;

    Some(SharedGenesis { private_key, genesis })
}

fn write_shared_genesis_to_file(shared: &SharedGenesis) {
    let path = genesis_cache_path();
    let tmp = path.with_extension("tmp");

    let mut buf = Vec::new();
    shared.private_key.write_le(&mut buf).expect("write private key");
    shared.genesis.write_le(&mut buf).expect("write genesis");

    {
        let mut f = fs::File::create(&tmp).expect("create tmp genesis cache file");
        f.write_all(&buf).expect("write tmp genesis cache file");
        let _ = f.sync_all();
    }
    fs::rename(&tmp, &path).expect("rename tmp genesis cache file");
}

fn compute_shared_genesis() -> SharedGenesis {
    let rng = &mut TestRng::default();

    let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
    let store = CurrentConsensusStore::open(StorageMode::new_test(None)).unwrap();
    let genesis = VM::from(store).unwrap().genesis_beacon(&private_key, rng).unwrap();

    SharedGenesis { private_key, genesis }
}

// Process-local cache, backed by the shared on-disk cache.
// This is the piece that makes nextest multi-process runs benefit.
pub static SHARED_GENESIS: Lazy<SharedGenesis> = Lazy::new(|| {
    if let Some(shared) = read_shared_genesis_from_file() {
        return shared;
    }

    let shared = compute_shared_genesis();
    write_shared_genesis_to_file(&shared);
    shared
});

pub fn sample_test_env(_rng: &mut (impl Rng + CryptoRng)) -> TestEnv {
    let shared = &*SHARED_GENESIS;

    let private_key = shared.private_key;
    let view_key = ViewKey::try_from(&private_key).unwrap();
    let address = Address::try_from(&private_key).unwrap();

    let ledger = CurrentLedger::load(shared.genesis.clone(), StorageMode::new_test(None)).unwrap();

    TestEnv { ledger, private_key, view_key, address }
}

pub fn sample_ledger(private_key: PrivateKey<CurrentNetwork>, rng: &mut (impl Rng + CryptoRng)) -> CurrentLedger {
    // Initialize the store.
    let store = CurrentConsensusStore::open(StorageMode::new_test(None)).unwrap();
    // Create a genesis block.
    let genesis = VM::from(store).unwrap().genesis_beacon(&private_key, rng).unwrap();
    // Initialize the ledger with the genesis block.
    let ledger = CurrentLedger::load(genesis.clone(), StorageMode::new_test(None)).unwrap();
    // Ensure the genesis block is correct.
    assert_eq!(genesis, ledger.get_block(0).unwrap());
    // Return the ledger.
    ledger
}
