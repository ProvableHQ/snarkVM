// Copyright (c) 2019-2025 Provable Inc.
// This file is part of the snarkVM library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:

// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WinnerANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

include!("../helpers/macros.rs");

use std::sync::OnceLock;

use circuit::AleoV0;
use console::{
    network::MainnetV0,
    prelude::*,
    program::{ArrayType, Identifier, Literal, LiteralType, Plaintext, PlaintextType, Register, Value},
    types::U32,
};
use snarkvm_synthesizer_process::{Process, Stack};
use snarkvm_synthesizer_program::{In, Opcode, Operand, Program, RegistersCircuit as _, RegistersTrait as _};

use crate::helpers::sample::{sample_finalize_registers, sample_registers};

type CurrentNetwork = MainnetV0;
type CurrentAleo = AleoV0;

const ITERATIONS: usize = 25;

/// Samples the stack. Note: Do not replicate this for real program use, it is insecure.
#[allow(clippy::type_complexity)]
fn sample_stack(
    opcode: Opcode,
    type_a: LiteralType,
    type_b: ArrayType<CurrentNetwork>,
    mode_a: circuit::Mode,
    mode_b: circuit::Mode,
) -> Result<(Stack<CurrentNetwork>, [Operand<CurrentNetwork>; 2], Register<CurrentNetwork>)> {
    // Initialize the opcode.
    let opcode = opcode.to_string();

    // Initialize the function name.
    let function_name = Identifier::<CurrentNetwork>::from_str("run")?;

    // Initialize the registers.
    let r0 = Register::Locator(0);
    let r1 = Register::Locator(1);
    let r2 = Register::Locator(2);

    // Initialize the program.
    let program = Program::from_str(&format!(
        "program testing.aleo;
            function {function_name}:
                input {r0} as {type_a}.{mode_a};
                input {r1} as {type_b}.{mode_b};
                {opcode} {r0} {r1} into {r2};
                async {function_name} {r0} {r1} into r3;
                output r3 as testing.aleo/{function_name}.future;

            finalize {function_name}:
                input {r0} as {type_a}.public;
                input {r1} as {type_b}.public;
                {opcode} {r0} {r1} into {r2};
        "
    ))?;

    // Initialize the operands.
    let operand_a = Operand::Register(r0);
    let operand_b = Operand::Register(r1);
    let operands = [operand_a, operand_b];

    // Initialize the stack.
    let stack = Stack::new(&Process::load()?, &program)?;

    Ok((stack, operands, r2))
}

