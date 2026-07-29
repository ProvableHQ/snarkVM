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

// TODO (Antonio) include tests for the constructor and view scopes

// Checks that various instances of casts to external structs or from using external-structs members
// function as expected. This is tested in both the function (i.e. private, RegisterType) setting as
// well as the public (i.e. finalise, FinaliseType) one.
#[test]
fn test_cast_with_external_structs_only() -> Result<()> {
    let rng = &mut TestRng::default();

    let deployer_private_key = sample_genesis_private_key(rng);

    let program_a = Program::from_str(
        r"
        program program_a.aleo;

        struct struct_a:
            val as u8;

        struct struct_d:
            a as address;
            b as address;
            c as address;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let program_b = Program::from_str(
        r"
        import program_a.aleo;
        program program_b.aleo;

        struct struct_b:
            a as program_a.aleo/struct_a;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    // This is a distinct program from program_b.aleo, but both declare a struct with the same name
    // and member specification.
    let program_b2 = Program::from_str(
        r"
        import program_a.aleo;
        program program_b2.aleo;

        struct struct_b:
            a as program_a.aleo/struct_a;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let program_c = Program::from_str(
        r"
        import program_a.aleo;
        import program_b.aleo;
        import program_b2.aleo;
        program program_c.aleo;

        struct struct_c:
            a as program_a.aleo/struct_a;
            b as program_b.aleo/struct_b;

        struct struct_e:
            a as address;
            b as address;
            c as address;

        // Case 1 / function: The function casts external structs defined in program_a and program_b, then casts a struct_c.
        function run_1:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;

            output r3 as struct_c.private;

        // Case 2 / function: Similar to case 1 but with a cast involving a external-struct member read (r1.val) into an external-struct.
        function run_2:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1.val into r2 as program_a.aleo/struct_a;
            cast r2 into r3 as program_b.aleo/struct_b;
            cast r2 r3 into r4 as struct_c;

            output r4 as struct_c.private;

        // Case 3 / function: Here struct_c is cast from an external-struct member received as an argument and
        //                    an external-struct member cast inside the function itself.
        function run_3:
            input r0 as program_a.aleo/struct_a.private;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;

            output r2 as struct_c.private;

        // Case 4 / function: Similar to case 2, but the target of the cast is the local struct_c.
        // (`r1.a`) of the received external struct.
        function run_4:
            input r0 as program_a.aleo/struct_a.private;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r1.a r1 into r2 as struct_c;

            output r2 as struct_c.private;

        // Case 5 / function: Here the same external struct struct_a is cast into a program_b/struct_b and a program_b2/struct_b.
        function run_5:
            input r0 as program_a.aleo/struct_a.private;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 into r2 as program_b2.aleo/struct_b;
            cast r2.a r1 into r3 as struct_c;

            output r3 as struct_c.private;


        // Case 6 / function: Tests casts involving the special, private-side-only operands signer, caller and program_id.
        function run_6:
            cast self.signer self.caller program_a.aleo into r0 as program_a.aleo/struct_d;
            cast r0.a r0.b r0.c into r1 as struct_e;
            output r1 as struct_e.private;

        // Case 7 / function: Tests a three-level struct access involving external structs.
        function run_7:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a.val into r3 as u8;

            output r3 as u8.private;

        // Case 8 / function: Tests a casts to external struct one of whose operands involves a two-level struct access.
        function run_8:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a into r3 as program_b2.aleo/struct_b;

            output r3 as program_b2.aleo/struct_b.private;

        // Case 9 / finalize: Finalize-side mirror of case 1.
        function run_9:
            input r0 as u8.public;
            async run_9 r0 into r1;
            output r1 as program_c.aleo/run_9.future;

        finalize run_9:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;

        // Case 10 / finalize: Finalize-side mirror of case 2.
        function run_10:
            input r0 as u8.public;
            async run_10 r0 into r1;
            output r1 as program_c.aleo/run_10.future;

        finalize run_10:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1.val into r2 as program_a.aleo/struct_a;
            cast r2 into r3 as program_b.aleo/struct_b;
            cast r2 r3 into r4 as struct_c;

        // Case 11 / finalize: Finalize-side mirror of case 3.
        function run_11:
            input r0 as program_a.aleo/struct_a.public;
            async run_11 r0 into r1;
            output r1 as program_c.aleo/run_11.future;

        finalize run_11:
            input r0 as program_a.aleo/struct_a.public;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
        
        // Case 12 / finalize: Finalize-side mirror of case 4.
        function run_12:
            input r0 as program_a.aleo/struct_a.public;
            async run_12 r0 into r1;
            output r1 as program_c.aleo/run_12.future;

        finalize run_12:
            input r0 as program_a.aleo/struct_a.public;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r1.a r1 into r2 as struct_c;

        // Case 13 / finalize: Finalize-side mirror of case 5.
        function run_13:
            input r0 as program_a.aleo/struct_a.public;
            async run_13 r0 into r1;
            output r1 as program_c.aleo/run_13.future;

        finalize run_13:
            input r0 as program_a.aleo/struct_a.public;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 into r2 as program_b2.aleo/struct_b;
            cast r2.a r1 into r3 as struct_c;

        // Case 14 / finalize: Finalize-side mirror of case 6.
        function run_14:
            async run_14 into r0;
            output r0 as program_c.aleo/run_14.future;

        finalize run_14:
            cast program_a.aleo program_b.aleo program_c.aleo into r0 as program_a.aleo/struct_d;
            cast r0.a r0.b r0.c into r1 as struct_e;

        // Case 15 / finalize: Finalize-side mirror of case 7.
        function run_15:
            async run_15 into r0;
            output r0 as program_c.aleo/run_15.future;

        finalize run_15:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a.val into r3 as u8;

        // Case 16 / finalize: Finalize-side mirror of case 8.
        function run_16:
            async run_16 into r0;
            output r0 as program_c.aleo/run_16.future;

        finalize run_16:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a into r3 as program_b2.aleo/struct_b;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V18)?, rng);

    let transaction = vm.deploy(&deployer_private_key, &program_a, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b2, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_c, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let struct_a = "{ val: 5u8 }";

    // The cases are documented in the source of program_c.aleo above.

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_1"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_2"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_3"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_4"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_5"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let transaction = vm.execute(
        &deployer_private_key,
        ("program_c.aleo", "run_6"),
        Vec::<Value<_>>::new().iter(),
        None,
        0,
        None,
        rng,
    )?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&[]]), &[transaction], rng);

    let transaction = vm.execute(
        &deployer_private_key,
        ("program_c.aleo", "run_7"),
        Vec::<Value<_>>::new().iter(),
        None,
        0,
        None,
        rng,
    )?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&[]]), &[transaction], rng);

    let transaction = vm.execute(
        &deployer_private_key,
        ("program_c.aleo", "run_8"),
        Vec::<Value<_>>::new().iter(),
        None,
        0,
        None,
        rng,
    )?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&[]]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_9"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_10"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_11"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_12"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_13"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let transaction = vm.execute(
        &deployer_private_key,
        ("program_c.aleo", "run_14"),
        Vec::<Value<_>>::new().iter(),
        None,
        0,
        None,
        rng,
    )?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&[]]), &[transaction], rng);

    let transaction = vm.execute(
        &deployer_private_key,
        ("program_c.aleo", "run_15"),
        Vec::<Value<_>>::new().iter(),
        None,
        0,
        None,
        rng,
    )?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&[]]), &[transaction], rng);

    let transaction = vm.execute(
        &deployer_private_key,
        ("program_c.aleo", "run_16"),
        Vec::<Value<_>>::new().iter(),
        None,
        0,
        None,
        rng,
    )?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&[]]), &[transaction], rng);

    Ok(())
}

