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
fn test_cast_with_external_structs() -> Result<()> {
    let rng = &mut TestRng::default();

    let deployer_private_key = sample_genesis_private_key(rng);

    // `program_a.aleo` defines `struct_a`, holding a single `u8`, and `struct_d`,
    // holding three `address` members. The latter is used to exercise casting the
    // `address`-typed program-reference operands (program ID, signer, caller) into
    // an externally-defined struct.
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

    // `program_b.aleo` imports `program_a.aleo` and defines `struct_b`, whose
    // only member is the external struct `program_a.aleo/struct_a`.
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

    // `program_b2.aleo` is a distinct program that imports `program_a.aleo` and
    // declares a `struct_b` with the same name and structure as
    // `program_b.aleo`'s. It exercises cross-program struct equivalence (same
    // name and structure, different defining program).
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

    // `program_c.aleo` imports both programs and defines `struct_c`, whose only
    // member is the external struct `program_b.aleo/struct_b`. Its function
    // casts a `struct_a` into a `struct_b`, and then that `struct_b` into the
    // local `struct_c`.
    let program_c = Program::from_str(
        r"
        import program_a.aleo;
        import program_b.aleo;
        import program_b2.aleo;
        program program_c.aleo;

        struct struct_c:
            a as program_a.aleo/struct_a;
            b as program_b.aleo/struct_b;

        // A local struct mirroring `program_a.aleo/struct_d`, used to receive the
        // `address` members read back out of the external struct.
        struct struct_e:
            a as address;
            b as address;
            c as address;

        // Case 1 / private: The function casts both external structs itself.
        function run_1:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;

            output r3 as struct_c.private;

        // Case 2 / private: The function casts both external structs itself,
        // feeding a member-access operand (`r1.val`) back into a cast.
        function run_2:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1.val into r2 as program_a.aleo/struct_a;
            cast r2 into r3 as program_b.aleo/struct_b;
            cast r2 r3 into r4 as struct_c;

            output r4 as struct_c.private;

        // Case 3 / private: The function casts one external struct itself and
        // receives the other.
        function run_3:
            input r0 as program_a.aleo/struct_a.private;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;

            output r2 as struct_c.private;

        // Case 4 / private: Same as case 3, but using a member-access operand
        // (`r1.a`) of the received external struct.
        function run_4:
            input r0 as program_a.aleo/struct_a.private;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r1.a r1 into r2 as struct_c;

            output r2 as struct_c.private;

        // TODO (Antonio) document this and other cases
        // Case 5 / private:
        function run_5:
            input r0 as program_a.aleo/struct_a.private;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 into r2 as program_b2.aleo/struct_b;
            cast r2.a r1 into r3 as struct_c;

            output r3 as struct_c.private;

        // Case 6 / finalize: Mirrors case 1, but the finalize casts both
        // external structs itself.
        function run_6:
            input r0 as u8.public;
            async run_6 r0 into r1;
            output r1 as program_c.aleo/run_6.future;

        finalize run_6:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;

        // Case 7 / finalize: Mirrors case 2, feeding a member-access operand
        // (`r1.val`) back into a cast in the finalize scope.
        function run_7:
            input r0 as u8.public;
            async run_7 r0 into r1;
            output r1 as program_c.aleo/run_7.future;

        finalize run_7:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1.val into r2 as program_a.aleo/struct_a;
            cast r2 into r3 as program_b.aleo/struct_b;
            cast r2 r3 into r4 as struct_c;

        // Case 8 / finalize: Mirrors case 3, the finalize casts one external
        // struct itself and receives the other.
        function run_8:
            input r0 as program_a.aleo/struct_a.public;
            async run_8 r0 into r1;
            output r1 as program_c.aleo/run_8.future;

        finalize run_8:
            input r0 as program_a.aleo/struct_a.public;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;

        // Case 9 / finalize: Mirrors case 4, using a member-access operand
        // (`r1.a`) of the received external struct in the finalize scope.
        function run_9:
            input r0 as program_a.aleo/struct_a.public;
            async run_9 r0 into r1;
            output r1 as program_c.aleo/run_9.future;

        finalize run_9:
            input r0 as program_a.aleo/struct_a.public;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r1.a r1 into r2 as struct_c;

        // Case 10 / finalize: Mirrors case 5, exercising cross-program struct
        // equivalence via `program_b2.aleo/struct_b` in the finalize scope.
        function run_10:
            input r0 as program_a.aleo/struct_a.public;
            async run_10 r0 into r1;
            output r1 as program_c.aleo/run_10.future;

        finalize run_10:
            input r0 as program_a.aleo/struct_a.public;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 into r2 as program_b2.aleo/struct_b;
            cast r2.a r1 into r3 as struct_c;

        // Case 11 / private: `self.signer`, `self.caller`, and the program ID
        // `program_a.aleo` all resolve to the `address` primitive. This casts them
        // into the external struct `program_a.aleo/struct_d` (whose members are
        // addresses), exercising the `ProgramID | Signer | Caller` arm of
        // `matches_struct` against an externally-defined struct. It then reads the
        // members back out and casts them into the local `struct_e`.
        function run_11:
            cast self.signer self.caller program_a.aleo into r0 as program_a.aleo/struct_d;
            cast r0.a r0.b r0.c into r1 as struct_e;
            output r1 as struct_e.private;

        // Case 12 / finalize: Mirrors case 11 in the finalize scope. Only the
        // program ID operand resolves to an `address` there (`self.signer` and
        // `self.caller` are rejected in a finalize scope), so this casts the program
        // IDs `program_a.aleo`, `program_b.aleo`, and `program_c.aleo` into the
        // external struct `program_a.aleo/struct_d`, then reads the members back out
        // into the local `struct_e`.
        function run_12:
            async run_12 into r0;
            output r0 as program_c.aleo/run_12.future;

        finalize run_12:
            cast program_a.aleo program_b.aleo program_c.aleo into r0 as program_a.aleo/struct_d;
            cast r0.a r0.b r0.c into r1 as struct_e;

        function run_13:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a.val into r3 as u8;

            output r3 as u8.private;

        function run_14:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a into r3 as program_b2.aleo/struct_b;

            output r3 as program_b2.aleo/struct_b.private;

        // Case 13 / finalize: Mirrors run_13, performing the nested cast chain and
        // member-access read (`r2.b.a.val`) entirely in the finalize scope.
        function run_15:
            async run_15 into r0;
            output r0 as program_c.aleo/run_15.future;

        finalize run_15:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a.val into r3 as u8;

        // Case 14 / finalize: Mirrors run_14, casting a member-access operand
        // (`r2.b.a`) back into an external struct entirely in the finalize scope.
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

    // Initialize the VM at V18.
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V18)?, rng);

    // Deploy the programs in dependency order.
    let transaction = vm.deploy(&deployer_private_key, &program_a, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b2, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_c, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    // Reusable literal for the external struct input.
    let struct_a = "{ val: 5u8 }";

    // Case 1: The function casts both external structs itself.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_1"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 2: The function casts both external structs itself, via a member-access operand.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_2"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 3: The function casts one external struct itself and receives the other.
    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_3"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 4: Same as case 3, but using a member-access operand of the received struct.
    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_4"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 5: Cross-program struct equivalence via `program_b2.aleo/struct_b`.
    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_5"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 6: Mirrors case 1 in the finalize scope.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_6"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 7: Mirrors case 2 in the finalize scope.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_7"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 8: Mirrors case 3 in the finalize scope.
    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_8"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 9: Mirrors case 4 in the finalize scope.
    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_9"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 10: Mirrors case 5 in the finalize scope.
    let inputs = [Value::from_str(struct_a)?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "run_10"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 11: Cast the `address`-typed program-reference operands (signer, caller,
    // program ID) into an external struct, in the private scope.
    let transaction = vm.execute(
        &deployer_private_key,
        ("program_c.aleo", "run_11"),
        Vec::<Value<_>>::new().iter(),
        None,
        0,
        None,
        rng,
    )?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&[]]), &[transaction], rng);

    // Case 12: Mirrors case 11 in the finalize scope, casting program IDs into an
    // external struct.
    let transaction = vm.execute(
        &deployer_private_key,
        ("program_c.aleo", "run_12"),
        Vec::<Value<_>>::new().iter(),
        None,
        0,
        None,
        rng,
    )?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&[]]), &[transaction], rng);

    // Case 13: Mirrors run_13's nested cast chain and member-access read in the
    // finalize scope.
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

    // Case 14: Mirrors run_14's member-access cast back into an external struct in
    // the finalize scope.
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

    // TODO (Antonio) reorder cases
    // TODO (Antonio) add negative tests eg that one cant pass stuct_b from program b2 to a

    Ok(())
}