fn check_in(
    operation: impl FnOnce([Operand<CurrentNetwork>; 2], Register<CurrentNetwork>) -> In<CurrentNetwork>,
    opcode: Opcode,
    literal: &Literal<CurrentNetwork>,
    array: &[Literal<CurrentNetwork>],
    mode_a: &circuit::Mode,
    mode_b: &circuit::Mode,
) {
    use circuit::Eject;

    println!("Checking '{opcode}' for '{literal}.{mode_a}' and '{array:?}.{mode_b}'");

    // Initialize the types.
    let type_a = literal.to_type();
    let type_b = ArrayType::<CurrentNetwork>::new(array.last().unwrap().to_type().into(), vec![U32::new(
        array.len().try_into().unwrap(),
    )])
    .unwrap();

    // Initialize the stack.
    let (stack, operands, destination) = sample_stack(opcode, type_a, type_b, *mode_a, *mode_b).unwrap();
    // Initialize the operation.
    let operation = operation(operands, destination.clone());
    // Initialize the function name.
    let function_name = Identifier::from_str("run").unwrap();
    // Initialize a destination operand.
    let destination_operand = Operand::Register(destination);

    // Create values from literals.
    let value_a = Value::Plaintext(Plaintext::from(literal.clone()));
    let value_b =
        Value::Plaintext(Plaintext::Array(array.iter().cloned().map(Plaintext::from).collect(), OnceLock::new()));

    /* Check the operation *succeeds* when the array contains the operand. */
    if array.contains(literal) {
        // Attempt to compute the valid operand case.
        let values = [(&value_a, None), (&value_b, None)];
        let mut registers = sample_registers(&stack, &function_name, &values).unwrap();
        operation.evaluate(&stack, &mut registers).unwrap();

        // Retrieve the output.
        let output_a = registers.load_literal(&stack, &destination_operand).unwrap();

        // Ensure the output is correct.
        if let Literal::Boolean(output_a) = output_a {
            assert!(*output_a, "Instruction '{operation}' failed (console): {literal} {array:?}")
        } else {
            panic!("The output must be a boolean (console)");
        }

        // Attempt to compute the valid operand case.
        let values = [(&value_a, Some(*mode_a)), (&value_b, Some(*mode_a))];
        let mut registers = sample_registers(&stack, &function_name, &values).unwrap();
        operation.execute::<CurrentAleo>(&stack, &mut registers).unwrap();

        // Retrieve the output.
        let output_b = registers.load_literal_circuit(&stack, &destination_operand).unwrap();

        // Ensure the output is correct.
        if let circuit::Literal::Boolean(output_b) = output_b {
            assert!(
                output_b.eject_value(),
                "Instruction '{operation}' failed (circuit): {literal}.{mode_a} {array:?}.{mode_b}"
            )
        } else {
            panic!("The output must be a boolean (circuit)");
        }

        // Ensure the circuit is satisfied.
        assert!(
            <CurrentAleo as circuit::Environment>::is_satisfied(),
            "Instruction '{operation}' should be satisfied (circuit): {literal}.{mode_a} {array:?}.{mode_b}"
        );

        // Reset the circuit.
        <CurrentAleo as circuit::Environment>::reset();

        // Attempt to finalize the valid operand case.
        let mut registers = sample_finalize_registers(&stack, &function_name, &[&value_a, &value_b]).unwrap();
        operation.finalize(&stack, &mut registers).unwrap();

        // Retrieve the output.
        let output_c = registers.load_literal(&stack, &destination_operand).unwrap();

        // Ensure the output is correct.
        if let Literal::Boolean(output_c) = output_c {
            assert!(*output_c, "Instruction '{operation}' failed (finalize): {literal} {array:?}")
        } else {
            panic!("The output must be a boolean (finalize)");
        }
    }
    /* Check the operation *fails* when the array does not contains the operand. */
    else {
        // Attempt to compute the valid operand case.
        let values = [(&value_a, None), (&value_b, None)];
        let mut registers = sample_registers(&stack, &function_name, &values).unwrap();
        operation.evaluate(&stack, &mut registers).unwrap();

        // Retrieve the output.
        let output_a = registers.load_literal(&stack, &destination_operand).unwrap();

        // Ensure the output is correct.
        if let Literal::Boolean(output_a) = output_a {
            assert!(!*output_a, "Instruction '{operation}' should have failed (console): {literal} {array:?}")
        } else {
            panic!("The output must be a boolean (console)");
        }

        // Attempt to compute the valid operand case.
        let values = [(&value_a, Some(*mode_a)), (&value_b, Some(*mode_a))];
        let mut registers = sample_registers(&stack, &function_name, &values).unwrap();
        operation.execute::<CurrentAleo>(&stack, &mut registers).unwrap();

        // Retrieve the output.
        let output_b = registers.load_literal_circuit(&stack, &destination_operand).unwrap();

        // Ensure the output is correct.
        if let circuit::Literal::Boolean(output_b) = output_b {
            assert!(
                !output_b.eject_value(),
                "Instruction '{operation}' should have failed (circuit): {literal}.{mode_a} {array:?}.{mode_b}"
            )
        } else {
            panic!("The output must be a boolean (circuit)");
        }

        // Ensure the circuit is satisfied.
        assert!(
            <CurrentAleo as circuit::Environment>::is_satisfied(),
            "Instruction '{operation}' should be satisfied (circuit): {literal}.{mode_a} {array:?}.{mode_b}"
        );

        // Reset the circuit.
        <CurrentAleo as circuit::Environment>::reset();

        // Attempt to finalize the valid operand case.
        let mut registers = sample_finalize_registers(&stack, &function_name, &[&value_a, &value_b]).unwrap();
        operation.finalize(&stack, &mut registers).unwrap();

        // Retrieve the output.
        let output_c = registers.load_literal(&stack, &destination_operand).unwrap();

        // Ensure the output is correct.
        if let Literal::Boolean(output_c) = output_c {
            assert!(!*output_c, "Instruction '{operation}' should have failed (finalize): {literal} {array:?}")
        } else {
            panic!("The output must be a boolean (finalize)");
        }
    }
}

