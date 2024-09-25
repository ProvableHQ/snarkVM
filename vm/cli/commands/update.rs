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

use super::*;
use crate::cli::helpers::Updater;

/// Update SnarkVM to the latest version
#[derive(Debug, Parser)]
pub struct Update {
    /// Lists all available versions of Aleo
    #[clap(short = 'l', long)]
    list: bool,
    /// Suppress outputs to terminal
    #[clap(short = 'q', long)]
    quiet: bool,
}

impl Update {
    pub fn parse(self) -> Result<String> {
        match self.list {
            true => match Updater::show_available_releases() {
                Ok(output) => Ok(output),
                Err(error) => Ok(format!("Failed to list the available versions of Aleo\n{error}\n")),
            },
            false => {
                let result = Updater::update_to_latest_release(!self.quiet);
                if !self.quiet {
                    match result {
                        Ok(status) => {
                            if status.uptodate() {
                                Ok("\nAleo is already on the latest version".to_string())
                            } else if status.updated() {
                                Ok(format!("\nAleo has updated to version {}", status.version()))
                            } else {
                                Ok(String::new())
                            }
                        }
                        Err(e) => Ok(format!("\nFailed to update Aleo to the latest version\n{e}\n")),
                    }
                } else {
                    Ok(String::new())
                }
            }
        }
    }
}
