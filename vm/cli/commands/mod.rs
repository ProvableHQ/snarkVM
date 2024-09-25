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

pub mod build;
pub use build::*;

pub mod clean;
pub use clean::*;

pub mod execute;
pub use execute::*;

pub mod new;
pub use new::*;

pub mod run;
pub use run::*;

pub mod update;
pub use update::*;

use crate::{
    console::program::{Identifier, Locator, ProgramID, Value},
    ledger::block::Transaction,
    package::Package,
};

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use core::str::FromStr;
use std::collections::HashMap;

pub const LOCALE: &num_format::Locale = &num_format::Locale::en;

pub(crate) type CurrentNetwork = crate::prelude::MainnetV0;
pub(crate) type Aleo = crate::circuit::AleoV0;
