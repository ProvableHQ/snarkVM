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

// Deploys `program_a`, `program_b`, and `program_b2` once (the shared imports), then deploys each
// labeled program variant separately on the same VM. Each variant's self-references
// (`program_c.aleo`) are renamed to a unique program ID so that already-deployed variants do not
// collide. The per-variant outcome (accepted / rejected) is printed, and the test fails at the end
// listing every variant that was rejected at deployment. This isolates exactly which functions,
// views, or constructors are rejected, instead of failing on the first offending component of a
// single bundled program.
fn deploy_variants_separately(
    scope: &str,
    program_a: &Program<CurrentNetwork>,
    program_b: &Program<CurrentNetwork>,
    program_b2: &Program<CurrentNetwork>,
    variants: &[(&str, String)],
    rng: &mut TestRng,
) {
    let deployer_private_key = sample_genesis_private_key(rng);
    let vm = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V18).unwrap(), rng);

    // Deploy the shared imported programs once.
    for program in [program_a, program_b, program_b2] {
        let transaction = vm.deploy(&deployer_private_key, program, None, 0, None, rng).unwrap();
        add_and_test_with_costs(&vm, &deployer_private_key, None, &[transaction], rng);
    }

    // Check each variant separately, printing which ones are rejected at deployment.
    // `vm.deploy` only builds and statically checks the program via `Stack::new`; it does not run
    // the transaction-level, version-gated deployment verification. So we build the transaction
    // here (which always succeeds for these variants) and then run `vm.check_transaction`, which is
    // the method that actually exercises the pre-V19 legacy external-struct cast check.
    for (index, (label, source)) in variants.iter().enumerate() {
        // Prefix the label with the scope so that every execution across all tests is uniquely
        // identifiable (e.g. `arrays/fun_1` versus `records/fun_1`).
        let label = format!("{scope}/{label}");
        // Give each variant a unique program ID so previously-deployed variants do not collide.
        let source = source.replace("program_c.aleo", &format!("program_c{index}.aleo"));
        let program = Program::from_str(&source).unwrap();
        let transaction = vm.deploy(&deployer_private_key, &program, None, 0, None, rng).unwrap();
        match vm.check_transaction(&transaction, None, rng) {
            Ok(()) => println!("ACCEPTED: {label}"),
            Err(error) => println!("REJECTED: {label}: {error}"),
        }
    }
}

