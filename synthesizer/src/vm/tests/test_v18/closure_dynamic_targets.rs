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

// Deploys a program whose function points the target of a call.dynamic instruction to a closure or a
// function depending on the last bit of self.signer. The test checks several runs of test-
#[test]
fn test_conditional_dynamic_call_target_deployment() -> Result<()> {
    const N_EXPERIMENTS: usize = 10;

    let rng = &mut TestRng::default();

    // Create a program whose name contains the passed integer. This is so that different executions
    // have different deployment IDs and therefore different deployment-check outcomes.
    let parse_program = |i| {
        Program::<CurrentNetwork>::from_str(&format!(
            r"
        program program_{i}.aleo;

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
            call.dynamic 'program_{i}' 'aleo' r4 with r0 (as field.public) into r5 (as field.public);
            output r5 as field.public;

        constructor:
            assert.eq true true;
        ",
        ))
        .unwrap()
    };

    let process = Process::<CurrentNetwork>::load()?;

    // About half of the calls to process.deploy should succeed. About half of those should be
    // accepted by process.verify_deployment, but the outcome for each fixed deployment should be
    // the same across all calls to verify_deployment.
    for i in 0..N_EXPERIMENTS {
        let deployment_attempt = process.deploy::<CurrentAleo, _>(&parse_program(i), rng);

        match deployment_attempt {
            Ok(deployment) => {
                println!(" - Deployment computation {i} succeeded with ID {}", deployment.to_deployment_id()?);

                // TODO (Antonio) V19
                // Ensure that different runs of verify_deployment agree on the result - even if it is a rejection.
                let verification_successful =
                    process.verify_deployment::<CurrentAleo, _>(ConsensusVersion::V18, &deployment, rng).is_ok();
                for _ in 1..N_EXPERIMENTS {
                    assert_eq!(
                        verification_successful,
                        process.verify_deployment::<CurrentAleo, _>(ConsensusVersion::V18, &deployment, rng).is_ok(),
                        "Verifier disagreement during deployment verification",
                    );
                }
                println!("   All verifiers {}", if verification_successful { "accepted" } else { "rejected" });
            }
            Err(error) => {
                println!(" - Deployment computation {i} failed: {error}");
            }
        }
    }

    Ok(())
}
