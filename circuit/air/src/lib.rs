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

#![forbid(unsafe_code)]
#![allow(clippy::too_many_arguments)]

//! Plonky3-style AIR arithmetization, kept separate from the R1CS gadget EDSL.
//!
//! `snarkvm-circuit-environment::Environment` parameterizes **network** (curve,
//! domains) and records `A * B = C` constraints. That API is not an
//! arithmetization backend: gadgets assume free linear combinations and a
//! multiplication gate. This crate adds a parallel `Air` / `AirBuilder` surface
//! for uniform trace constraints, plus a lowering from an R1CS `Assignment`.

mod air;
pub use air::*;

mod builder;
pub use builder::*;

mod debug;
pub use debug::*;

mod expr;
pub use expr::*;

mod poseidon;
pub use poseidon::*;

mod r1cs;
pub use r1cs::*;

mod symbolic;
pub use symbolic::*;

mod trace;
pub use trace::*;