// Checks that various instances of casts to arrays involving external structs and their members, as
// well as structs having arrays as members, work as expected. This is tested in both the function
// (i.e. private, RegisterType) setting as well as the public (i.e. finalise, FinaliseType) one.
#[test]
fn test_cast_with_external_structs_in_arrays() -> Result<()> {
    let rng = &mut TestRng::default();

    let deployer_private_key = sample_genesis_private_key(rng);

    let program_a = Program::from_str(
        r"
        program program_a.aleo;

        struct struct_a:
            val as u8;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let program_b = Program::from_str(
        r"
        import program_a.aleo;
        program program_b.aleo;

        struct struct_b:
            a as program_a.aleo/struct_a;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    // This is a distinct program from program_b.aleo, but both declare a struct with the same name
    // and member specification.
    let program_b2 = Program::from_str(
        r"
        import program_a.aleo;
        program program_b2.aleo;

        struct struct_b:
            a as program_a.aleo/struct_a;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let program_c = Program::from_str(
        r"
        import program_a.aleo;
        import program_b.aleo;
        import program_b2.aleo;
        program program_c.aleo;

        struct struct_c:
            a as program_a.aleo/struct_a;
            b as program_b.aleo/struct_b;

        // struct containing an array whose elements are external structs.
        struct struct_arr:
            arr as [program_a.aleo/struct_a; 2u32];

        // Case 1 / function: Cast an [struct_c; 2] from a register holding a struct_c.
        function fun_1:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast r3 r3 into r4 as [struct_c; 2u32];

            output r4 as [struct_c; 2u32].private;

        // Case 2 / function: Cast a [program_b.aleo/struct_b; 2] one of whose elements involves a
        //                   read of an external struct from a local struct.
        function fun_2:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast r2 r3.b into r4 as [program_b.aleo/struct_b; 2u32];

            output r4 as [program_b.aleo/struct_b; 2u32].private;

        // Case 3 / function: Cast a [program_a.aleo/struct_a; 5] from elements involving various levels of nested reads.
        function fun_3:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast r1 r2.a r3.a r4.a r4.b.a into r5 as [program_a.aleo/struct_a; 5u32];

            output r5 as [program_a.aleo/struct_a; 5u32].private;

        // Case 4 / function: Cast a local struct containing an array of external structs.
        function fun_4:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 r1 into r2 as [program_a.aleo/struct_a; 2u32];
            cast r2 into r3 as struct_arr;

            output r3 as struct_arr.private;

        // Case 5 / finalize: Finalize-side mirror of case 1.
        function fun_5:
            input r0 as u8.public;
            async fun_5 r0 into r1;
            output r1 as program_c.aleo/fun_5.future;

        finalize fun_5:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast r3 r3 into r4 as [struct_c; 2u32];

        // Case 6 / finalize: Finalize-side mirror of case 2.
        function fun_6:
            input r0 as u8.public;
            async fun_6 r0 into r1;
            output r1 as program_c.aleo/fun_6.future;

        finalize fun_6:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast r2 r3.b into r4 as [program_b.aleo/struct_b; 2u32];

        // Case 7 / finalize: Finalize-side mirror of case 3.
        function fun_7:
            input r0 as u8.public;
            async fun_7 r0 into r1;
            output r1 as program_c.aleo/fun_7.future;

        finalize fun_7:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast r1 r2.a r3.a r4.a r4.b.a into r5 as [program_a.aleo/struct_a; 5u32];

        // Case 8 / finalize: Finalize-side mirror of case 4.
        function fun_8:
            input r0 as u8.public;
            async fun_8 r0 into r1;
            output r1 as program_c.aleo/fun_8.future;

        finalize fun_8:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 r1 into r2 as [program_a.aleo/struct_a; 2u32];
            cast r2 into r3 as struct_arr;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V18)?, rng);

    let transaction = vm.deploy(&deployer_private_key, &program_a, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b2, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_c, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    // The cases are documented in the source of program_c.aleo above.

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_1"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_2"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_3"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_4"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_5"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_6"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_7"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_8"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    Ok(())
}

// Checks that various instances of casts to records involving external structs and their members
// behave as expected. Tests only involve the function (i.e. non-finalize) scope, which records are
// restricted to.
#[test]
fn test_cast_with_external_structs_in_records() -> Result<()> {
    let rng = &mut TestRng::default();

    let deployer_private_key = sample_genesis_private_key(rng);

    let program_a = Program::from_str(
        r"
        program program_a.aleo;

        struct struct_a:
            val as u8;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let program_b = Program::from_str(
        r"
        import program_a.aleo;
        program program_b.aleo;

        struct struct_b:
            a as program_a.aleo/struct_a;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    // This is a distinct program from program_b.aleo, but both declare a struct with the same name
    // and member specification.
    let program_b2 = Program::from_str(
        r"
        import program_a.aleo;
        program program_b2.aleo;

        struct struct_b:
            a as program_a.aleo/struct_a;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let program_c = Program::from_str(
        r"
        import program_a.aleo;
        import program_b.aleo;
        import program_b2.aleo;
        program program_c.aleo;

        struct struct_c:
            a as program_a.aleo/struct_a;
            b as program_b.aleo/struct_b;

        record record_c:
            owner as address.private;
            a as struct_c.private;
            b as struct_c.private;

        record record_b:
            owner as address.private;
            a as program_b.aleo/struct_b.private;
            b as program_b.aleo/struct_b.private;

        record record_a:
            owner as address.private;
            a as program_a.aleo/struct_a.private;
            b as program_a.aleo/struct_a.private;
            c as program_a.aleo/struct_a.private;
            d as program_a.aleo/struct_a.private;
            e as program_a.aleo/struct_a.private;

        record record_mixed:
            owner as address.private;
            entry_1 as program_a.aleo/struct_a.private;
            entry_2 as program_b.aleo/struct_b.private;
            entry_3 as program_b2.aleo/struct_b.private;
            entry_4 as struct_c.private;

        // Case 1: Cast a record_c with two entries from a register containing a local struct_c.
        function fun_1:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast self.signer r3 r3 into r4 as record_c.record;

            output r4 as record_c.record;

        // Case 2: Cast a record_b where the first entry is a register holding an external struct_b
        //         and the second is a struct_b read from a register holding a struct_c (`r3.b`).
        function fun_2:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast self.signer r2 r3.b into r4 as record_b.record;

            output r4 as record_b.record;

        // Case 3: Cast a record_a whose five entries come from five different sources involving
        //         various reads.
        function fun_3:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast self.signer r1 r2.a r3.a r4.a r4.b.a into r5 as record_a.record;

            output r5 as record_a.record;

        // Case 4: Cast a record_mixed by reading each entry directly from a register of the matching type.
        function fun_4:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast self.signer r1 r2 r3 r4 into r5 as record_mixed.record;

            output r5 as record_mixed.record;

        // Case 5: Cast a record_mixed by combining direct registers and register member reads.
        function fun_5:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast self.signer r4.a r4.b r3 r4 into r5 as record_mixed.record;

            output r5 as record_mixed.record;

        constructor:
            assert.eq edition 0u16;
        ",
    )?;

    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V18)?, rng);

    let transaction = vm.deploy(&deployer_private_key, &program_a, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b2, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_c, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    // The cases are documented in the source of program_c.aleo above.

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_1"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_2"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_3"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_4"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_5"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    Ok(())
}
