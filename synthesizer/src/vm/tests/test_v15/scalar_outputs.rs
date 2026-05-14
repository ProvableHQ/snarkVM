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

// Tests that a function which receives a Field/Group/Address input, casts it to
// a Scalar and outputs the latter, deploys correctly.
#[test]
fn test_program_with_output_scalar() {
    let rng = &mut TestRng::default();

    let caller_private_key = sample_genesis_private_key(rng);
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);

    // The three types which can be cast to a Scalar but do not always fit
    // inside one are Field, Group and Address.
    let operand_types = ["field", "group", "address"];

    // Since |scalar field| ~ |base field|/4, we run each test enough times N =
    // 3 that a value which does not fit inside a Scalar will be sampled with a
    // high enough probability 1 - 1/4^N.
    for i in 0..3 {
        let mut program_str = format!(
            r"
            program test_{i}.aleo;

        "
        );

        for operand_type in operand_types.iter() {
            program_str += &format!(
                r"
            function fun_cast_pub_pub_{operand_type}:
                input r0 as {operand_type}.public;
                cast r0 into r1 as scalar;
                output r1 as scalar.public;

            function fun_cast_pub_pri_{operand_type}:
                input r0 as {operand_type}.public;
                cast r0 into r1 as scalar;
                output r1 as scalar.private;

            function fun_cast_pri_pri_{operand_type}:
                input r0 as {operand_type}.private;
                cast r0 into r1 as scalar;
                output r1 as scalar.private;

            function fun_cast_pri_pub_{operand_type}:
                input r0 as {operand_type}.private;
                cast r0 into r1 as scalar;
                output r1 as scalar.public;

            function fun_cast_in_closure_{operand_type}:
                input r0 as {operand_type}.public;
                call clo_cast_{operand_type} r0 into r1;
                output r1 as scalar.public;

            closure clo_cast_{operand_type}:
                input r0 as {operand_type};
                cast r0 into r1 as scalar;
                output r1 as scalar;

            "
            );
        }

        program_str += r"
        constructor:
            assert.eq true true;
        ";

        let program = Program::<CurrentNetwork>::from_str(&program_str).unwrap();

        // Build and apply the deployment transaction.
        let deployment = vm.deploy(&caller_private_key, &program, None, 0, None, rng).unwrap();
        let block = sample_next_block(&vm, &caller_private_key, &[deployment], rng).unwrap();

        // The deployment must be accepted: not rejected and not aborted.
        assert_eq!(block.transactions().num_accepted(), 1, "expected the deployment to be accepted");
        assert_eq!(block.transactions().num_rejected(), 0, "expected no rejected transactions");
        assert!(block.aborted_transaction_ids().is_empty(), "expected no aborted transactions");
    }
}