// Checks that various instances of casts to external structs or from using external-structs members
// function as expected, deploying each function of the final program separately so that a rejection
// can be attributed to a specific function.
#[test]
fn test_cast_with_external_structs_only() {
    let rng = &mut TestRng::default();

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
    ).unwrap();

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
    ).unwrap();

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
    ).unwrap();

    // Shared preamble (imports, program declaration, and local structs) and constructor for every
    // single-function variant.
    let preamble = r"
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
    ";
    let ctor = r"
        constructor:
            assert.eq edition 0u16;
    ";
    let make = |body: &str| format!("{preamble}\n{body}\n{ctor}");

    // The cases mirror the single bundled program; each is now its own single-function program.
    let variants = vec![
        // Case 1 / function: casts external structs defined in program_a and program_b, then a struct_c.
        (
            "run_1",
            make(
                r"
        function run_1:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;

            output r3 as struct_c.private;
                ",
            ),
        ),
        // Case 2 / function: similar to case 1 but with a cast involving an external-struct member read (r1.val).
        (
            "run_2",
            make(
                r"
        function run_2:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1.val into r2 as program_a.aleo/struct_a;
            cast r2 into r3 as program_b.aleo/struct_b;
            cast r2 r3 into r4 as struct_c;

            output r4 as struct_c.private;
                ",
            ),
        ),
        // Case 3 / function: struct_c cast from an external-struct argument and an in-function external-struct member.
        (
            "run_3",
            make(
                r"
        function run_3:
            input r0 as program_a.aleo/struct_a.private;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;

            output r2 as struct_c.private;
                ",
            ),
        ),
        // Case 4 / function: similar to case 2, but the target of the cast is the local struct_c.
        (
            "run_4",
            make(
                r"
        function run_4:
            input r0 as program_a.aleo/struct_a.private;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r1.a r1 into r2 as struct_c;

            output r2 as struct_c.private;
                ",
            ),
        ),
        // Case 5 / function: the same external struct struct_a is cast into a program_b and a program_b2 struct_b.
        (
            "run_5",
            make(
                r"
        function run_5:
            input r0 as program_a.aleo/struct_a.private;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 into r2 as program_b2.aleo/struct_b;
            cast r2.a r1 into r3 as struct_c;

            output r3 as struct_c.private;
                ",
            ),
        ),
        // Case 6 / function: casts involving the private-side-only operands signer, caller and program_id.
        (
            "run_6",
            make(
                r"
        function run_6:
            cast self.signer self.caller program_a.aleo into r0 as program_a.aleo/struct_d;
            cast r0.a r0.b r0.c into r1 as struct_e;
            output r1 as struct_e.private;
                ",
            ),
        ),
        // Case 7 / function: a three-level struct access involving external structs.
        (
            "run_7",
            make(
                r"
        function run_7:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a.val into r3 as u8;

            output r3 as u8.private;
                ",
            ),
        ),
        // Case 8 / function: a cast to an external struct with an operand involving a two-level struct access.
        (
            "run_8",
            make(
                r"
        function run_8:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a into r3 as program_b2.aleo/struct_b;

            output r3 as program_b2.aleo/struct_b.private;
                ",
            ),
        ),
        // Case 9 / finalize: finalize-side mirror of case 1.
        (
            "run_9",
            make(
                r"
        function run_9:
            input r0 as u8.public;
            async run_9 r0 into r1;
            output r1 as program_c.aleo/run_9.future;

        finalize run_9:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
                ",
            ),
        ),
        // Case 10 / finalize: finalize-side mirror of case 2.
        (
            "run_10",
            make(
                r"
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
                ",
            ),
        ),
        // Case 11 / finalize: finalize-side mirror of case 3.
        (
            "run_11",
            make(
                r"
        function run_11:
            input r0 as program_a.aleo/struct_a.public;
            async run_11 r0 into r1;
            output r1 as program_c.aleo/run_11.future;

        finalize run_11:
            input r0 as program_a.aleo/struct_a.public;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
                ",
            ),
        ),
        // Case 12 / finalize: finalize-side mirror of case 4.
        (
            "run_12",
            make(
                r"
        function run_12:
            input r0 as program_a.aleo/struct_a.public;
            async run_12 r0 into r1;
            output r1 as program_c.aleo/run_12.future;

        finalize run_12:
            input r0 as program_a.aleo/struct_a.public;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r1.a r1 into r2 as struct_c;
                ",
            ),
        ),
        // Case 13 / finalize: finalize-side mirror of case 5.
        (
            "run_13",
            make(
                r"
        function run_13:
            input r0 as program_a.aleo/struct_a.public;
            async run_13 r0 into r1;
            output r1 as program_c.aleo/run_13.future;

        finalize run_13:
            input r0 as program_a.aleo/struct_a.public;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 into r2 as program_b2.aleo/struct_b;
            cast r2.a r1 into r3 as struct_c;
                ",
            ),
        ),
        // Case 14 / finalize: finalize-side mirror of case 6.
        (
            "run_14",
            make(
                r"
        function run_14:
            async run_14 into r0;
            output r0 as program_c.aleo/run_14.future;

        finalize run_14:
            cast program_a.aleo program_b.aleo program_c.aleo into r0 as program_a.aleo/struct_d;
            cast r0.a r0.b r0.c into r1 as struct_e;
                ",
            ),
        ),
        // Case 15 / finalize: finalize-side mirror of case 7.
        (
            "run_15",
            make(
                r"
        function run_15:
            async run_15 into r0;
            output r0 as program_c.aleo/run_15.future;

        finalize run_15:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a.val into r3 as u8;
                ",
            ),
        ),
        // Case 16 / finalize: finalize-side mirror of case 8.
        (
            "run_16",
            make(
                r"
        function run_16:
            async run_16 into r0;
            output r0 as program_c.aleo/run_16.future;

        finalize run_16:
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a into r3 as program_b2.aleo/struct_b;
                ",
            ),
        ),
    ];

    deploy_variants_separately("only", &program_a, &program_b, &program_b2, &variants, rng);
}

