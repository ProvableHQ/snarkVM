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

/// Implements an Algebraic Holographic Proof (AHP) for the R1CS indexed relation.
pub mod ahp;
pub use ahp::*;

pub(crate) mod data_structures;
pub use data_structures::*;

/// Implements the Varuna zkSNARK proof system.
mod varuna;
pub use varuna::*;

/// Specifies the SNARK mode.
mod mode;
pub use mode::*;

#[cfg(test)]
pub mod tests;