// Exercises casting arrays whose elements are externally-defined structs, drawing
// the elements from a mix of register types and (nested) member-access operands.
// Each scenario is tested in both a private function scope and a public finalize
// scope.
#[test]
fn test_cast_with_external_structs_in_arrays() -> Result<()> {
    let rng = &mut TestRng::default();

    let deployer_private_key = sample_genesis_private_key(rng);

    // `program_a.aleo` defines `struct_a`, holding a single `u8`.
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

    // `program_b.aleo` imports `program_a.aleo` and defines `struct_b`, whose only
    // member is the external struct `program_a.aleo/struct_a`.
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

    // `program_b2.aleo` is a distinct program declaring a `struct_b` with the same
    // name and structure as `program_b.aleo`'s. It exercises cross-program struct
    // equivalence (same name and structure, different defining program).
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

    // `program_c.aleo` imports the other programs and defines `struct_c`, holding a
    // `program_a.aleo/struct_a` and a `program_b.aleo/struct_b`. Its functions cast
    // arrays of external structs, drawing elements from registers and member-access
    // operands.
    let program_c = Program::from_str(
        r"
        import program_a.aleo;
        import program_b.aleo;
        import program_b2.aleo;
        program program_c.aleo;

        struct struct_c:
            a as program_a.aleo/struct_a;
            b as program_b.aleo/struct_b;

        // `struct_arr` wraps an array of the external struct `program_a.aleo/struct_a`.
        struct struct_arr:
            arr as [program_a.aleo/struct_a; 2u32];

        // Case 1 / private: Cast an array `[struct_c; 2]` from a register holding a
        // `struct_c`.
        function fun_1:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast r3 r3 into r4 as [struct_c; 2u32];

            output r4 as [struct_c; 2u32].private;

        // Case 2 / private: Cast an array `[program_b.aleo/struct_b; 2]` where the
        // first element is a register holding a `struct_b` and the second is the
        // `struct_b` member read out of a register holding a `struct_c` (`r3.b`).
        function fun_2:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast r2 r3.b into r4 as [program_b.aleo/struct_b; 2u32];

            output r4 as [program_b.aleo/struct_b; 2u32].private;

        // Case 3 / private: Cast an array `[program_a.aleo/struct_a; 5]` whose
        // elements come from five different sources: a register holding a
        // `struct_a`, the `struct_a` members of a `program_b.aleo/struct_b` and a
        // `program_b2.aleo/struct_b`, and both the direct (`r4.a`) and nested
        // (`r4.b.a`) `struct_a` members of a `struct_c`.
        function fun_3:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast r1 r2.a r3.a r4.a r4.b.a into r5 as [program_a.aleo/struct_a; 5u32];

            output r5 as [program_a.aleo/struct_a; 5u32].private;

        // Case 4 / finalize: Mirrors case 1 in the finalize scope.
        function fun_4:
            input r0 as u8.public;
            async fun_4 r0 into r1;
            output r1 as program_c.aleo/fun_4.future;

        finalize fun_4:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast r3 r3 into r4 as [struct_c; 2u32];

        // Case 5 / finalize: Mirrors case 2 in the finalize scope.
        function fun_5:
            input r0 as u8.public;
            async fun_5 r0 into r1;
            output r1 as program_c.aleo/fun_5.future;

        finalize fun_5:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast r2 r3.b into r4 as [program_b.aleo/struct_b; 2u32];

        // Case 6 / finalize: Mirrors case 3 in the finalize scope.
        function fun_6:
            input r0 as u8.public;
            async fun_6 r0 into r1;
            output r1 as program_c.aleo/fun_6.future;

        finalize fun_6:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast r1 r2.a r3.a r4.a r4.b.a into r5 as [program_a.aleo/struct_a; 5u32];

        // Case 7 / private: Cast an array of `program_a.aleo/struct_a` into the
        // `struct_arr` struct, whose sole member is that array type.
        function fun_7:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 r1 into r2 as [program_a.aleo/struct_a; 2u32];
            cast r2 into r3 as struct_arr;

            output r3 as struct_arr.private;

        // Case 8 / finalize: Mirrors case 7 in the finalize scope.
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

    // Initialize the VM at V18.
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V18)?, rng);

    // Deploy the programs in dependency order.
    let transaction = vm.deploy(&deployer_private_key, &program_a, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b2, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_c, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    // Case 1: Cast an array `[struct_c; 2]` in the private scope.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_1"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 2: Cast an array `[struct_b; 2]` from mixed register and member-access
    // sources in the private scope.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_2"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 3: Cast an array `[struct_a; 5]` from five mixed sources in the private
    // scope.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_3"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 4: Mirrors case 1 in the finalize scope.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_4"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 5: Mirrors case 2 in the finalize scope.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_5"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 6: Mirrors case 3 in the finalize scope.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_6"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 7: Cast an array of external structs into `struct_arr` in the private
    // scope.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_7"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 8: Mirrors case 7 in the finalize scope.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_8"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    Ok(())
}

