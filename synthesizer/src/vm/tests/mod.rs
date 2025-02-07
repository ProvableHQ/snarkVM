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

mod test_update;
mod test_vm;

use super::*;

use crate::vm::test_helpers::*;

use console::{
    account::{Address, ViewKey},
    network::MainnetV0,
    program::{Entry, Value},
};
use ledger_block::Transition;
use ledger_test_helpers::{large_transaction_program, small_transaction_program};
use synthesizer_program::Program;

use indexmap::IndexMap;
use synthesizer_snark::VerifyingKey;
