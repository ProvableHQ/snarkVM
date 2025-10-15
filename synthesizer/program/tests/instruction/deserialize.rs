// Copyright (c) 2019-2025 Provable Inc.
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

use crate::helpers::sample::{sample_finalize_registers, sample_registers};

use circuit::{AleoV0, Eject};
use console::{
    network::MainnetV0,
    prelude::*,
    program::{ArrayType, Identifier, LiteralType, Plaintext, PlaintextType, Register, U32, Value},
};
use snarkvm_synthesizer_process::{Process, Stack};
use snarkvm_synthesizer_program::{
    DeserializeBits,
    DeserializeBitsRaw,
    DeserializeInstruction,
    DeserializeVariant,
    Opcode,
    Operand,
    Program,
    RegistersCircuit as _,
    RegistersTrait as _,
};

type CurrentNetwork = MainnetV0;
type CurrentAleo = AleoV0;

const ITERATIONS: usize = 1000;

/// Samples the stack. Note: Do not replicate this for real program use, it is insecure.
#[allow(clippy::type_complexity)]
fn sample_stack(
    opcode: Opcode,
    type_: &PlaintextType<CurrentNetwork>,
    bits: &ArrayType<CurrentNetwork>,
    mode: circuit::Mode,
) -> Result<(Stack<CurrentNetwork>, Vec<Operand<CurrentNetwork>>, Register<CurrentNetwork>)> {
    // Initialize the opcode.
    let opcode = opcode.to_string();

    // Initialize the function name.
    let function_name = Identifier::<CurrentNetwork>::from_str("run")?;

    // Initialize the registers.
    let r0 = Register::Locator(0);
    let r1 = Register::Locator(1);

    // Initialize the program.
    let program = Program::from_str(&format!(
        "program testing.aleo;
            function {function_name}:
                input {r0} as {bits}.{mode};
                {opcode} {r0} ({bits}) into {r1} ({type_});
                async {function_name} {r0} into r2;
                output r2 as testing.aleo/{function_name}.future;
            finalize {function_name}:
                input {r0} as {bits}.public;
                {opcode} {r0} ({bits}) into {r1} ({type_});
        "
    ))?;

    // Initialize the operands.
    let operands = vec![Operand::Register(r0)];

    // Initialize the stack.
    let stack = Stack::new(&Process::load()?, &program)?;

    Ok((stack, operands, r1))
}

