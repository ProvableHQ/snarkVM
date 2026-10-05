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

//! Callgrind measurements of the account operations in `account.rs`.
//!
//! Each case runs once. Rayon is pinned to one worker so the instruction count does not follow the pool size.

use snarkvm_console_account::{Address, PrivateKey, ViewKey};
use snarkvm_console_network::{MainnetV0, environment::prelude::*};

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

type CurrentNetwork = MainnetV0;

const SEED: u64 = 0xA11C_0A17;

fn setup_rng() -> TestRng {
    TestRng::from_seed(SEED)
}

fn setup_private_key() -> PrivateKey<CurrentNetwork> {
    PrivateKey::new(&mut setup_rng()).unwrap()
}

#[library_benchmark]
#[bench::new(setup_rng())]
fn account_private_key(mut rng: TestRng) -> PrivateKey<CurrentNetwork> {
    black_box(PrivateKey::new(black_box(&mut rng)).unwrap())
}

#[library_benchmark]
#[bench::from_private_key(setup_private_key())]
fn account_view_key(private_key: PrivateKey<CurrentNetwork>) -> ViewKey<CurrentNetwork> {
    black_box(ViewKey::try_from(black_box(&private_key)).unwrap())
}

#[library_benchmark]
#[bench::from_private_key(setup_private_key())]
fn account_address(private_key: PrivateKey<CurrentNetwork>) -> Address<CurrentNetwork> {
    black_box(Address::try_from(black_box(&private_key)).unwrap())
}

library_benchmark_group!(name = account, benchmarks = [account_private_key, account_view_key, account_address]);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = account
);
