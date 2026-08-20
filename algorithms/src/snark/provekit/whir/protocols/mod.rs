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

// Originally derived from WHIR (https://github.com/WizardOfMenlo/whir),
// licensed under Apache-2.0 OR MIT.

//! Interactive (sub)protocols for WHIR.
//!
//! These interact through the [`spongefish`] Fiat–Shamir transformation.
//!
//! Protocols are parameterized through `Config` structs. These implement serde
//! `Serialize` and `Deserialize` and importantly all generics are *also*
//! serialized so the serialization captures all necessary information to
//! uniquely identify a concrete protocol. The intention is for the hash of the
//! Config serialization to serve as protocol domain separator for Spongefish.

pub mod basecase;
pub mod challenge_indices;
pub mod geometric_challenge;
pub mod irs_commit;
pub mod matrix_commit;
pub mod merkle_tree;
pub mod proof_of_work;
pub mod sumcheck;
pub mod whir;