// This test function verifies that the deserialize instruction is consistent across evaluation, execution, and finalization.
// It repeats the test for a desired number of iterations and until it reaches a desired number of failures.
fn check_deserialize<const VARIANT: u8>(
    operation: impl FnOnce(
        Vec<Operand<CurrentNetwork>>,
        ArrayType<CurrentNetwork>,
        Register<CurrentNetwork>,
        PlaintextType<CurrentNetwork>,
    ) -> DeserializeInstruction<CurrentNetwork, VARIANT>,
    opcode: Opcode,
    type_: &PlaintextType<CurrentNetwork>,
    mode: &circuit::Mode,
    iterations: usize,
    num_failures: usize,
) {
    // Initalize an RNG.
    let rng = &mut TestRng::default();

    // Struct definitions are not supported.
    let fail_get_struct = |_: &Identifier<CurrentNetwork>| bail!("structs are not supported");

    // Get the size in bits.
    let size_in_bits = match VARIANT {
        0 => type_.size_in_bits(&fail_get_struct).unwrap(),
        1 => type_.size_in_bits_raw(&fail_get_struct).unwrap(),
        _ => panic!("Invalid 'deserialize' variant"),
    };
    let size_in_bits = u32::try_from(size_in_bits).unwrap();

    println!("Checking '{opcode}' for '{type_}.{mode}' to [boolean; {size_in_bits}u32]");

    // Construct the array type.
    let bits_type = ArrayType::new(PlaintextType::Literal(LiteralType::Boolean), vec![U32::new(size_in_bits)]).unwrap();

    // Initialize the stack.
    let (stack, operands, destination) = sample_stack(opcode, type_, &bits_type, *mode).unwrap();

    // Initialize the operation.
    let operation = operation(operands, bits_type.clone(), destination.clone(), type_.clone());
    // Initialize the function name.
    let function_name = Identifier::from_str("run").unwrap();
    // Initialize a destination operand.
    let destination_operand = Operand::Register(destination);

    // Run the test for a desired number of iterations and yntil we reach the desired number of failures.
    let mut failures = 0;
    let mut total_iterations = 0;

    while failures < num_failures || total_iterations < iterations {
        // Sample the plaintext.
        let plaintext = stack.sample_plaintext(type_, rng).unwrap();

        // Get the bits of the plaintext.
        // On odd iterations, use the correct bits.
        // On even iterations, sample random bits of the correct size.
        let bits = match (type_, total_iterations % 2 == 1) {
            // Note. We make an exception for scalar types since the underlying implementation panics in a hard-to-test way.
            (PlaintextType::Literal(LiteralType::Scalar), _) | (_, true) => match VARIANT {
                0 => plaintext.to_bits_le(),
                1 => plaintext.to_bits_raw_le(),
                _ => panic!("Invalid 'deserialize' variant"),
            },
            (_, false) => {
                stack.sample_plaintext(&PlaintextType::Array(bits_type.clone()), rng).unwrap().to_bits_raw_le()
            }
        };

        // Check that the number of bits matches.
        assert_eq!(bits.len(), size_in_bits as usize, "The number of bits does not match the expected size");

        // Construct the bit array input.
        let bit_array = Plaintext::from_bit_array(bits, size_in_bits).unwrap();

        // Attempt to evaluate the valid operand case.
        let mut evaluate_registers =
            sample_registers(&stack, &function_name, &[(Value::Plaintext(bit_array.clone()), None)]).unwrap();
        let result_a = operation.evaluate(&stack, &mut evaluate_registers);

        // Attempt to execute the valid operand case.
        let mut execute_registers =
            sample_registers(&stack, &function_name, &[(Value::Plaintext(bit_array.clone()), Some(*mode))]).unwrap();
        let result_b = operation.execute::<CurrentAleo>(&stack, &mut execute_registers);

        // Attempt to finalize the valid operand case.
        let mut finalize_registers = sample_finalize_registers(&stack, &function_name, &[bit_array]).unwrap();
        let result_c = operation.finalize(&stack, &mut finalize_registers);

        // Check that either all operations failed, or all operations succeeded.
        let result_a_is_ok = result_a.is_ok();
        let result_b_is_ok = result_b.is_ok() && <CurrentAleo as circuit::Environment>::is_satisfied();
        let result_c_is_ok = result_c.is_ok();
        let all_failed = !result_a_is_ok && !result_b_is_ok && !result_c_is_ok;
        let all_succeeded = result_a_is_ok && result_b_is_ok && result_c_is_ok;
        assert!(
            all_failed ^ all_succeeded,
            "The results of the evaluation (pass: {result_a_is_ok}), execution (pass: {result_b_is_ok}), and finalization (pass: {result_c_is_ok}) should either all succeed or all fail",
        );

        // If all operations succeeded, check that the outputs are consistent.
        if all_succeeded {
            // Retrieve the output of evaluation.
            let output_a = evaluate_registers.load(&stack, &destination_operand).unwrap();

            // Retrieve the output of execution.
            let output_b = execute_registers.load_circuit(&stack, &destination_operand).unwrap();

            // Retrieve the output of finalization.
            let output_c = finalize_registers.load(&stack, &destination_operand).unwrap();

            // Check that the outputs are consistent.
            assert_eq!(
                output_a,
                output_b.eject_value(),
                "The results of the evaluation and execution are inconsistent"
            );
            assert_eq!(output_a, output_c, "The results of the evaluation and finalization are inconsistent");
        }
        // Otherwise, increment the failure counter.
        else {
            failures += 1;
        }
        // Reset the circuit.
        <CurrentAleo as circuit::Environment>::reset();
        // Increment the total iteration counter.
        total_iterations += 1;
    }
}

// Get the types to be tested and the required number of failures.
// For some data types, failures are expected when the input bits do not correspond to a valid encoding of the data type.
// For example, not all bit strings of length 253 are valid encodings of a field element.
// In other cases, such as integers, all bit strings of the correct length are valid encodings, and no failures are expected.
fn test_types(variant: DeserializeVariant) -> Vec<(PlaintextType<CurrentNetwork>, usize)> {
    let mut types = vec![
        (PlaintextType::Literal(LiteralType::Address), 25),
        (PlaintextType::Literal(LiteralType::Boolean), 0),
        (PlaintextType::Literal(LiteralType::Field), 25),
        (PlaintextType::Literal(LiteralType::Group), 25),
        (PlaintextType::Literal(LiteralType::I8), 0),
        (PlaintextType::Literal(LiteralType::I16), 0),
        (PlaintextType::Literal(LiteralType::I32), 0),
        (PlaintextType::Literal(LiteralType::I64), 0),
        (PlaintextType::Literal(LiteralType::I128), 0),
        (PlaintextType::Literal(LiteralType::U8), 0),
        (PlaintextType::Literal(LiteralType::U16), 0),
        (PlaintextType::Literal(LiteralType::U32), 0),
        (PlaintextType::Literal(LiteralType::U64), 0),
        (PlaintextType::Literal(LiteralType::U128), 0),
        // Note. We make an exception for scalar types since the underlying implementation panics in a hard-to-test way.
        (PlaintextType::Literal(LiteralType::Scalar), 0),
        (PlaintextType::Array(ArrayType::new(PlaintextType::Literal(LiteralType::U8), vec![U32::new(8)]).unwrap()), 0),
    ];

    // Add additional types for the raw variant.
    if variant == DeserializeVariant::FromBitsRaw {
        types.push((
            PlaintextType::Array(ArrayType::new(PlaintextType::Literal(LiteralType::U8), vec![U32::new(32)]).unwrap()),
            0,
        ))
    }

    types
}