fn check_in_fails(
    opcode: Opcode,
    literal: &Literal<CurrentNetwork>,
    array: &[Literal<CurrentNetwork>],
    mode_a: &circuit::Mode,
    mode_b: &circuit::Mode,
) {
    // Initialize the types.
    // Initialize the types.
    let type_a = literal.to_type();
    let type_b = ArrayType::<CurrentNetwork>::new(array.last().unwrap().to_type().into(), vec![U32::new(
        array.len().try_into().unwrap(),
    )])
    .unwrap();
    assert_ne!(
        &PlaintextType::Literal(type_a),
        type_b.next_element_type(),
        "The literal and array elements must be *different* types for this test"
    );

    // If the types mismatch, ensure the stack fails to initialize.
    let result = sample_stack(opcode, type_a, type_b.clone(), *mode_a, *mode_b);
    assert!(
        result.is_err(),
        "Stack should have failed to initialize for: {opcode} {type_a}.{mode_a} {type_b}.{mode_b}"
    );
}

#[test]
fn test_in_succeeds() {
    // Initialize the operation.
    let operation = |operands, destination| In::<CurrentNetwork>::new(operands, destination).unwrap();
    // Initialize the opcode.
    let opcode = In::<CurrentNetwork>::opcode();

    // Prepare the rng.
    let mut rng = TestRng::default();

    // Prepare the test.
    let modes_a = [circuit::Mode::Public, circuit::Mode::Private];
    let modes_b = [circuit::Mode::Public, circuit::Mode::Private];

    for _ in 0..ITERATIONS {
        let literals = sample_literals!(CurrentNetwork, &mut rng);
        let arrays = sample_arrays(&mut rng);
        for (literal, array) in literals.iter().zip_eq(arrays.iter()) {
            for mode_a in &modes_a {
                for mode_b in &modes_b {
                    // Check the operation.
                    check_in(operation, opcode, literal, array, mode_a, mode_b);
                }
            }
        }
    }
}

#[test]
fn test_in_fails() {
    // Initialize the opcode.
    let opcode = In::<CurrentNetwork>::opcode();

    // Prepare the rng.
    let mut rng = TestRng::default();

    // Prepare the test.
    let modes_a = [circuit::Mode::Public, circuit::Mode::Private];
    let modes_b = [circuit::Mode::Public, circuit::Mode::Private];

    for _ in 0..ITERATIONS {
        let literals = sample_literals!(CurrentNetwork, &mut rng);
        let arrays = sample_arrays(&mut rng);
        for array in &arrays {
            let type_array = ArrayType::<CurrentNetwork>::new(array.last().unwrap().to_type().into(), vec![U32::new(
                array.len().try_into().unwrap(),
            )])
            .unwrap();
            for literal in &literals {
                if PlaintextType::from(literal.to_type()) != *type_array.next_element_type() {
                    for mode_a in &modes_a {
                        for mode_b in &modes_b {
                            // Check the operation fails.
                            check_in_fails(opcode, literal, array, mode_a, mode_b);
                        }
                    }
                }
            }
        }
    }
}

/// Samples a random array of a random length for each literal type.
fn sample_arrays(rng: &mut TestRng) -> Vec<Vec<Literal<CurrentNetwork>>> {
    use rand::Rng;

    let len = rng.gen_range(1..32);
    let mut outer = Vec::with_capacity(17);

    for choice in 0..17 {
        let mut inner = Vec::with_capacity(len);
        match choice {
            0 => {
                for _ in 0..len {
                    inner
                        .push(console::program::Literal::<CurrentNetwork>::Address(console::types::Address::rand(rng)));
                }
            }
            1 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::Boolean(console::types::Boolean::rand(rng)));
                }
            }
            2 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::Field(console::types::Field::rand(rng)));
                }
            }
            3 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::Group(console::types::Group::rand(rng)));
                }
            }
            4 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::I8(console::types::I8::rand(rng)));
                }
            }
            5 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::I16(console::types::I16::rand(rng)));
                }
            }
            6 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::I32(console::types::I32::rand(rng)));
                }
            }
            7 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::I64(console::types::I64::rand(rng)));
                }
            }
            8 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::I128(console::types::I128::rand(rng)));
                }
            }
            9 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::U8(console::types::U8::rand(rng)));
                }
            }
            10 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::U16(console::types::U16::rand(rng)));
                }
            }
            11 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::U32(console::types::U32::rand(rng)));
                }
            }
            12 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::U64(console::types::U64::rand(rng)));
                }
            }
            13 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::U128(console::types::U128::rand(rng)));
                }
            }
            14 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::Scalar(console::types::Scalar::rand(rng)));
                }
            }
            15 => {
                for _ in 0..len {
                    inner.push(console::program::Literal::sample(console::program::LiteralType::Signature, rng));
                }
            }
            _ => {
                for _ in 0..len {
                    inner.push(console::program::Literal::String(console::types::StringType::rand(rng)));
                }
            }
        }

        outer.push(inner);
    }

    outer
}
