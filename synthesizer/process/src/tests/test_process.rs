// Copyright (C) 2019-2023 Aleo Systems Inc.
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
    program::{Parser, ProgramID},
};
use synthesizer_program::Program;

use crate::Process;

type CurrentNetwork = MainnetV0;

#[test]
pub fn test_credits() {
    let process = Process::load().unwrap();
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
    let (_, program2) = Program::<CurrentNetwork>::parse(
        r"
program testing2.aleo;

function compute:
    input r0 as u32.private;
    input r1 as u32.public;
    add r0 r1 into r2;
    output r2 as u32.public;",
    )
    .unwrap();
    let (_, program3) = Program::<CurrentNetwork>::parse(
        r"
program testing3.aleo;

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
    process.add_program(&program2).unwrap();
    assert!(process.contains_program(program2.id()));
    process.add_program(&program3).unwrap();
    assert!(process.contains_program(program3.id()));
    // only 2 programs are cached, so program1 should be evicted
    assert!(!process.contains_program(program1.id()));
    // test we still have credits.aleo
    let credits_id = ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap();
    assert!(process.contains_program(&credits_id));
}