// Checks that various instances of casts to arrays involving external structs and their members, as
// well as structs having arrays as members, work as expected, deploying each function separately.
#[test]
fn test_cast_with_external_structs_in_arrays() {
    let rng = &mut TestRng::default();

    let program_a = Program::from_str(
        r"
        program program_a.aleo;

        struct struct_a:
            val as u8;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    ).unwrap();

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
    ).unwrap();

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
    ).unwrap();

    let preamble = r"
        import program_a.aleo;
        import program_b.aleo;
        import program_b2.aleo;
        program program_c.aleo;

        struct struct_c:
            a as program_a.aleo/struct_a;
            b as program_b.aleo/struct_b;

        struct struct_arr:
            arr as [program_a.aleo/struct_a; 2u32];
    ";
    let ctor = r"
        constructor:
            assert.eq edition 0u16;
    ";
    let make = |body: &str| format!("{preamble}\n{body}\n{ctor}");

    let variants = vec![
        // Case 1 / function: cast an [struct_c; 2] from a register holding a struct_c.
        (
            "fun_1",
            make(
                r"
        function fun_1:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast r3 r3 into r4 as [struct_c; 2u32];

            output r4 as [struct_c; 2u32].private;
                ",
            ),
        ),
        // Case 2 / function: cast a [program_b.aleo/struct_b; 2] with an element read from a local struct.
        (
            "fun_2",
            make(
                r"
        function fun_2:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast r2 r3.b into r4 as [program_b.aleo/struct_b; 2u32];

            output r4 as [program_b.aleo/struct_b; 2u32].private;
                ",
            ),
        ),
        // Case 3 / function: cast a [program_a.aleo/struct_a; 5] from elements at various levels of nesting.
        (
            "fun_3",
            make(
                r"
        function fun_3:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast r1 r2.a r3.a r4.a r4.b.a into r5 as [program_a.aleo/struct_a; 5u32];

            output r5 as [program_a.aleo/struct_a; 5u32].private;
                ",
            ),
        ),
        // Case 4 / function: cast a local struct containing an array of external structs.
        (
            "fun_4",
            make(
                r"
        function fun_4:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 r1 into r2 as [program_a.aleo/struct_a; 2u32];
            cast r2 into r3 as struct_arr;

            output r3 as struct_arr.private;
                ",
            ),
        ),
        // Case 5 / finalize: finalize-side mirror of case 1.
        (
            "fun_5",
            make(
                r"
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
                ",
            ),
        ),
        // Case 6 / finalize: finalize-side mirror of case 2.
        (
            "fun_6",
            make(
                r"
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
                ",
            ),
        ),
        // Case 7 / finalize: finalize-side mirror of case 3.
        (
            "fun_7",
            make(
                r"
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
                ",
            ),
        ),
        // Case 8 / finalize: finalize-side mirror of case 4.
        (
            "fun_8",
            make(
                r"
        function fun_8:
            input r0 as u8.public;
            async fun_8 r0 into r1;
            output r1 as program_c.aleo/fun_8.future;

        finalize fun_8:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 r1 into r2 as [program_a.aleo/struct_a; 2u32];
            cast r2 into r3 as struct_arr;
                ",
            ),
        ),
    ];

    deploy_variants_separately("arrays", &program_a, &program_b, &program_b2, &variants, rng);
}

