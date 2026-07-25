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

// TODO (Antonio) review documentation

// Deploys a program whose function selects the target of a `call.dynamic` at runtime, based on the
// last bit of `self.signer`, choosing between the field representation of a closure (`some_closure`)
// and a function (`some_function`). The deployment is then verified repeatedly to ensure the
// verification decision is stable across freshly sampled randomness.
#[test]
fn test_dynamic_call_target_safe() -> Result<()> {
    let rng = &mut TestRng::default();

    // The program declares a closure `some_closure` and a function `some_function`. `some_function`
    // reads the last bit of `self.signer`, then uses it to drive a `ternary` that selects between the
    // field representation of `some_closure` and `some_function`. The selected field is used as the
    // function-name operand of a `call.dynamic` back into the same program.
    // Note: `serialize.bits.raw` on an `address` yields `field`-many bits (253 for this network), so
    // the last bit is at index 252. `ternary` is applied to the field representations (identifiers
    // cannot be used with `ternary` directly), obtained via `cast <identifier> ... as field`.
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

    // Initialize a fresh process and construct the deployment once.
    let process = Process::<CurrentNetwork>::load()?;
    let deployment = process.deploy::<CurrentAleo, _>(&program, rng)?;

    // Verify the deployment 10 times. Each call to `verify_deployment` samples fresh randomness from
    // `rng` (which advances between iterations), so the certificates are re-checked against new
    // randomness on every iteration.
    for _ in 0..10 {
        process.verify_deployment::<CurrentAleo, _>(ConsensusVersion::V14, &deployment, rng)?;
    }

    Ok(())
}
