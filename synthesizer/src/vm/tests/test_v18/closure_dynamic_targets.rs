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

use super::*;

// Deploys a program whose function selects the target of a call.dynamic at runtime, poinring to a
// closure or to a function depending on based on the last bit of self.signer
#[test]
fn test_dynamic_call_target_safe() -> Result<()> {
    let rng = &mut TestRng::default();

    let program = Program::<CurrentNetwork>::from_str(
        r"
program some_dcall.aleo;

closure some_closure:
    input r0 as field;
    add r0 r0 into r1;
    output r1 as field;

function some_function:
    input r0 as field.public;
    serialize.bits.raw self.signer (address) into r1 ([boolean; 253u32]);
    cast 'some_closure' into r2 as field;
    cast 'some_function' into r3 as field;
    ternary r1[0u32] r2 r3 into r4;
    call.dynamic 'some_dcall' 'aleo' r4 with r0 (as field.public) into r5 (as field.public);
    output r5 as field.public;

constructor:
    assert.eq true true;
",
    )?;

    let process = Process::<CurrentNetwork>::load()?;
    let deployment = process.deploy::<CurrentAleo, _>(&program, rng)?;

    // Verify the deployment 10 times, each of which has a 50% chance of selecting each of the
    // targets.
    for _ in 0..10 {
        process.verify_deployment::<CurrentAleo, _>(ConsensusVersion::V14, &deployment, rng)?;
    }

    Ok(())
}
