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

#![forbid(unsafe_code)]
#![warn(clippy::cast_possible_truncation)]
// TODO (howardwu): Update the return type on `execute` after stabilizing the interface.
#![allow(clippy::type_complexity)]

#[macro_use]
extern crate tracing;

#[cfg(feature = "process")]
pub use synthesizer_process as process;
#[cfg(feature = "program")]
pub use synthesizer_program as program;
#[cfg(feature = "snark")]
pub use synthesizer_snark as snark;

#[cfg(feature = "process")]
pub use crate::process::{Authorization, CallMetrics, Process, Stack, Trace};
#[cfg(feature = "program")]
pub use crate::program::{Closure, Command, Finalize, Function, Instruction, Program};

#[cfg(all(feature = "process", feature = "program", feature = "snark"))]
mod restrictions;
#[cfg(all(feature = "process", feature = "program", feature = "snark"))]
pub use restrictions::*;

#[cfg(all(feature = "process", feature = "program", feature = "snark"))]
pub mod vm;
#[cfg(all(feature = "process", feature = "program", feature = "snark"))]
pub use vm::*;

pub mod prelude {
    #[cfg(feature = "process")]
    pub use crate::process::*;
    #[cfg(feature = "program")]
    pub use crate::program::{Closure, Finalize, Function, Instruction, Mapping, Program};
    #[cfg(feature = "snark")]
    pub use crate::snark::{Certificate, Proof, ProvingKey, UniversalSRS, VerifyingKey};
    #[cfg(all(feature = "process", feature = "program", feature = "snark"))]
    pub use crate::vm::*;
}
