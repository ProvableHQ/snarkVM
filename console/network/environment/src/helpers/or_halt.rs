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

use crate::Environment;

/// A trait to unwrap a `Result` or `Halt`.
pub trait OrHalt<T> {
    /// Returns the result if it is successful, otherwise halt.
    fn or_halt<E: Environment>(self) -> T;

    /// Returns the result if it is successful, otherwise halts with the message.
    fn or_halt_with<E: Environment>(self, msg: &str) -> T;
}

impl<T, Error: core::fmt::Display> OrHalt<T> for Result<T, Error> {
    /// Returns the result if it is successful, otherwise halt.
    fn or_halt<E: Environment>(self) -> T {
        match self {
            Ok(result) => result,
            Err(error) => E::halt(error.to_string()),
        }
    }

    /// Returns the result if it is successful, otherwise halts with the message.
    fn or_halt_with<E: Environment>(self, msg: &str) -> T {
        match self {
            Ok(result) => result,
            Err(error) => E::halt(format!("{msg}: {error}")),
        }
    }
}
