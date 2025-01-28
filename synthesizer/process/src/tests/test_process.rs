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
    // Check whether the program is in memory.
    assert_eq!(process.num_stacks_in_memory(), 1);
    assert!(process.contains_program_in_cache(program1.id()));
}

#[test]
pub fn test_cache_evict() {
    // Prepare the credit program id.
    let credits_id = ProgramID::<CurrentNetwork>::from_str("credits.aleo").unwrap();
    // Create a program sampler which imports credits.aleo.
    let mid_program_template = |i| {
        format!(
            r"
import credits.aleo;

program mid{i}.aleo;

function mid{i}_transfer:
    input r0 as credits.aleo/credits.record;
    input r1 as address.private;
    input r2 as u64.private;

    call credits.aleo/transfer_private r0 r1 r2 into r3 r4;

    output r3 as credits.aleo/credits.record;
    output r4 as credits.aleo/credits.record;"
        )
    };
    // Create a program sampler which imports mid1.aleo and mid2.aleo.
    let root_program_template = |i| {
        format!(
            r"
import credits.aleo;
import mid1.aleo;
import mid2.aleo;

program root{i}.aleo;

function root_call:
    input r0 as credits.aleo/credits.record;
    input r1 as address.private;
    input r2 as u64.private;

    // Call mid1's transfer function
    call mid1.aleo/mid1_transfer r0 r1 r2 into r3 r4;

    // Call mid2's transfer function
    call mid2.aleo/mid2_transfer r4 r1 r2 into r5 r6;

    output r3 as credits.aleo/credits.record;
    output r4 as credits.aleo/credits.record;
    output r5 as credits.aleo/credits.record;
    output r6 as credits.aleo/credits.record;"
        )
    };

    // Sample two programs.
    let (_, mid1) = Program::<CurrentNetwork>::parse(&mid_program_template(1)).unwrap();
    let (_, mid2) = Program::<CurrentNetwork>::parse(&mid_program_template(2)).unwrap();
    // Initialize a new process.
    let mut process = crate::test_helpers::sample_process(&mid1);

    // Adding the root program should fail because an import is missing from memory and storage.
    let (_, root1) = Program::<CurrentNetwork>::parse(&root_program_template(1)).unwrap();
    assert!(process.add_program(&root1).is_err());
    assert!(!process.contains_program_in_cache(root1.id()));

    // Check whether mid1 is in memory.
    assert!(process.contains_program_in_cache(mid1.id()));
    assert_eq!(process.num_stacks_in_memory(), 1);
    // add mid2 to memory
    process.add_program(&mid2).unwrap();
    // Check whether mid2 is in memory.
    assert!(process.contains_program_in_cache(mid2.id()));
    assert_eq!(process.num_stacks_in_memory(), 2);

    for i in 3..=<CurrentNetwork as Network>::MAX_STACKS + 1 {
        let (_, program) = Program::<CurrentNetwork>::parse(&root_program_template(i)).unwrap();
        process.add_program(&program).unwrap();
        // The new program and imports should be cached.
        assert!(process.contains_program_in_cache(program.id()));
        assert!(process.contains_program_in_cache(mid1.id()));
        assert!(process.contains_program_in_cache(mid2.id()));
    }

    // Only MAX_STACKS programs are cached, so the oldest root should be evicted.
    let root3_id = ProgramID::<CurrentNetwork>::from_str("root3.aleo").unwrap();
    assert!(!process.contains_program_in_cache(&root3_id));
    // Test we still have credits.aleo in memory.
    assert!(process.contains_program_in_cache(&credits_id));
    // Test that an example root program's imports are correct.
    let root4_id = ProgramID::<CurrentNetwork>::from_str("root4.aleo").unwrap();
    let root4_stack = process.get_stack(root4_id).unwrap();
    let root4_import_ids = root4_stack.all_external_stacks().into_iter().map(|(id, _)| id).collect::<Vec<_>>();
    let expected_imports = vec![credits_id, credits_id, *mid1.id(), credits_id, *mid2.id()];
    assert_eq!(root4_import_ids, expected_imports);
}
