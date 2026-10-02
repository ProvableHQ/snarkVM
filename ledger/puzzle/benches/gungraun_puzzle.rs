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

//! Callgrind measurements of `Puzzle::prove` and `Puzzle::check_solutions`.
//!
//! Inputs come from a fixed seed. Rayon is pinned to one worker so the instruction count does not follow the pool size.

use snarkvm_console::{
    account::*,
    network::{MainnetV0, Network},
};
use snarkvm_ledger_puzzle::{Puzzle, PuzzleSolutions};
use snarkvm_ledger_puzzle_epoch::MerklePuzzle;

use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use rand::RngExt;
use std::hint::black_box;

const SEED: u64 = 0x0A22_1E00;

type BlockHash = <MainnetV0 as Network>::BlockHash;

fn sample_address_and_counter(rng: &mut TestRng) -> (Address<MainnetV0>, u64) {
    let private_key = PrivateKey::new(rng).unwrap();
    let address = Address::try_from(private_key).unwrap();
    let counter = rng.random::<u64>();
    (address, counter)
}

struct ProveInput {
    puzzle: Puzzle<MainnetV0>,
    epoch_hash: BlockHash,
    address: Address<MainnetV0>,
    counter: u64,
}

fn setup_prove() -> ProveInput {
    let rng = &mut TestRng::from_seed(SEED);
    let puzzle = Puzzle::<MainnetV0>::new::<MerklePuzzle<MainnetV0>>();
    let epoch_hash = rng.random();
    let (address, counter) = sample_address_and_counter(rng);
    ProveInput { puzzle, epoch_hash, address, counter }
}

fn setup_solutions(batch_size: usize) -> (Puzzle<MainnetV0>, PuzzleSolutions<MainnetV0>, BlockHash) {
    let rng = &mut TestRng::from_seed(SEED);
    let puzzle = Puzzle::<MainnetV0>::new::<MerklePuzzle<MainnetV0>>();
    let epoch_hash = rng.random();
    let solutions = (0..batch_size)
        .map(|_| {
            let (address, counter) = sample_address_and_counter(rng);
            puzzle.prove(epoch_hash, address, counter, None).unwrap()
        })
        .collect::<Vec<_>>();
    (puzzle, PuzzleSolutions::new(solutions).unwrap(), epoch_hash)
}

#[library_benchmark]
#[bench::once(setup_prove())]
fn puzzle_prove(input: ProveInput) {
    black_box(
        input
            .puzzle
            .prove(black_box(input.epoch_hash), black_box(input.address), black_box(input.counter), None)
            .unwrap(),
    );
}

#[library_benchmark]
#[bench::batch_1(setup_solutions(1))]
#[bench::batch_2(setup_solutions(2))]
#[bench::batch_max(setup_solutions(<MainnetV0 as Network>::MAX_SOLUTIONS))]
fn puzzle_check_solutions(input: (Puzzle<MainnetV0>, PuzzleSolutions<MainnetV0>, BlockHash)) {
    let (puzzle, solutions, epoch_hash) = black_box(input);
    puzzle.check_solutions(&solutions, epoch_hash, 0u64).unwrap();
}

library_benchmark_group!(name = puzzle, benchmarks = [puzzle_prove, puzzle_check_solutions]);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false).env("RAYON_NUM_THREADS", "1"),
    library_benchmark_groups = puzzle
);
