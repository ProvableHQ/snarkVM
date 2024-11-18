// Copyright 2024 Aleo Network Foundation
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

use std::str::FromStr;

use console::{
    network::MainnetV0,
    prelude::Network,
    program::{Parser, ProgramID},
};
use synthesizer_program::Program;

use crate::Process;

type CurrentNetwork = MainnetV0;

#[test]
pub fn test_credits() {
    let process = Process::load_testing_only().unwrap();
    let credits_id = ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap();
    assert!(process.contains_program(&credits_id));
}

#[test]
pub fn test_cache() {
    let (_, program1) = Program::<CurrentNetwork>::parse(
        r"
program testing1.aleo;

function compute:
    input r0 as u32.private;
    input r1 as u32.public;
    add r0 r1 into r2;
    output r2 as u32.public;",
    )
    .unwrap();
    // Initialize a new process.
    let process = crate::test_helpers::sample_process(&program1);
    assert!(process.contains_program(program1.id()));
}

#[test]
pub fn test_cache_evict() {
    let (_, program1) = Program::<CurrentNetwork>::parse(
        r"
program testing1.aleo;

function compute:
    input r0 as u32.private;
    input r1 as u32.public;
    add r0 r1 into r2;
    output r2 as u32.public;",
    )
    .unwrap();
    // Initialize a new process.
    let mut process = crate::test_helpers::sample_process(&program1);
    assert!(process.contains_program(program1.id()));

    for i in 2..=<CurrentNetwork as Network>::MAX_STACKS + 1 {
        let source = format!(
            r"
program testing{i}.aleo;

function compute:
    input r0 as u32.private;
    input r1 as u32.public;
    add r0 r1 into r2;
    output r2 as u32.public;"
        );
        // Program1 should still be cached
        assert!(process.contains_program(program1.id()));
        let (_, program) = Program::<CurrentNetwork>::parse(&source).unwrap();
        process.add_program(&program).unwrap();
        assert!(process.contains_program(program.id()));
    }

    // only MAX_STACKS programs are cached, so program1 should be evicted
    assert!(!process.contains_program(program1.id()));
    // test we still have credits.aleo
    let credits_id = ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap();
    assert!(process.contains_program(&credits_id));
}
