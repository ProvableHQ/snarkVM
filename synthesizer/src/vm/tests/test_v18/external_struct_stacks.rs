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
            cast r1 into r2 as struct_c;
            cast r2.b.a into r3 as u8;

            output r3 as u8.private;

        function run_14:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r1 into r2 as struct_c;
            cast r2.b into r3 as program_b2.aleo/struct_b2;

            output r3 as program_b2.aleo/struct_b2private;

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

    Ok(())
}
