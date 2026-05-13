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

// Tests that a function which receives an input of the given type, casts it to
// a Scalar and outputs the latter, deploys correctly.
fn test_program_with_cast_source_type(
    src_type: &str,
    vm: &VM<CurrentNetwork, LedgerType>,
    caller_private_key: &PrivateKey<CurrentNetwork>,
    rng: &mut TestRng,
) {
    let program = Program::<CurrentNetwork>::from_str(&format!(
        r"
        program test.aleo;

        function function_cast_public_public:
            input r0 as {src_type}.public;
            cast r0 into r1 as scalar;
            output r1 as scalar.public;

        function function_cast_public_private:
            input r0 as {src_type}.public;
            cast r0 into r1 as scalar;
            output r1 as scalar.private;

        function function_cast_private_private:
            input r0 as {src_type}.private;
            cast r0 into r1 as scalar;
            output r1 as scalar.private;

        function function_cast_private_public:
            input r0 as {src_type}.private;
            cast r0 into r1 as scalar;
            output r1 as scalar.public;

        function function_cast_in_closure:
            input r0 as {src_type}.public;
            call closure_cast r0 into r1;
            output r1 as scalar.public;

        closure closure_cast:
            input r0 as {src_type};
            cast r0 into r1 as scalar;
            output r1 as scalar;

        constructor:
            assert.eq true true;
    "
    ))
    .unwrap();

    // Build and apply the deployment transaction.
    let deployment = vm.deploy(caller_private_key, &program, None, 0, None, rng).unwrap();
    let block = sample_next_block(vm, caller_private_key, &[deployment], rng).unwrap();

    // The deployment must be accepted: not rejected and not aborted.
    assert_eq!(block.transactions().num_accepted(), 1, "expected the deployment to be accepted");
    assert_eq!(block.transactions().num_rejected(), 0, "expected no rejected transactions");
    assert!(block.aborted_transaction_ids().is_empty(), "expected no aborted transactions");
}

#[test]
fn test_program_with_output_scalar() {
    let rng = &mut TestRng::default();

    let caller_private_key = sample_genesis_private_key(rng);
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V15).unwrap(), rng);

    // Since |scalar field| ~ |base field|/4, we run each test enough times N =
    // 5 that a value which does not fit inside a Scalar will be sampled with a
    // high enough probability 1 - 1/4^N.
    for _ in 0..5 {
        // The three types which can be cast to a Scalar but do not always fit
        // inside one are Field, Group and Address.
        test_program_with_cast_source_type("field", &vm, &caller_private_key, rng);
        test_program_with_cast_source_type("group", &vm, &caller_private_key, rng);
        test_program_with_cast_source_type("address", &vm, &caller_private_key, rng);
    }
}
