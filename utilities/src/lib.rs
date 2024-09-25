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

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(test, not(feature = "std")))]
#[macro_use]
extern crate std;

#[cfg(not(feature = "std"))]
pub use alloc::*;

#[cfg(not(feature = "std"))]
pub use core::*;

#[cfg(feature = "std")]
#[doc(hidden)]
pub use std::*;

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
#[doc(hidden)]
pub use alloc::{boxed::Box, vec::Vec};

#[cfg(feature = "std")]
#[allow(unused_imports)]
#[doc(hidden)]
pub use std::{boxed::Box, vec::Vec};

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

pub mod error;
pub use error::*;

pub mod iterator;
pub use iterator::*;

#[macro_use]
pub mod parallel;
pub use parallel::*;

pub mod rand;
pub use self::rand::*;

pub mod serialize;
pub use serialize::*;

#[cfg(not(feature = "std"))]
pub mod io;

#[cfg(not(feature = "std"))]
pub fn error(_msg: &'static str) -> io::Error {
    io::Error
}

#[cfg(feature = "std")]
pub fn error<S: ToString>(msg: S) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg.to_string())
}
