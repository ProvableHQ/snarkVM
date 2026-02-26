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

#![warn(unsafe_code)]
#![allow(clippy::module_inception)]
#![allow(clippy::type_complexity)]
#![cfg_attr(test, allow(clippy::assertions_on_result_states))]

#[allow(unused_imports)]
#[macro_use]
extern crate aleo_std;
#[macro_use]
extern crate thiserror;

pub use snarkvm_utilities::{cfg_chunks, cfg_chunks_mut, cfg_into_iter, cfg_iter, cfg_iter_mut, cfg_reduce};

pub mod crypto_hash;
pub mod fft;
pub mod msm;
pub mod polycommit;
pub mod r1cs;
pub mod snark;

pub mod srs;

pub mod errors;
pub use errors::*;

pub mod traits;
pub use traits::*;

pub mod prelude {
    pub use crate::{errors::*, traits::*};

    pub use crate::{polycommit::error::*, r1cs::errors::*};
}