// Checks that various instances of casts to records involving external structs and their members
// behave as expected, deploying each function separately. Records are restricted to the function
// (i.e. non-finalize) scope.
#[test]
fn test_cast_with_external_structs_in_records() {
    let rng = &mut TestRng::default();

    let program_a = Program::from_str(
        r"
        program program_a.aleo;

        struct struct_a:
            val as u8;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    ).unwrap();

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
    ).unwrap();

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
    ).unwrap();

    let preamble = r"
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
    ";
    let ctor = r"
        constructor:
            assert.eq edition 0u16;
    ";
    let make = |body: &str| format!("{preamble}\n{body}\n{ctor}");

    let variants = vec![
        // Case 1: cast a record_c with two entries from a register containing a local struct_c.
        (
            "fun_1",
            make(
                r"
        function fun_1:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast self.signer r3 r3 into r4 as record_c.record;

            output r4 as record_c.record;
                ",
            ),
        ),
        // Case 2: cast a record_b from an external struct_b register and a struct_b read from a struct_c (`r3.b`).
        (
            "fun_2",
            make(
                r"
        function fun_2:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
            cast self.signer r2 r3.b into r4 as record_b.record;

            output r4 as record_b.record;
                ",
            ),
        ),
        // Case 3: cast a record_a whose five entries come from five different sources.
        (
            "fun_3",
            make(
                r"
        function fun_3:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast self.signer r1 r2.a r3.a r4.a r4.b.a into r5 as record_a.record;

            output r5 as record_a.record;
                ",
            ),
        ),
        // Case 4: cast a record_mixed by reading each entry directly from a register of the matching type.
        (
            "fun_4",
            make(
                r"
        function fun_4:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast self.signer r1 r2 r3 r4 into r5 as record_mixed.record;

            output r5 as record_mixed.record;
                ",
            ),
        ),
        // Case 5: cast a record_mixed by combining direct registers and register member reads.
        (
            "fun_5",
            make(
                r"
        function fun_5:
            input r0 as u8.private;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 into r3 as program_b2.aleo/struct_b;
            cast r1 r2 into r4 as struct_c;
            cast self.signer r4.a r4.b r3 r4 into r5 as record_mixed.record;

            output r5 as record_mixed.record;
                ",
            ),
        ),
    ];

    deploy_variants_separately("records", &program_a, &program_b, &program_b2, &variants, rng);
}

// Checks that casts to external structs (and reads of their members) are well-formed inside a
// `constructor` scope, deploying each constructor case as its own program so that a rejection can be
// attributed to a specific case. Each case is self-contained (it rebuilds the registers it needs).
#[test]
fn test_cast_with_external_structs_in_constructor() {
    let rng = &mut TestRng::default();

    let program_a = Program::from_str(
        r"
        program program_a.aleo;

        struct struct_a:
            val as u8;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    ).unwrap();

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
    ).unwrap();

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
    ).unwrap();

    // Shared preamble (imports, program declaration, structs, and a trivial function). Each variant
    // varies only the constructor body. Constructors have no inputs, so every cast starts from a
    // literal.
    let preamble = r"
        import program_a.aleo;
        import program_b.aleo;
        import program_b2.aleo;
        program program_c.aleo;

        struct struct_c:
            a as program_a.aleo/struct_a;
            b as program_b.aleo/struct_b;

        function noop:
    ";
    let make = |constructor_body: &str| {
        format!("{preamble}\n        constructor:\n            assert.eq edition 0u16;\n{constructor_body}")
    };

    let variants = vec![
        // Case 1: literal -> external struct -> external struct -> local struct.
        (
            "constructor_case_1",
            make(
                r"
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
                ",
            ),
        ),
        // Case 2: cast involving an external-struct member read (`r0.val`).
        (
            "constructor_case_2",
            make(
                r"
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0.val into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;
                ",
            ),
        ),
        // Case 3: the same struct_a cast into a program_b2/struct_b, then combined into struct_c.
        (
            "constructor_case_3",
            make(
                r"
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 into r2 as program_b2.aleo/struct_b;
            cast r2.a r1 into r3 as struct_c;
                ",
            ),
        ),
        // Case 4: three-level nested access reaching through external structs.
        (
            "constructor_case_4",
            make(
                r"
            cast 24u8 into r0 as program_a.aleo/struct_a;
            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;
            cast r2.b.a.val into r3 as u8;
                ",
            ),
        ),
    ];

    deploy_variants_separately("constructor", &program_a, &program_b, &program_b2, &variants, rng);
}

