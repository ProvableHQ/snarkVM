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
//
// Note: The code needs to be commented out to minimize build/clippy times.
//
// #[macro_use]
// extern crate criterion;
//
// use console::{
//     network::{MainnetV0, Network},
//     prelude::{TestRng, Uniform, Zero},
//     program::{
//         Identifier,
//         Boolean,
//         Field,
//         Group,
//         Plaintext,
//         Register,
//         Scalar,
//         Value,
//         I128,
//         I16,
//         I32,
//         I64,
//         I8,
//         U128,
//         U16,
//         U32,
//         U64,
//         U8,
//     },
// };
// use snarkvm_synthesizer_program::{
//     Finalize,
//     FinalizeGlobalState,
//     Instruction,
//     RegistersStore,
// };
// use synthesizer_process::{
//     FinalizeRegisters,
//     FinalizeTypes,
//     Process,
//     Stack,
// };
//
// use criterion::{BatchSize, Criterion};
// use std::{fmt::Display, iter, str::FromStr};
//
// // TODO (d0cd): Add benchmarks using `Address` once random sampling for `Address` is supported.
// // TODO (d0cd): Generalize macros to support arbitrary arity instructions (low priority).
//
// /// A helper function to construct a set of `FinalizeRegisters` with the given arguments.
// fn setup_finalize_registers(
//     stack: &Stack<MainnetV0>,
//     finalize_body: impl Display,
//     args: &[Value<MainnetV0>],
// ) -> FinalizeRegisters<MainnetV0> {
//     // Initialize a `Finalize` block with the benchmark arguments as inputs.
//     let mut finalize_string = "finalize foo:".to_string();
//     for (i, arg) in args.iter().enumerate() {
//         finalize_string.push_str(&format!("input r{i} as {}.public;", match arg {
//             Value::Plaintext(Plaintext::Literal(literal, _)) => literal.to_type().to_string(),
//             _ => panic!("invalid benchmark argument type"),
//         }));
//     }
//     finalize_string.push_str(&finalize_body.to_string());
//     let finalize = Finalize::<MainnetV0>::from_str(&finalize_string).unwrap();
//     // Construct the finalize state.
//     let state = FinalizeGlobalState::new::<MainnetV0>(0, 0, 0, 0, <MainnetV0 as Network>::BlockHash::default()).unwrap();
//     // Initialize a fresh set of finalize registers.
//     let mut registers = FinalizeRegisters::new(state, <MainnetV0 as Network>::TransitionID::default(), Identifier::from_str("test").unwrap(),  FinalizeTypes::from_finalize(stack, &finalize).unwrap());
//     // Add the arguments into the registers.
//     for (i, arg) in args.iter().enumerate() {
//         registers.store(stack, &Register::Locator(i as u64), arg.clone()).unwrap();
//     }
//     registers
// }
//
// #[rustfmt::skip]
// fn bench_instructions(c: &mut Criterion) {
//     // Initialize an RNG.
//     let rng = &mut TestRng::default();
//     // Initialize a process.
//     let process = Process::<MainnetV0>::load().unwrap();
//     // Get the stack for the credits program.
//     // Note that this is not used for anything other than to satisfy the function signature for `finalize`.
//     // This is because `Stack`s are only used in finalize contexts to check that structs are well-formed.
//     let stack = process.get_stack("credits.aleo").unwrap();
//
//      macro_rules! bench_instruction {
//         // Benchmark a unary instruction, with the given sampling method.
//         ($samples:tt, $instruction:ident { $input:ident , }) => {
//             {
//                 use snarkvm_synthesizer_program::$instruction;
//                 let name = concat!(stringify!($instruction), "/", stringify!($input));
//                 let instruction = Instruction::<MainnetV0>::$instruction($instruction::from_str(&format!("{} r0 into r1", $instruction::<MainnetV0>::opcode().to_string())).unwrap());
//                 c.bench_function(&format!("{name}/instruction"), |b| {
//                     b.iter_batched(
//                         || {
//                             let arg = $samples.next().unwrap();
//                             setup_finalize_registers(stack, instruction.to_string(), &[Value::from_str(&arg.to_string()).unwrap()])
//                         },
//                         |mut finalize_registers| instruction.finalize(stack, &mut finalize_registers).unwrap(),
//                         BatchSize::PerIteration,
//                     )
//                 });
//             };
//         };
//         // Benchmark a unary instruction, with the given sampling method.
//         ($samples:tt, $instruction:ident { $input:ident , }, $as_type:expr) => {
//             {
//                 use snarkvm_synthesizer_program::$instruction;
//                 let name = concat!(stringify!($instruction), "/", stringify!($input));
//                 let instruction = Instruction::<MainnetV0>::$instruction($instruction::from_str(&format!("{} r0 into r1 as {}", $instruction::<MainnetV0>::opcode().to_string(), $as_type)).unwrap());
//                 c.bench_function(&format!("{name}/instruction"), |b| {
//                     b.iter_batched(
//                         || {
//                             let arg = $samples.next().unwrap();
//                             setup_finalize_registers(stack, instruction.to_string(), &[Value::from_str(&arg.to_string()).unwrap()])
//                         },
//                         |mut finalize_registers| instruction.finalize(stack, &mut finalize_registers).unwrap(),
//                         BatchSize::PerIteration,
//                     )
//                 });
//             };
//         };
//         // Benchmark a binary instruction, with the given sampling method.
//         ($samples:tt, $instruction:ident { ($input_a:ident, $input_b:ident) , }) => {
//             {
//                 use snarkvm_synthesizer_program::$instruction;
//                 let name = concat!(stringify!($instruction), "/", stringify!($input_a), "_", stringify!($input_b));
//                 let instruction = Instruction::<MainnetV0>::$instruction($instruction::from_str(&format!("{} r0 r1 into r2", $instruction::<MainnetV0>::opcode().to_string())).unwrap());
//                 c.bench_function(&format!("{name}/instruction"), |b| {
//                     b.iter_batched(
//                         || {
//                             let (first, second) = $samples.next().unwrap();
//                             setup_finalize_registers(stack, instruction.to_string(), &[Value::from_str(&first.to_string()).unwrap(), Value::from_str(&second.to_string()).unwrap()])
//                         },
//                         |mut finalize_registers| instruction.finalize(stack, &mut finalize_registers).unwrap(),
//                         BatchSize::PerIteration,
//                     )
//                 });
//             };
//         };
//         // Benchmark a ternary instruction, with the given sampling method.
//         ($samples:tt, $instruction:ident { ($input_a:ident, $input_b:ident, $input_c:ident), }) => {
//             {
//                 use snarkvm_synthesizer_program::$instruction;
//                 let name = concat!(stringify!($instruction), "/", stringify!($input_a), "_",  stringify!($input_b), "_", stringify!($input_c));
//                 let instruction = Instruction::<MainnetV0>::$instruction($instruction::from_str(&format!("{} r0 r1 r2 into r3", $instruction::<MainnetV0>::opcode().to_string())).unwrap());
//                 c.bench_function(&format!("{name}/instruction"), |b| {
//                     b.iter_batched(
//                         || {
//                             let (first, second, third) = $samples.next().unwrap();
//                             setup_finalize_registers(stack, instruction.to_string(), &[Value::from_str(&first.to_string()).unwrap(), Value::from_str(&second.to_string()).unwrap(), Value::from_str(&third.to_string()).unwrap()])
//                         },
//                         |mut finalize_registers| instruction.finalize(stack, &mut finalize_registers).unwrap(),
//                         BatchSize::PerIteration,
//                     )
//                 });
//             }
//         };
//     }
//
//     macro_rules! bench_instruction_with_default {
//         // Benchmark a unary instruction, using the default sampling method.
//         ($operation:tt, $instruction:ident { $( $input:ident , )+ }) => {
//             $({
//                 // Define the default sampling method.
//                 let mut samples = iter::repeat_with(|| {
//                     let mut arg: $input::<MainnetV0> = Uniform::rand(rng);
//                     while (std::panic::catch_unwind(|| arg.$operation())).is_err() {
//                         arg = Uniform::rand(rng);
//                     }
//                     arg
//                 });
//                 // Benchmark the underlying operation.
//                 let name = concat!(stringify!($instruction), "/", stringify!($input));
//                 c.bench_function(&format!("{name}/core"), |b| {
//                     b.iter_batched(
//                         || samples.next().unwrap(),
//                         |arg| arg.$operation(),
//                         BatchSize::SmallInput,
//                     )
//                 });
//                 // Benchmark the instruction.
//                 bench_instruction!(samples, $instruction { $input , });
//             })+
//         };
//         // Benchmark a unary instruction with a question mark (?), using the default sampling method.
//         ($operation:tt?, $instruction:ident { $( $input:ident , )+ }) => {
//             $({
//                 // Define the default sampling method.
//                 let mut samples = iter::repeat_with(|| {
//                     let mut arg: $input::<MainnetV0> = Uniform::rand(rng);
//                     while (std::panic::catch_unwind(|| arg.$operation().unwrap())).is_err() {
//                         arg = Uniform::rand(rng);
//                     }
//                     arg
//                 });
//                 // Benchmark the underlying operation.
//                 let name = concat!(stringify!($instruction), "/", stringify!($input));
//                 c.bench_function(&format!("{name}/core"), |b| {
//                     b.iter_batched(
//                         || samples.next().unwrap(),
//                         |arg| arg.$operation().unwrap(),
//                         BatchSize::SmallInput,
//                     )
//                 });
//                 // Benchmark the instruction.
//                 bench_instruction!(samples, $instruction { $input , });
//             })+
//         };
//         // Benchmark a binary instruction, using the default sampling method.
//         ($operation:tt, $instruction:ident { $( ($input_a:ident, $input_b:ident) , )+ }) => {
//             $({
//                 // Define the default sampling method.
//                 let mut samples = iter::repeat_with(|| {
//                     let mut first: $input_a::<MainnetV0> = Uniform::rand(rng);
//                     let mut second: $input_b::<MainnetV0> = Uniform::rand(rng);
//                     while (std::panic::catch_unwind(|| first.$operation(&second))).is_err() {
//                         first = Uniform::rand(rng);
//                         second = Uniform::rand(rng);
//                     }
//                     (first, second)
//                 });
//                 // Benchmark the underlying operation.
//                 let name = concat!(stringify!($instruction), "/", stringify!($input_a), "_", stringify!($input_b));
//                 c.bench_function(&format!("{name}/core"), |b| {
//                     b.iter_batched(
//                         || samples.next().unwrap(),
//                         |(first, second)| first.$operation(&second),
//                         BatchSize::SmallInput,
//                     )
//                 });
//                 // Benchmark the instruction.
//                 bench_instruction!(samples, $instruction { ($input_a, $input_b) , });
//             })+
//         };
//         // Benchmark a ternary instruction, with the default sampling method.
//         ($operation:tt, $instruction:ident { $( ($input_a:ident, $input_b:ident, $input_c:ident), )+ }) => {
//             $({
//                 let mut samples = iter::repeat_with(|| {
//                     let mut first: $input_a::<MainnetV0> = Uniform::rand(rng);
//                     let mut second: $input_b::<MainnetV0> = Uniform::rand(rng);
//                     let mut third: $input_c::<MainnetV0> = Uniform::rand(rng);
//                     while (std::panic::catch_unwind(|| $input_b::ternary(&first, &second, &third))).is_err() {
//                         first = Uniform::rand(rng);
//                         second = Uniform::rand(rng);
//                         third = Uniform::rand(rng);
//                     }
//                     (first, second, third)
//                 });
//                 // Benchmark the underlying operation.
//                 let name = concat!(stringify!($instruction), "/", stringify!($input_a), "_",  stringify!($input_b), "_", stringify!($input_c));
//                 c.bench_function(&format!("{name}/core"), |b| {
//                     b.iter_batched(
//                         || samples.next().unwrap(),
//                         |(first, second, third)| $input_b::ternary(&first, &second, &third),
//                         BatchSize::SmallInput,
//                     )
//                 });
//                 // Benchmark the instruction.
//                 bench_instruction!(samples, $instruction { ($input_a, $input_b, $input_c), });
//             })+
//         };
//     }
//
//     use console::prelude::AbsChecked;
//     bench_instruction_with_default!(abs_checked, Abs { I8, I16, I32, I64, I128, });
//
//     use console::prelude::AbsWrapped;
//     bench_instruction_with_default!(abs_wrapped, AbsWrapped { I8, I16, I32, I64, I128, });
//
//     use std::ops::Add;
//     bench_instruction_with_default!(add, Add {
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//     });
//
//     use console::prelude::AddWrapped;
//     bench_instruction_with_default!(add_wrapped, AddWrapped {
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//     });
//
//     macro_rules! bench_assert {
//         ($typ:tt) => {
//             let mut samples = iter::repeat_with(|| {
//                 let result = $typ::<MainnetV0>::rand(rng);
//                 (result.clone(), result)
//             });
//             {
//                 use snarkvm_synthesizer_program::AssertEq;
//                 let name = concat!("AssertEq/", stringify!($typ), "_", stringify!($typ));
//                 let instruction = Instruction::<MainnetV0>::AssertEq(AssertEq::from_str(&format!("{} r0 r1", AssertEq::<MainnetV0>::opcode().to_string())).unwrap());
//                 c.bench_function(&format!("{name}/instruction"), |b| {
//                     b.iter_batched(
//                         || {
//                             let (first, second) = samples.next().unwrap();
//                             setup_finalize_registers(stack, instruction.to_string(), &[Value::from_str(&first.to_string()).unwrap(), Value::from_str(&second.to_string()).unwrap()])
//                         },
//                         |mut finalize_registers| instruction.finalize(stack, &mut finalize_registers).unwrap(),
//                         BatchSize::PerIteration,
//                     )
//                 });
//             };
//             let mut samples = iter::repeat_with(|| {
//                 let first = $typ::<MainnetV0>::rand(rng);
//                 let mut second = $typ::<MainnetV0>::rand(rng);
//                 while first == second {
//                     second = $typ::<MainnetV0>::rand(rng);
//                 }
//                 (first, second)
//             });
//             {
//                 use snarkvm_synthesizer_program::AssertNeq;
//                 let name = concat!("AssertNeq/", stringify!($typ), "_", stringify!($typ));
//                 let instruction = Instruction::<MainnetV0>::AssertNeq(AssertNeq::from_str(&format!("{} r0 r1", AssertNeq::<MainnetV0>::opcode().to_string())).unwrap());
//                 c.bench_function(&format!("{name}/instruction"), |b| {
//                     b.iter_batched(
//                         || {
//                             let (first, second) = samples.next().unwrap();
//                             setup_finalize_registers(stack, instruction.to_string(), &[Value::from_str(&first.to_string()).unwrap(), Value::from_str(&second.to_string()).unwrap()])
//                         },
//                         |mut finalize_registers| instruction.finalize(stack, &mut finalize_registers).unwrap(),
//                         BatchSize::PerIteration,
//                     )
//                 });
//             };
//         };
//     }
//
//     bench_assert!(Boolean);
//     bench_assert!(Field);
//     bench_assert!(Group);
//     bench_assert!(I8);
//     bench_assert!(I16);
//     bench_assert!(I32);
//     bench_assert!(I64);
//     bench_assert!(I128);
//     bench_assert!(Scalar);
//     bench_assert!(U8);
//     bench_assert!(U16);
//     bench_assert!(U32);
//     bench_assert!(U64);
//     bench_assert!(U128);
//
//     use core::ops::BitAnd;
//     bench_instruction_with_default!(bitand, And {
//         (Boolean, Boolean),
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//     });
//
//     macro_rules! bench_ped64_commit_instruction {
//         ($instruction:tt) => {
//             let mut samples = iter::repeat_with(|| { (Boolean::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (Boolean, Scalar), });
//             let mut samples = iter::repeat_with(|| { (I8::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (I8, Scalar), });
//             let mut samples = iter::repeat_with(|| { (I16::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (I16, Scalar), });
//             let mut samples = iter::repeat_with(|| { (I32::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (I32, Scalar), });
//             let mut samples = iter::repeat_with(|| { (U8::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (U8, Scalar), });
//             let mut samples = iter::repeat_with(|| { (U16::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (U16, Scalar), });
//             let mut samples = iter::repeat_with(|| { (U32::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (U32, Scalar), });
//         }
//     }
//
//     macro_rules! bench_commit_instruction {
//         ($instruction:tt) => {
//             bench_ped64_commit_instruction!($instruction);
//             let mut samples = iter::repeat_with(|| { (Field::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (Field, Scalar), });
//             let mut samples = iter::repeat_with(|| { (Group::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (Group, Scalar), });
//             let mut samples = iter::repeat_with(|| { (I64::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (I64, Scalar), });
//             let mut samples = iter::repeat_with(|| { (I128::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (I128, Scalar), });
//             let mut samples = iter::repeat_with(|| { (U64::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (U64, Scalar), });
//             let mut samples = iter::repeat_with(|| { (U128::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (U128, Scalar), });
//             let mut samples = iter::repeat_with(|| { (Scalar::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//             bench_instruction!(samples, $instruction { (Scalar, Scalar), });
//         }
//     }
//
//     bench_commit_instruction!(CommitBHP256);
//     bench_commit_instruction!(CommitBHP512);
//     bench_commit_instruction!(CommitBHP768);
//     bench_commit_instruction!(CommitBHP1024);
//
//     bench_ped64_commit_instruction!(CommitPED64);
//
//     bench_ped64_commit_instruction!(CommitPED128);
//     let mut samples = iter::repeat_with(|| { (I64::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, CommitPED128 { (I64, Scalar), });
//     let mut samples = iter::repeat_with(|| { (U64::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, CommitPED128 { (U64, Scalar), });
//
//     use console::prelude::Div;
//     bench_instruction_with_default!(div, Div {
//         (Field, Field),
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//     });
//
//     use console::prelude::DivWrapped;
//     bench_instruction_with_default!(div_wrapped, DivWrapped {
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//     });
//
//     use console::prelude::Double;
//     bench_instruction_with_default!(double, Double { Field, Group, });
//
//     use console::prelude::Compare;
//     bench_instruction_with_default!(is_greater_than, GreaterThan {
//         (Field, Field),
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//         (Scalar, Scalar),
//     });
//
//     bench_instruction_with_default!(is_greater_than_or_equal, GreaterThanOrEqual {
//         (Field, Field),
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//         (Scalar, Scalar),
//     });
//
//     macro_rules! bench_ped64_hash_instruction {
//         ($instruction:tt) => {
//             let mut samples = iter::repeat_with(|| { Boolean::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { Boolean, }, "group");
//             let mut samples = iter::repeat_with(|| { I8::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { I8, }, "group");
//             let mut samples = iter::repeat_with(|| { I16::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { I16, }, "group");
//             let mut samples = iter::repeat_with(|| { I32::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { I32, }, "group");
//             let mut samples = iter::repeat_with(|| { U8::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { U8, }, "group");
//             let mut samples = iter::repeat_with(|| { U16::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { U16, }, "group");
//             let mut samples = iter::repeat_with(|| { U32::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { U32, }, "group");
//         }
//     }
//
//     macro_rules! bench_hash_instruction {
//         ($instruction:tt) => {
//             bench_ped64_hash_instruction!($instruction);
//             let mut samples = iter::repeat_with(|| { Field::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { Field, }, "group");
//             let mut samples = iter::repeat_with(|| { Group::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { Group, }, "group");
//             let mut samples = iter::repeat_with(|| { I64::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { I64, }, "group");
//             let mut samples = iter::repeat_with(|| { I128::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { I128, }, "group");
//             let mut samples = iter::repeat_with(|| { U64::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { U64, }, "group");
//             let mut samples = iter::repeat_with(|| { U128::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { U128, }, "group");
//             let mut samples = iter::repeat_with(|| { Scalar::<MainnetV0>::rand(rng) });
//             bench_instruction!(samples, $instruction { Scalar, }, "group");
//         }
//     }
//
//     bench_hash_instruction!(HashBHP256);
//     bench_hash_instruction!(HashBHP512);
//     bench_hash_instruction!(HashBHP768);
//     bench_hash_instruction!(HashBHP1024);
//
//     bench_ped64_hash_instruction!(HashPED64);
//
//     bench_ped64_hash_instruction!(HashPED128);
//     let mut samples = iter::repeat_with(|| { I64::<MainnetV0>::rand(rng) });
//     bench_instruction!(samples, HashPED128 { I64, }, "group");
//     let mut samples = iter::repeat_with(|| { U64::<MainnetV0>::rand(rng) });
//     bench_instruction!(samples, HashPED128 { U64, }, "group");
//
//     bench_hash_instruction!(HashPSD2);
//     bench_hash_instruction!(HashPSD4);
//     bench_hash_instruction!(HashPSD8);
//
//     use console::prelude::Inverse;
//     bench_instruction_with_default!(inverse?, Inv { Field, });
//
//     let mut samples = iter::repeat_with(|| { (Boolean::<MainnetV0>::rand(rng), Boolean::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (Boolean, Boolean), });
//     bench_instruction!(samples, IsNeq { (Boolean, Boolean), });
//     let mut samples = iter::repeat_with(|| { (Field::<MainnetV0>::rand(rng), Field::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (Field, Field), });
//     bench_instruction!(samples, IsNeq { (Field, Field), });
//     let mut samples = iter::repeat_with(|| { (Group::<MainnetV0>::rand(rng), Group::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (Group, Group), });
//     bench_instruction!(samples, IsNeq { (Group, Group), });
//     let mut samples = iter::repeat_with(|| { (I8::<MainnetV0>::rand(rng), I8::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (I8, I8), });
//     bench_instruction!(samples, IsNeq { (I8, I8), });
//     let mut samples = iter::repeat_with(|| { (I16::<MainnetV0>::rand(rng), I16::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (I16, I16), });
//     bench_instruction!(samples, IsNeq { (I16, I16), });
//     let mut samples = iter::repeat_with(|| { (I32::<MainnetV0>::rand(rng), I32::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (I32, I32), });
//     bench_instruction!(samples, IsNeq { (I32, I32), });
//     let mut samples = iter::repeat_with(|| { (I64::<MainnetV0>::rand(rng), I64::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (I64, I64), });
//     bench_instruction!(samples, IsNeq { (I64, I64), });
//     let mut samples = iter::repeat_with(|| { (I128::<MainnetV0>::rand(rng), I128::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (I128, I128), });
//     bench_instruction!(samples, IsNeq { (I128, I128), });
//     let mut samples = iter::repeat_with(|| { (Scalar::<MainnetV0>::rand(rng), Scalar::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (Scalar, Scalar), });
//     bench_instruction!(samples, IsNeq { (Scalar, Scalar), });
//     let mut samples = iter::repeat_with(|| { (U8::<MainnetV0>::rand(rng), U8::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (U8, U8), });
//     bench_instruction!(samples, IsNeq { (U8, U8), });
//     let mut samples = iter::repeat_with(|| { (U16::<MainnetV0>::rand(rng), U16::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (U16, U16), });
//     bench_instruction!(samples, IsNeq { (U16, U16), });
//     let mut samples = iter::repeat_with(|| { (U32::<MainnetV0>::rand(rng), U32::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (U32, U32), });
//     bench_instruction!(samples, IsNeq { (U32, U32), });
//     let mut samples = iter::repeat_with(|| { (U64::<MainnetV0>::rand(rng), U64::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (U64, U64), });
//     bench_instruction!(samples, IsNeq { (U64, U64), });
//     let mut samples = iter::repeat_with(|| { (U128::<MainnetV0>::rand(rng), U128::<MainnetV0>::rand(rng)) });
//     bench_instruction!(samples, IsEq { (U128, U128), });
//     bench_instruction!(samples, IsNeq { (U128, U128), });
//
//     bench_instruction_with_default!(is_less_than, LessThan {
//         (Field, Field),
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//         (Scalar, Scalar),
//     });
//
//     bench_instruction_with_default!(is_less_than_or_equal, LessThanOrEqual {
//         (Field, Field),
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//         (Scalar, Scalar),
//     });
//
//     use console::prelude::Modulo;
//     bench_instruction_with_default!(modulo, Modulo {
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//     });
//
//     use core::ops::Mul;
//     bench_instruction_with_default!(mul, Mul {
//         (Field, Field),
//         (Group, Scalar),
//         (Scalar, Group),
//     });
//     // Use a custom sampling method for integer multiplication, since there is a high chance of overflow.
//     let mut samples = iter::repeat((I8::<MainnetV0>::zero(), I8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Mul { (I8, I8), });
//     let mut samples = iter::repeat((I16::<MainnetV0>::zero(), I16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Mul { (I16, I16), });
//     let mut samples = iter::repeat((I32::<MainnetV0>::zero(), I32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Mul { (I32, I32), });
//     let mut samples = iter::repeat((I64::<MainnetV0>::zero(), I64::<MainnetV0>::zero()));
//     bench_instruction!(samples, Mul { (I64, I64), });
//     let mut samples = iter::repeat((I128::<MainnetV0>::zero(), I128::<MainnetV0>::zero()));
//     bench_instruction!(samples, Mul { (I128, I128), });
//     let mut samples = iter::repeat((U8::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Mul { (U8, U8), });
//     let mut samples = iter::repeat((U16::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Mul { (U16, U16), });
//     let mut samples = iter::repeat((U32::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Mul { (U32, U32), });
//     let mut samples = iter::repeat((U64::<MainnetV0>::zero(), U64::<MainnetV0>::zero()));
//     bench_instruction!(samples, Mul { (U64, U64), });
//     let mut samples = iter::repeat((U128::<MainnetV0>::zero(), U128::<MainnetV0>::zero()));
//     bench_instruction!(samples, Mul { (U128, U128), });
//
//     use console::prelude::MulWrapped;
//     bench_instruction_with_default!(mul_wrapped, MulWrapped {
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//     });
//
//     use console::prelude::Nand;
//     bench_instruction_with_default!(nand, Nand {
//         (Boolean, Boolean),
//     });
//
//     use core::ops::Neg;
//     bench_instruction_with_default!(neg, Neg { Field, Group, I8, I16, I32, I64, I128, });
//
//     use console::prelude::Nor;
//     bench_instruction_with_default!(nor, Nor {
//         (Boolean, Boolean),
//     });
//
//     use core::ops::Not;
//     bench_instruction_with_default!(not, Not { Boolean, I8, I16, I32, I64, I128, U8, U16, U32, U64, });
//
//     use core::ops::BitOr;
//     bench_instruction_with_default!(bitor, Or {
//         (Boolean, Boolean),
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//     });
//
//     use console::prelude::Pow;
//     bench_instruction_with_default!(pow, Pow {
//         (Field, Field),
//     });
//     // Use a custom sampling method for integer exponentiation, since there is a high chance of overflow.
//     let mut samples = iter::repeat((I8::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I8, U8), });
//     let mut samples = iter::repeat((I8::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I8, U16), });
//     let mut samples = iter::repeat((I8::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I8, U32), });
//     let mut samples = iter::repeat((I16::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I16, U8), });
//     let mut samples = iter::repeat((I16::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I16, U16), });
//     let mut samples = iter::repeat((I16::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I16, U32), });
//     let mut samples = iter::repeat((I32::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I32, U8), });
//     let mut samples = iter::repeat((I32::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I32, U16), });
//     let mut samples = iter::repeat((I32::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I32, U32), });
//     let mut samples = iter::repeat((I64::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I64, U8), });
//     let mut samples = iter::repeat((I64::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I64, U16), });
//     let mut samples = iter::repeat((I64::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I64, U32), });
//     let mut samples = iter::repeat((I128::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I128, U8), });
//     let mut samples = iter::repeat((I128::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I128, U16), });
//     let mut samples = iter::repeat((I128::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (I128, U32), });
//     let mut samples = iter::repeat((U8::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U8, U8), });
//     let mut samples = iter::repeat((U8::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U8, U16), });
//     let mut samples = iter::repeat((U8::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U8, U32), });
//     let mut samples = iter::repeat((U16::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U16, U8), });
//     let mut samples = iter::repeat((U16::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U16, U16), });
//     let mut samples = iter::repeat((U16::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U16, U32), });
//     let mut samples = iter::repeat((U32::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U32, U8), });
//     let mut samples = iter::repeat((U32::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U32, U16), });
//     let mut samples = iter::repeat((U32::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U32, U32), });
//     let mut samples = iter::repeat((U64::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U64, U8), });
//     let mut samples = iter::repeat((U64::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U64, U16), });
//     let mut samples = iter::repeat((U64::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U64, U32), });
//     let mut samples = iter::repeat((U128::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U128, U8), });
//     let mut samples = iter::repeat((U128::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U128, U16), });
//     let mut samples = iter::repeat((U128::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Pow { (U128, U32), });
//
//     use console::prelude::PowWrapped;
//     bench_instruction_with_default!(pow_wrapped, PowWrapped {
//         (I8, U8),
//         (I8, U16),
//         (I8, U32),
//         (I16, U8),
//         (I16, U16),
//         (I16, U32),
//         (I32, U8),
//         (I32, U16),
//         (I32, U32),
//         (I64, U8),
//         (I64, U16),
//         (I64, U32),
//         (I128, U8),
//         (I128, U16),
//         (I128, U32),
//         (U8, U8),
//         (U8, U16),
//         (U8, U32),
//         (U16, U8),
//         (U16, U16),
//         (U16, U32),
//         (U32, U8),
//         (U32, U16),
//         (U32, U32),
//         (U64, U8),
//         (U64, U16),
//         (U64, U32),
//         (U128, U8),
//         (U128, U16),
//         (U128, U32),
//     });
//
//     use core::ops::Rem;
//     bench_instruction_with_default!(rem, Rem {
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//     });
//
//     use console::prelude::RemWrapped;
//     bench_instruction_with_default!(rem_wrapped, RemWrapped {
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//     });
//
//     // Use a custom sampling method for left-shift, since there is a high chance of overflow.
//     let mut samples = iter::repeat((I8::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I8, U8), });
//     let mut samples = iter::repeat((I8::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I8, U16), });
//     let mut samples = iter::repeat((I8::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I8, U32), });
//     let mut samples = iter::repeat((I16::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I16, U8), });
//     let mut samples = iter::repeat((I16::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I16, U16), });
//     let mut samples = iter::repeat((I16::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I16, U32), });
//     let mut samples = iter::repeat((I32::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I32, U8), });
//     let mut samples = iter::repeat((I32::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I32, U16), });
//     let mut samples = iter::repeat((I32::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I32, U32), });
//     let mut samples = iter::repeat((I64::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I64, U8), });
//     let mut samples = iter::repeat((I64::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I64, U16), });
//     let mut samples = iter::repeat((I64::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I64, U32), });
//     let mut samples = iter::repeat((I128::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I128, U8), });
//     let mut samples = iter::repeat((I128::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I128, U16), });
//     let mut samples = iter::repeat((I128::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (I128, U32), });
//     let mut samples = iter::repeat((U8::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U8, U8), });
//     let mut samples = iter::repeat((U8::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U8, U16), });
//     let mut samples = iter::repeat((U8::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U8, U32), });
//     let mut samples = iter::repeat((U16::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U16, U8), });
//     let mut samples = iter::repeat((U16::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U16, U16), });
//     let mut samples = iter::repeat((U16::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U16, U32), });
//     let mut samples = iter::repeat((U32::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U32, U8), });
//     let mut samples = iter::repeat((U32::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U32, U16), });
//     let mut samples = iter::repeat((U32::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U32, U32), });
//     let mut samples = iter::repeat((U64::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U64, U8), });
//     let mut samples = iter::repeat((U64::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U64, U16), });
//     let mut samples = iter::repeat((U64::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U64, U32), });
//     let mut samples = iter::repeat((U128::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U128, U8), });
//     let mut samples = iter::repeat((U128::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U128, U16), });
//     let mut samples = iter::repeat((U128::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shl { (U128, U32), });
//
//     use console::prelude::ShlWrapped;
//     bench_instruction_with_default!(shl_wrapped, ShlWrapped {
//         (I8, U8),
//         (I8, U16),
//         (I8, U32),
//         (I16, U8),
//         (I16, U16),
//         (I16, U32),
//         (I32, U8),
//         (I32, U16),
//         (I32, U32),
//         (I64, U8),
//         (I64, U16),
//         (I64, U32),
//         (I128, U8),
//         (I128, U16),
//         (I128, U32),
//         (U8, U8),
//         (U8, U16),
//         (U8, U32),
//         (U16, U8),
//         (U16, U16),
//         (U16, U32),
//         (U32, U8),
//         (U32, U16),
//         (U32, U32),
//         (U64, U8),
//         (U64, U16),
//         (U64, U32),
//         (U128, U8),
//         (U128, U16),
//         (U128, U32),
//     });
//
//     // Use a custom sampling method for left-shift, since there is a high chance of overflow.
//     let mut samples = iter::repeat((I8::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I8, U8), });
//     let mut samples = iter::repeat((I8::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I8, U16), });
//     let mut samples = iter::repeat((I8::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I8, U32), });
//     let mut samples = iter::repeat((I16::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I16, U8), });
//     let mut samples = iter::repeat((I16::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I16, U16), });
//     let mut samples = iter::repeat((I16::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I16, U32), });
//     let mut samples = iter::repeat((I32::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I32, U8), });
//     let mut samples = iter::repeat((I32::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I32, U16), });
//     let mut samples = iter::repeat((I32::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I32, U32), });
//     let mut samples = iter::repeat((I64::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I64, U8), });
//     let mut samples = iter::repeat((I64::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I64, U16), });
//     let mut samples = iter::repeat((I64::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I64, U32), });
//     let mut samples = iter::repeat((I128::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I128, U8), });
//     let mut samples = iter::repeat((I128::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I128, U16), });
//     let mut samples = iter::repeat((I128::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (I128, U32), });
//     let mut samples = iter::repeat((U8::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U8, U8), });
//     let mut samples = iter::repeat((U8::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U8, U16), });
//     let mut samples = iter::repeat((U8::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U8, U32), });
//     let mut samples = iter::repeat((U16::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U16, U8), });
//     let mut samples = iter::repeat((U16::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U16, U16), });
//     let mut samples = iter::repeat((U16::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U16, U32), });
//     let mut samples = iter::repeat((U32::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U32, U8), });
//     let mut samples = iter::repeat((U32::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U32, U16), });
//     let mut samples = iter::repeat((U32::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U32, U32), });
//     let mut samples = iter::repeat((U64::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U64, U8), });
//     let mut samples = iter::repeat((U64::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U64, U16), });
//     let mut samples = iter::repeat((U64::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U64, U32), });
//     let mut samples = iter::repeat((U128::<MainnetV0>::zero(), U8::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U128, U8), });
//     let mut samples = iter::repeat((U128::<MainnetV0>::zero(), U16::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U128, U16), });
//     let mut samples = iter::repeat((U128::<MainnetV0>::zero(), U32::<MainnetV0>::zero()));
//     bench_instruction!(samples, Shr { (U128, U32), });
//
//     use console::prelude::ShrWrapped;
//     bench_instruction_with_default!(shr_wrapped, ShrWrapped {
//         (I8, U8),
//         (I8, U16),
//         (I8, U32),
//         (I16, U8),
//         (I16, U16),
//         (I16, U32),
//         (I32, U8),
//         (I32, U16),
//         (I32, U32),
//         (I64, U8),
//         (I64, U16),
//         (I64, U32),
//         (I128, U8),
//         (I128, U16),
//         (I128, U32),
//         (U8, U8),
//         (U8, U16),
//         (U8, U32),
//         (U16, U8),
//         (U16, U16),
//         (U16, U32),
//         (U32, U8),
//         (U32, U16),
//         (U32, U32),
//         (U64, U8),
//         (U64, U16),
//         (U64, U32),
//         (U128, U8),
//         (U128, U16),
//         (U128, U32),
//     });
//
//     use console::prelude::Square;
//     bench_instruction_with_default!(square, Square { Field, });
//
//     use console::prelude::SquareRoot;
//     bench_instruction_with_default!(square_root?, SquareRoot { Field, });
//
//     use console::prelude::Sub;
//     bench_instruction_with_default!(sub, Sub {
//         (Field, Field),
//         (Group, Group),
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//     });
//
//     use console::prelude::SubWrapped;
//     bench_instruction_with_default!(sub_wrapped, SubWrapped {
//         (I8, I8),
//         (I16, I16),
//         (I32, I32),
//         (I64, I64),
//         (I128, I128),
//         (U8, U8),
//         (U16, U16),
//         (U32, U32),
//         (U64, U64),
//         (U128, U128),
//     });
//
//     use console::prelude::Ternary;
//     bench_instruction_with_default!(ternary, Ternary {
//         // (Boolean, Address, Address),
//         (Boolean, Boolean, Boolean),
//         (Boolean, Field, Field),
//         (Boolean, Group, Group),
//         (Boolean, I8, I8),
//         (Boolean, I16, I16),
//         (Boolean, I32, I32),
//         (Boolean, I64, I64),
//         (Boolean, I128, I128),
//         (Boolean, U8, U8),
//         (Boolean, U16, U16),
//         (Boolean, U32, U32),
//         (Boolean, U64, U64),
//         (Boolean, U128, U128),
//         (Boolean, Scalar, Scalar),
//     });
// }
//
// criterion_group! {
//     name = bench;
//     config = Criterion::default().sample_size(100);
//     targets = bench_instructions,
// }
//
// criterion_main!(bench);