// Exercises casting records whose entries are externally-defined structs, drawing
// the entries from a mix of register types and (nested) member-access operands.
// This mirrors `test_cast_with_external_structs_in_arrays`, but casts to records
// instead of arrays (an array of N elements becomes a record of N entries). Since
// records cannot be cast in a finalize scope, only private scopes are exercised.
#[test]
fn test_cast_with_external_structs_in_records() -> Result<()> {
    let rng = &mut TestRng::default();

    let deployer_private_key = sample_genesis_private_key(rng);

    // `program_a.aleo` defines `struct_a`, holding a single `u8`.
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

    // `program_b.aleo` imports `program_a.aleo` and defines `struct_b`, whose only
    // member is the external struct `program_a.aleo/struct_a`.
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

    // `program_b2.aleo` is a distinct program declaring a `struct_b` with the same
    // name and structure as `program_b.aleo`'s. It exercises cross-program struct
    // equivalence (same name and structure, different defining program).
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

    // `program_c.aleo` imports the other programs and defines `struct_c`, holding a
    // `program_a.aleo/struct_a` and a `program_b.aleo/struct_b`. Its records hold
    // external structs as entries, and its functions cast into those records,
    // drawing entries from registers and member-access operands.
    let program_c = Program::from_str(
        r"
        import program_a.aleo;
        import program_b.aleo;
        import program_b2.aleo;
        program program_c.aleo;

        struct struct_c:
            a as program_a.aleo/struct_a;
            b as program_b.aleo/struct_b;

        // A record with two `struct_c` entries.
        record record_c:
            owner as address.private;
            a as struct_c.private;
            b as struct_c.private;

        // A record with two `program_b.aleo/struct_b` entries.
        record record_b:
            owner as address.private;
            a as program_b.aleo/struct_b.private;
            b as program_b.aleo/struct_b.private;

        // A record with five `program_a.aleo/struct_a` entries.
        record record_a:
            owner as address.private;
            a as program_a.aleo/struct_a.private;
            b as program_a.aleo/struct_a.private;
            c as program_a.aleo/struct_a.private;
            d as program_a.aleo/struct_a.private;
            e as program_a.aleo/struct_a.private;

        // A record with entries of mixed external-struct types.
        record record_mixed:
            owner as address.private;
            entry_1 as program_a.aleo/struct_a.private;
            entry_2 as program_b.aleo/struct_b.private;
            entry_3 as program_b2.aleo/struct_b.private;
            entry_4 as struct_c.private;

        // Case 1 / private: Cast a `record_c` with two entries from a register
        // holding a `struct_c`.
        function fun_1:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast self.signer r3 r3 into r4 as record_c.record;

            output r4 as record_c.record;

        // Case 2 / private: Cast a `record_b` where the first entry is a register
        // holding a `struct_b` and the second is the `struct_b` member read out of a
        // register holding a `struct_c` (`r3.b`).
        function fun_2:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast self.signer r2 r3.b into r4 as record_b.record;

            output r4 as record_b.record;

        // Case 3 / private: Cast a `record_a` whose five entries come from five
        // different sources: a register holding a `struct_a`, the `struct_a` members
        // of a `program_b.aleo/struct_b` and a `program_b2.aleo/struct_b`, and both
        // the direct (`r4.a`) and nested (`r4.b.a`) `struct_a` members of a
        // `struct_c`.
        function fun_3:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast self.signer r1 r2.a r3.a r4.a r4.b.a into r5 as record_a.record;

            output r5 as record_a.record;

        // Case 4 / private: Cast a `record_mixed` reading each entry directly from a
        // register of the matching type.
        function fun_4:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast self.signer r1 r2 r3 r4 into r5 as record_mixed.record;

            output r5 as record_mixed.record;

        // Case 5 / private: Cast a `record_mixed` mixing member-access operands
        // (`r4.a`, `r4.b`) with direct register reads (`r3`, `r4`).
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

    // Initialize the VM at V18.
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V18)?, rng);

    // Deploy the programs in dependency order.
    let transaction = vm.deploy(&deployer_private_key, &program_a, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_b2, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    let transaction = vm.deploy(&deployer_private_key, &program_c, None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);

    // Case 1: Cast a `record_c` with two entries in the private scope.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_1"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 2: Cast a `record_b` from mixed register and member-access sources.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_2"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 3: Cast a `record_a` from five mixed sources.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_3"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 4: Cast a mixed-entry `record_mixed`, reading each entry directly from a
    // register.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_4"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    // Case 5: Cast a mixed-entry `record_mixed`, mixing member-access operands with
    // direct register reads.
    let inputs = [Value::from_str("5u8")?];
    let transaction =
        vm.execute(&deployer_private_key, ("program_c.aleo", "fun_5"), inputs.iter(), None, 0, None, rng)?;
    add_and_test_with_costs(&vm, &deployer_private_key, Some(&[&inputs]), &[transaction], rng);

    Ok(())
}
