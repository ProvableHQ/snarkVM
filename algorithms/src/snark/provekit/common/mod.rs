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

// Originally derived from ProveKit, Copyright 2026 World Foundation (MIT).

pub mod field;
pub mod hash_config;
mod interner;
pub mod prefix_covector;
pub mod public_inputs;
mod r1cs;
pub mod sparse_matrix;
pub mod utils;
mod whir_r1cs;

pub use field::{Base, Ext, FieldHash, ProofField};
pub use hash_config::{HashConfig, POSEIDON2, SKYSCRAPER};
pub use interner::{InternedFieldElement, Interner};
pub use prefix_covector::{OffsetCovector, PrefixCovector, SparseCovector};
pub use public_inputs::{PublicInputs, PublicInputsHash};
pub use r1cs::R1CS;
pub use sparse_matrix::{HydratedSparseMatrix, SparseMatrix};
pub use whir_r1cs::{MIN_WHIR_NUM_VARIABLES, ProvekitProof, R1csHash, WhirR1CSProof, WhirR1CSScheme};
