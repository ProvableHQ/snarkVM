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

pub use cast::Cast;
pub use cast_lossy::CastLossy;

mod bytes;
mod cast;
mod cast_lossy;
mod equal;
mod from_bits;
mod parse;
mod sample;
mod serialize;
mod size_in_bits;
mod size_in_bytes;
mod to_bits;
mod to_type;
mod variant;

use crate::{LiteralType, ProgramID};
use snarkvm_console_account::{ComputeKey, PrivateKey, Signature};
use snarkvm_console_network::Network;
use snarkvm_console_types::{prelude::*, Boolean};

/// The literal enum represents all supported types in snarkVM.
#[derive(Clone)]
pub enum Literal<N: Network> {
    /// The Aleo address type.
    Address(Address<N>),
    /// The boolean type.
    Boolean(Boolean<N>),
    /// The field type.
    Field(Field<N>),
    /// The group type.
    Group(Group<N>),
    /// The 8-bit signed integer type.
    I8(I8<N>),
    /// The 16-bit signed integer type.
    I16(I16<N>),
    /// The 32-bit signed integer type.
    I32(I32<N>),
    /// The 64-bit signed integer type.
    I64(I64<N>),
    /// The 128-bit signed integer type.
    I128(I128<N>),
    /// The 8-bit unsigned integer type.
    U8(U8<N>),
    /// The 16-bit unsigned integer type.
    U16(U16<N>),
    /// The 32-bit unsigned integer type.
    U32(U32<N>),
    /// The 64-bit unsigned integer type.
    U64(U64<N>),
    /// The 128-bit unsigned integer type.
    U128(U128<N>),
    /// The scalar type.
    Scalar(Scalar<N>),
    /// The signature type.
    Signature(Box<Signature<N>>),
    /// The string type.
    String(StringType<N>),
}
