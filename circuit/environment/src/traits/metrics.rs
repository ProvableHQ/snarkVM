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

use crate::{Count, Mode};

/// Trait for determining the number of constants, public input, private inputs, and constraints for an operation.
pub trait Metrics<Op: ?Sized> {
    type Case: ?Sized;

    /// Returns the number of constants, public inputs, private inputs, and constraints.
    fn count(parameter: &Self::Case) -> Count;
}

/// Trait for determining the mode of the output of an operation.
pub trait OutputMode<Op: ?Sized> {
    type Case: ?Sized;

    /// Returns the mode of the output.
    fn output_mode(input: &Self::Case) -> Mode;
}