// Checks that casts to external structs (and reads of their members) are well-formed inside a
// `view` scope, deploying each view as its own program so that a rejection can be attributed to a
// specific view.
#[test]
fn test_cast_with_external_structs_in_views() {
    let rng = &mut TestRng::default();

    let program_a = Program::from_str(
        r"
        program program_a.aleo;

        struct struct_a:
            val as u8;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    ).unwrap();

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
    ).unwrap();

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
    ).unwrap();

    // Shared preamble (imports, program declaration, structs, and a trivial function) and
    // constructor. Each variant varies only the view.
    let preamble = r"
        import program_a.aleo;
        import program_b.aleo;
        import program_b2.aleo;
        program program_c.aleo;

        struct struct_c:
            a as program_a.aleo/struct_a;
            b as program_b.aleo/struct_b;

        function noop:
    ";
    let ctor = r"
        constructor:
            assert.eq edition 0u16;
    ";
    let make = |body: &str| format!("{preamble}\n{body}\n{ctor}");

    let variants = vec![
        // Case 1 / view: cast external structs defined in program_a and program_b, then a struct_c.
        (
            "view_1",
            make(
                r"
        view view_1:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1 into r2 as program_b.aleo/struct_b;
            cast r1 r2 into r3 as struct_c;

            output r3 as struct_c.public;
                ",
            ),
        ),
        // Case 2 / view: similar to case 1 but with an external-struct member read (`r1.val`).
        (
            "view_2",
            make(
                r"
        view view_2:
            input r0 as u8.public;

            cast r0 into r1 as program_a.aleo/struct_a;
            cast r1.val into r2 as program_a.aleo/struct_a;
            cast r2 into r3 as program_b.aleo/struct_b;
            cast r2 r3 into r4 as struct_c;

            output r4 as struct_c.public;
                ",
            ),
        ),
        // Case 3 / view: struct_c is cast from an external struct received as an input argument.
        (
            "view_3",
            make(
                r"
        view view_3:
            input r0 as program_a.aleo/struct_a.public;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 r1 into r2 as struct_c;

            output r2 as struct_c.public;
                ",
            ),
        ),
        // Case 4 / view: the same input external struct is cast into program_b and program_b2 structs.
        (
            "view_4",
            make(
                r"
        view view_4:
            input r0 as program_a.aleo/struct_a.public;

            cast r0 into r1 as program_b.aleo/struct_b;
            cast r0 into r2 as program_b2.aleo/struct_b;
            cast r2.a r1 into r3 as struct_c;

            output r3 as struct_c.public;
                ",
            ),
        ),
    ];

    deploy_variants_separately("views", &program_a, &program_b, &program_b2, &variants, rng);
}

#[test]
// Checks that a cast-to-external-struct instruction whose operand-type resolution is incorrect in
// the old version of the FinalizeType matches_struct check fails in the same way before after V19,
// since at both points in time there is a separate, correct check.
fn test_cast_with_external_structs_false_type() {
    let rng = &mut TestRng::default();

    let deployer_private_key = sample_genesis_private_key(rng);

    let program_a = Program::from_str(
        r"
        program program_a.aleo;

        struct my_struct:
            val_1 as u8;
            val_2 as field;

        function noop:

        constructor:
            assert.eq edition 0u16;
        ",
    ).unwrap();

    let program_b = Program::from_str(
        r"
        import program_a.aleo;
        program program_b.aleo;

        struct my_struct:
            val_1 as field;

        function cast_external:
            async cast_external into r0;
            output r0 as program_b.aleo/cast_external.future;

        finalize cast_external:
            cast 42field into r0 as my_struct;
            cast r0.val_1 1field into r1 as program_a.aleo/my_struct;

        constructor:
            assert.eq edition 0u16;
        ",
    ).unwrap();

    let err_old = {
        let vm_old = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V17).unwrap(), rng);

        let transaction = vm_old.deploy(&deployer_private_key, &program_a, None, 0, None, rng).unwrap();
        add_and_test_with_costs(&vm_old, &deployer_private_key, None, &[transaction], rng);

        vm_old.deploy(&deployer_private_key, &program_b, None, 0, None, rng).unwrap_err().to_string()
    };

    let err_new = {
        let vm_new = sample_vm_at_height(CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V19).unwrap(), rng);

        let transaction = vm_new.deploy(&deployer_private_key, &program_a, None, 0, None, rng).unwrap();
        add_and_test_with_costs(&vm_new, &deployer_private_key, None, &[transaction], rng);

        vm_new.deploy(&deployer_private_key, &program_b, None, 0, None, rng).unwrap_err().to_string()
    };

    assert_eq!(err_old, err_new);
    assert!(err_old.contains("expects a 'u8', but found a 'field'"))
}
