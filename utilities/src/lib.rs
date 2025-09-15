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

#[macro_use]
extern crate thiserror;

pub mod biginteger;
pub use biginteger::*;

pub mod bititerator;
pub use bititerator::*;

#[macro_use]
pub mod bits;
pub use bits::*;

#[macro_use]
pub mod bytes;
pub use bytes::*;

#[macro_use]
pub mod defer;
pub use defer::*;

pub mod iterator;
pub use iterator::*;

#[macro_use]
pub mod parallel;
pub use parallel::*;

#[macro_use]
mod print;

pub mod rand;
pub use self::rand::*;

/// Helpers for data (de-)serialization.
pub mod serialize;
pub use serialize::*;

/// Helpers for error handling.
pub mod errors;
pub use errors::*;

#[cfg(feature = "async")]
/// Helpers to spawn async tasks.
pub mod task;
#[cfg(feature = "async")]
pub use task::*;

/// Use old name for backward-compatibility.
pub use errors::io_error as error;

/// This macro provides a VM runtime environment which will safely halt
/// without producing logs that look like unexpected behavior.
/// In debug mode, it prints to stderr using the format: "VM safely halted at {location}: {halt message}".
///
/// It is more efficient to set the panic hook once and directly use `errors::try_vm_runtime`.
#[macro_export]
macro_rules! try_vm_runtime {
    ($e:expr) => {{
        $crate::errors::set_panic_hook();
        $crate::errors::try_vm_runtime($e)
    }};
}
