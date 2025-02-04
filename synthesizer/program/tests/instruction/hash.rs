// Copyright 2024 Aleo Network Foundation
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

include!("../helpers/macros.rs");

use crate::helpers::sample::{sample_finalize_registers, sample_registers};

use circuit::{AleoV0, Eject};
use console::{
    network::MainnetV0,
    prelude::*,
    program::{Identifier, Literal, LiteralType, Plaintext, PlaintextType, Register, Value},
};
use snarkvm_synthesizer_program::{
    HashBHP256,
    HashBHP512,
    HashBHP768,
    HashBHP1024,
    HashInstruction,
    HashKeccak256,
    HashKeccak384,
    HashKeccak512,
    HashPED64,
    HashPED128,
    HashPSD2,
    HashPSD4,
    HashPSD8,
    HashSha3_256,
    HashSha3_384,
    HashSha3_512,
    Opcode,
    Operand,
    Program,
    RegistersLoad,
    RegistersLoadCircuit,
};
use synthesizer_process::{Process, Stack};

type CurrentNetwork = MainnetV0;
type CurrentAleo = AleoV0;

const ITERATIONS: usize = 25;

/// **Attention**: When changing this, also update in `src/logic/instruction/hash.rs`.
fn valid_destination_types<N: Network>() -> &'static [PlaintextType<N>] {
    &[
        PlaintextType::Literal(LiteralType::Address),
        PlaintextType::Literal(LiteralType::Field),
        PlaintextType::Literal(LiteralType::Group),
        PlaintextType::Literal(LiteralType::I8),
        PlaintextType::Literal(LiteralType::I16),
        PlaintextType::Literal(LiteralType::I32),
        PlaintextType::Literal(LiteralType::I64),
        PlaintextType::Literal(LiteralType::I128),
        PlaintextType::Literal(LiteralType::U8),
        PlaintextType::Literal(LiteralType::U16),
        PlaintextType::Literal(LiteralType::U32),
        PlaintextType::Literal(LiteralType::U64),
        PlaintextType::Literal(LiteralType::U128),
        PlaintextType::Literal(LiteralType::Scalar),
    ]
}

/// Samples the stack. Note: Do not replicate this for real program use, it is insecure.
#[allow(clippy::type_complexity)]
fn sample_stack(
    opcode: Opcode,
    type_: LiteralType,
    mode: circuit::Mode,
    destination_type: PlaintextType<CurrentNetwork>,
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
                input {r0} as {type_}.{mode};
                {opcode} {r0} into {r1} as {destination_type};
                async {function_name} {r0} into r2;
                output r2 as testing.aleo/{function_name}.future;
            finalize {function_name}:
                input {r0} as {type_}.public;
                {opcode} {r0} into {r1} as {destination_type};
        "
    ))?;

    // Initialize the operands.
    let operands = vec![Operand::Register(r0)];

    // Initialize the stack.
    let stack = Stack::new(&Process::load()?, &program)?;

    Ok((stack, operands, r1))
}

fn check_hash<const VARIANT: u8>(
    operation: impl FnOnce(
        Vec<Operand<CurrentNetwork>>,
        Register<CurrentNetwork>,
        PlaintextType<CurrentNetwork>,
    ) -> HashInstruction<CurrentNetwork, VARIANT>,
    opcode: Opcode,
    literal: &Literal<CurrentNetwork>,
    mode: &circuit::Mode,
    destination_type: PlaintextType<CurrentNetwork>,
) {
    println!("Checking '{opcode}' for '{literal}.{mode}'");

    // Initialize the types.
    let type_ = literal.to_type();

    // Initialize the stack.
    let (stack, operands, destination) = sample_stack(opcode, type_, *mode, destination_type.clone()).unwrap();

    // Initialize the operation.
    let operation = operation(operands, destination.clone(), destination_type.clone());
    // Initialize the function name.
    let function_name = Identifier::from_str("run").unwrap();
    // Initialize a destination operand.
    let destination_operand = Operand::Register(destination);

    // Attempt to evaluate the valid operand case.
    let mut evaluate_registers = sample_registers(&stack, &function_name, &[(literal, None)]).unwrap();
    let result_a = operation.evaluate(&stack, &mut evaluate_registers);

    // Attempt to execute the valid operand case.
    let mut execute_registers = sample_registers(&stack, &function_name, &[(literal, Some(*mode))]).unwrap();
    let result_b = operation.execute::<CurrentAleo>(&stack, &mut execute_registers);

    // Attempt to finalize the valid operand case.
    let mut finalize_registers = sample_finalize_registers(&stack, &function_name, &[literal]).unwrap();
    let result_c = operation.finalize(&stack, &mut finalize_registers);

    // Check that either all operations failed, or all operations succeeded.
    let all_failed = result_a.is_err() && result_b.is_err() && result_c.is_err();
    let all_succeeded = result_a.is_ok() && result_b.is_ok() && result_c.is_ok();
    assert!(
        all_failed || all_succeeded,
        "The results of the evaluation, execution, and finalization should either all succeed or all fail"
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
        assert_eq!(output_a, output_b.eject_value(), "The results of the evaluation and execution are inconsistent");
        assert_eq!(output_a, output_c, "The results of the evaluation and finalization are inconsistent");

        // Check that the output type is consistent with the declared type.
        match output_a {
            Value::Plaintext(Plaintext::Literal(literal, _)) => {
                assert_eq!(
                    PlaintextType::Literal(literal.to_type()),
                    destination_type,
                    "The output type is inconsistent with the declared type"
                );
            }
            _ => unreachable!("The output type is inconsistent with the declared type"),
        }
    }

    // Reset the circuit.
    <CurrentAleo as circuit::Environment>::reset();
}

