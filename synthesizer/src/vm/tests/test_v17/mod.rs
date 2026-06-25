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

// Tests for block-wide synthesis limits.
mod blockwide_synthesis_limit;

use super::*;

use std::{collections::HashSet, sync::Arc};

use crate::vm::test_helpers::*;

use console::{
    account::{Address, PrivateKey},
    network::ConsensusVersion,
    prelude::FromStr,
    program::Value,
};

use snarkvm_ledger_block::{Deployment, Solutions, Transaction};
use snarkvm_ledger_narwhal_subdag::test_helpers::subdag_with_cert_count;
use snarkvm_synthesizer_program::{FinalizeGlobalState, Program};
use snarkvm_synthesizer_snark::VerifyingKey;
use snarkvm_utilities::{TestRng, try_vm_runtime};

use super::test_v14::add_and_test_with_costs;
