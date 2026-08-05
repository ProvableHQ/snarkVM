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

// TODO (Antonio) document
#[test]
fn test_rand_chacha_times() {
    let rng = &mut TestRng::default();

    let caller_private_key = sample_genesis_private_key(rng);

    let program_1 = Program::from_str(
        r"
        program program_1.aleo;

        function foo:
            input r0 as u64.public;
            async foo into r1;
            output r0 as u64.public;
            output r1 as program_1.aleo/foo.future;

        finalize foo:
            rand.chacha into r0 as field;
            rand.chacha 1u8 into r1 as field;
            rand.chacha 1u16 into r2 as field;

            rand.chacha into r3 as address;
            rand.chacha 1u8 into r4 as address;
            rand.chacha 1u16 into r5 as address;

            rand.chacha into r6 as group;
            rand.chacha 1u8 into r7 as group;
            rand.chacha 1u16 into r8 as group;

            rand.chacha into r9 as u8;
            rand.chacha 1u8 into r10 as u8;
            rand.chacha 1u16 into r11 as u8;

            rand.chacha into r12 as u128;
            rand.chacha 1u8 into r13 as u128;
            rand.chacha 1u16 into r14 as u128;

            rand.chacha into r15 as scalar;
            rand.chacha 1u8 into r16 as scalar;
            rand.chacha 1u16 into r17 as scalar;

        constructor:
            assert.eq true true;
        ",
    ).unwrap();

    let program_2 = Program::from_str(
        r"
        program program_2.aleo;

        function foo:
            async foo into r0;
            output r0 as program_2.aleo/foo.future;

        finalize foo:
            cast 1field into r0 as field;
            cast 1field 1field into r1 as [field; 2u32];
            cast 1field 1field 1field into r2 as [field; 3u32];
            cast 1field 1field 1field 1field into r3 as [field; 4u32];
            cast 1field 1field 1field 1field 1field into r4 as [field; 5u32];
            rand.chacha r0 into r5 as field;
            rand.chacha r1 into r6 as field;
            rand.chacha r2 into r7 as field;
            rand.chacha r3 into r8 as field;
            rand.chacha r4 into r9 as field;

        constructor:
            assert.eq true true;
        ",
    ).unwrap();

    let program_3 = Program::from_str(
        r"
        program program_3.aleo;

        function foo:
            async foo into r0;
            output r0 as program_3.aleo/foo.future;

        finalize foo:
            cast 1field into r0 as [field; 1u32];
            cast 1field 1field into r1 as [field; 2u32];
            cast 1field 1field 1field into r2 as [field; 3u32];
            cast 1field 1field 1field 1field into r3 as [field; 4u32];
            cast 1field 1field 1field 1field 1field into r4 as [field; 5u32];
            hash.bhp1024 r0 into r5 as field;
            hash.bhp1024 r1 into r6 as field;
            hash.bhp1024 r2 into r7 as field;
            hash.bhp1024 r3 into r8 as field;
            hash.bhp1024 r4 into r9 as field; 

        constructor:
            assert.eq true true;
        ",
    ).unwrap();

    let program_4 = Program::from_str(
        r"
        program program_4.aleo;

        function foo:
            async foo into r0;
            output r0 as program_4.aleo/foo.future;

        finalize foo:
            cast 1field 1field 1field into r0 as [field; 3u32];
            hash.bhp1024.raw r0 into r1 as field;

            rand.chacha into r2 as field;

            cast 1field 1field 1field 1field 1field 1field into r3 as [field; 6u32];
            hash.bhp1024 r3 into r4 as field;

            rand.chacha into r5 as group;

        constructor:
            assert.eq true true;
        ",
    ).unwrap();

    println!("Sampling VM at consensus version V19");
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V19).unwrap(), rng);

    println!("Deploying programs");
    let deployment_1 = vm.deploy(&caller_private_key, &program_1, None, 0, None, rng).unwrap();
    add_and_test_with_costs(&vm, &caller_private_key, None, &[deployment_1], rng);

    let deployment_2 = vm.deploy(&caller_private_key, &program_2, None, 0, None, rng).unwrap();
    add_and_test_with_costs(&vm, &caller_private_key, None, &[deployment_2], rng);

    let deployment_3 = vm.deploy(&caller_private_key, &program_3, None, 0, None, rng).unwrap();
    add_and_test_with_costs(&vm, &caller_private_key, None, &[deployment_3], rng);

    let deployment_4 = vm.deploy(&caller_private_key, &program_4, None, 0, None, rng).unwrap();
    add_and_test_with_costs(&vm, &caller_private_key, None, &[deployment_4], rng);

    println!("Executing programs\n");

    // println!("************************* program 1 *************************");

    // let inputs = [Value::from_str("7u64").unwrap()];

    // let transaction_1 = vm
    //     .execute(&caller_private_key, ("program_1.aleo", "foo"), inputs.iter(), None, 0, None, rng)
    //     .unwrap();

    // println!("************************* program 2 *************************");

    // let transaction_2 = vm
    //     .execute(
    //         &caller_private_key,
    //         ("program_2.aleo", "foo"),
    //         Vec::<Value<CurrentNetwork>>::new().iter(),
    //         None,
    //         0,
    //         None,
    //         rng,
    //     )
    //     .unwrap();

    // println!("************************* program 3 *************************");

    // let transaction_3 = vm
    //     .execute(
    //         &caller_private_key,
    //         ("program_3.aleo", "foo"),
    //         Vec::<Value<CurrentNetwork>>::new().iter(),
    //         None,
    //         0,
    //         None,
    //         rng,
    //     )
    //     .unwrap();

    println!("************************* program 4 *************************");

    let transaction_4 = vm
        .execute(
            &caller_private_key,
            ("program_4.aleo", "foo"),
            Vec::<Value<CurrentNetwork>>::new().iter(),
            None,
            0,
            None,
            rng,
        )
        .unwrap();

    add_and_test_with_costs(
        &vm,
        &caller_private_key,
        None,
        &[transaction_4],
        rng,
    );


}
