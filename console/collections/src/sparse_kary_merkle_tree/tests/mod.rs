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

use super::*;

mod helpers;
use helpers::*;

mod insert;
mod insert_many;
mod update;
mod update_many;
mod remove;
mod prove;

macro_rules! run_tests {
    ($rng:expr, [$($i:expr),*]) => {
        $( assert!(run_test::<$i>($rng).is_ok()); )*
    };
}
use run_tests;