macro_rules! test_deserialize {
        ($name: tt, $deserialize:ident, $variant:ident, $iterations:expr) => {
            paste::paste! {
                #[test]
                fn [<test _ $name _ is _ consistent>]() {
                    // Initialize the operation.
                    let operation = |operands, operand_type, destination, destination_type| $deserialize::<CurrentNetwork>::new(operands, operand_type, destination, destination_type).unwrap();
                    // Initialize the opcode.
                    let opcode = $deserialize::<CurrentNetwork>::opcode();

                    // Prepare the test.
                    let modes = [circuit::Mode::Public, circuit::Mode::Private];

                    for mode in modes.iter() {
                        for (type_, num_failures) in test_types(DeserializeVariant::$variant).iter() {
                            check_deserialize(
                                operation,
                                opcode,
                                type_,
                                mode,
                                $iterations,
                                *num_failures,
                            );
                        }
                    }
                }
            }
        };
    }

test_deserialize!(deserialize_bits, DeserializeBits, FromBits, ITERATIONS);
test_deserialize!(deserialize_bits_raw, DeserializeBitsRaw, FromBitsRaw, ITERATIONS);

// This test verifies that programs that use deserialize with the wrong bit sizes fail to compile.
#[test]
fn test_deserialize_invalid_types() {
    // Load a process.
    let mut process = Process::<CurrentNetwork>::load().unwrap();

    // Sample an rng.
    let rng = &mut TestRng::default();

    // Verify that programs that use deserialize with the wrong types fail to compile.
    for (i, variant) in [DeserializeVariant::FromBits, DeserializeVariant::FromBitsRaw].into_iter().enumerate() {
        for j in 0..ITERATIONS {
            for (k, (type_, _)) in test_types(DeserializeVariant::FromBits).iter().enumerate() {
                println!("Testing deserialize program with invalid type {type_} for iteration {i}");

                // A dummy function to get the struct definition.
                let fail_get_struct = |_: &Identifier<CurrentNetwork>| bail!("structs are not supported");

                // Determine if we are testing the raw variant.
                let is_raw = variant == DeserializeVariant::FromBitsRaw;

                // Get the size in bits.
                let size_in_bits = match is_raw {
                    false => type_.size_in_bits(&fail_get_struct).unwrap(),
                    true => type_.size_in_bits_raw(&fail_get_struct).unwrap(),
                };

                // Sample a wrong size in bits.
                let wrong_size_in_bits = loop {
                    let candidate = rng.gen_range(1..=CurrentNetwork::MAX_ARRAY_ELEMENTS);
                    if candidate != size_in_bits {
                        break candidate;
                    }
                };

                // Get the instruction suffix.
                let suffix = if is_raw { ".raw" } else { "" };

                // Sample a program that uses deserialize with the wrong bit size in the function scope.
                let program = Program::from_str(&format!(
                    "program testing_{i}_{j}_{k}.aleo;
                function run:
                    input r0 as [boolean; {wrong_size_in_bits}u32].public;
                    deserialize.bits{suffix} r0 ([boolean; {wrong_size_in_bits}u32]) into r1 ({type_});
                ",
                ))
                .unwrap();

                // Verify that the program cannot be added to the process.
                let result = process.add_program(&program);
                assert!(result.is_err());

                // Sample a program that uses deserialize with the wrong bit size in the function scope.
                let program = Program::from_str(&format!(
                    "program testing_{i}_{j}_{k}.aleo;
                function run:
                    input r0 as [boolean; {wrong_size_in_bits}u32].public;
                    async run r0 into r1;
                    output r1 as testing_{i}_{j}_{k}.aleo/run.future;
                finalize run:
                    input r0 as [boolean; {wrong_size_in_bits}u32].public;
                    deserialize.bits{suffix} r0 ([boolean; {wrong_size_in_bits}u32]) into r1 ({type_});
                ",
                ))
                .unwrap();

                // Verify that the program cannot be added to the process.
                let result = process.add_program(&program);
                assert!(result.is_err());

                // Sample a program that uses the correct bit size in the function and finalize scope.
                let program = Program::from_str(&format!(
                    "program testing_{i}_{j}_{k}.aleo;
                function run:
                    input r0 as [boolean; {size_in_bits}u32].public;
                    deserialize.bits{suffix} r0 ([boolean; {size_in_bits}u32]) into r1 ({type_});
                    async run r0 into r2;
                    output r2 as testing_{i}_{j}_{k}.aleo/run.future;
                finalize run:
                    input r0 as [boolean; {size_in_bits}u32].public;
                    deserialize.bits{suffix} r0 ([boolean; {size_in_bits}u32]) into r1 ({type_});
                ",
                ))
                .unwrap();

                // Verify that the program can be added to the prcess.
                process.add_program(&program).unwrap();
            }
        }
    }
}