macro_rules! test_hash {
        ($name: tt, $hash:ident, $iterations:expr) => {
            paste::paste! {
                #[test]
                fn [<test _ $name _ is _ consistent>]() {
                    // Initialize the operation.
                    let operation = |operands, destination, destination_type| $hash::<CurrentNetwork>::new(operands, destination, destination_type).unwrap();
                    // Initialize the opcode.
                    let opcode = $hash::<CurrentNetwork>::opcode();

                    // Prepare the rng.
                    let mut rng = TestRng::default();

                    // Prepare the test.
                    let modes = [circuit::Mode::Public, circuit::Mode::Private];

                    for _ in 0..$iterations {
                        let literals = sample_literals!(CurrentNetwork, &mut rng);
                        for literal in literals.iter() {
                            for mode in modes.iter() {
                                for destination_type in valid_destination_types() {
                                    check_hash(
                                        operation,
                                        opcode,
                                        literal,
                                        mode,
                                        destination_type.clone(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        };
    }

test_hash!(hash_bhp256, HashBHP256, ITERATIONS);
test_hash!(hash_bhp512, HashBHP512, ITERATIONS);
test_hash!(hash_bhp768, HashBHP768, ITERATIONS);
test_hash!(hash_bhp1024, HashBHP1024, ITERATIONS);

test_hash!(hash_keccak256, HashKeccak256, 5);
test_hash!(hash_keccak384, HashKeccak384, 5);
test_hash!(hash_keccak512, HashKeccak512, 5);

test_hash!(hash_psd2, HashPSD2, ITERATIONS);
test_hash!(hash_psd4, HashPSD4, ITERATIONS);
test_hash!(hash_psd8, HashPSD8, ITERATIONS);

test_hash!(hash_sha3_256, HashSha3_256, 5);
test_hash!(hash_sha3_384, HashSha3_384, 5);
test_hash!(hash_sha3_512, HashSha3_512, 5);

// Note this test must be explicitly written, instead of using the macro, because HashPED64 fails on certain input types.
#[test]
fn test_hash_ped64_is_consistent() {
    // Prepare the rng.
    let mut rng = TestRng::default();

    // Prepare the test.
    let modes = [circuit::Mode::Public, circuit::Mode::Private];

    macro_rules! check_hash {
        ($operation:tt) => {
            for _ in 0..ITERATIONS {
                let literals = [
                    Literal::Boolean(console::types::Boolean::rand(&mut rng)),
                    Literal::I8(console::types::I8::rand(&mut rng)),
                    Literal::I16(console::types::I16::rand(&mut rng)),
                    Literal::I32(console::types::I32::rand(&mut rng)),
                    Literal::U8(console::types::U8::rand(&mut rng)),
                    Literal::U16(console::types::U16::rand(&mut rng)),
                    Literal::U32(console::types::U32::rand(&mut rng)),
                ];
                for literal in literals.iter() {
                    for mode in modes.iter() {
                        for destination_type in valid_destination_types() {
                            check_hash(
                                |operands, destination, destination_type| {
                                    $operation::<CurrentNetwork>::new(operands, destination, destination_type).unwrap()
                                },
                                $operation::<CurrentNetwork>::opcode(),
                                literal,
                                mode,
                                destination_type.clone(),
                            );
                        }
                    }
                }
            }
        };
    }
    check_hash!(HashPED64);
}

// Note this test must be explicitly written, instead of using the macro, because HashPED128 fails on certain input types.
#[test]
fn test_hash_ped128_is_consistent() {
    // Prepare the rng.
    let mut rng = TestRng::default();

    // Prepare the test.
    let modes = [circuit::Mode::Public, circuit::Mode::Private];

    macro_rules! check_hash {
        ($operation:tt) => {
            for _ in 0..ITERATIONS {
                let literals = [
                    Literal::Boolean(console::types::Boolean::rand(&mut rng)),
                    Literal::I8(console::types::I8::rand(&mut rng)),
                    Literal::I16(console::types::I16::rand(&mut rng)),
                    Literal::I32(console::types::I32::rand(&mut rng)),
                    Literal::I64(console::types::I64::rand(&mut rng)),
                    Literal::U8(console::types::U8::rand(&mut rng)),
                    Literal::U16(console::types::U16::rand(&mut rng)),
                    Literal::U32(console::types::U32::rand(&mut rng)),
                    Literal::U64(console::types::U64::rand(&mut rng)),
                ];
                for literal in literals.iter() {
                    for mode in modes.iter() {
                        for destination_type in valid_destination_types() {
                            check_hash(
                                |operands, destination, destination_type| {
                                    $operation::<CurrentNetwork>::new(operands, destination, destination_type).unwrap()
                                },
                                $operation::<CurrentNetwork>::opcode(),
                                literal,
                                mode,
                                destination_type.clone(),
                            );
                        }
                    }
                }
            }
        };
    }
    check_hash!(HashPED128);
}
