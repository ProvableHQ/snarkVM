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

use crate::cli::errors::UpdaterError;

use anyhow::Result;
use colored::Colorize;
use self_update::{backends::github, version::bump_is_greater, Status};

pub struct Updater;

// TODO Add logic for users to easily select release versions.
impl Updater {
    const SNARKVM_BIN_NAME: &'static str = "snarkvm";
    const SNARKVM_REPO_NAME: &'static str = "snarkvm";
    const SNARKVM_REPO_OWNER: &'static str = "AleoNet";

    /// Show all available releases for `snarkvm`.
    #[allow(clippy::format_push_string)]
    pub fn show_available_releases() -> Result<String> {
        let releases = github::ReleaseList::configure()
            .repo_owner(Self::SNARKVM_REPO_OWNER)
            .repo_name(Self::SNARKVM_REPO_NAME)
            .build()?
            .fetch()?;

        let mut output = "List of available versions\n".to_string();
        for release in releases {
            output += &format!("  * {}\n", release.version);
        }
        Ok(output)
    }

    /// Update `snarkvm` to the latest release.
    pub fn update_to_latest_release(show_output: bool) -> Result<Status> {
        let status = github::Update::configure()
            .repo_owner(Self::SNARKVM_REPO_OWNER)
            .repo_name(Self::SNARKVM_REPO_NAME)
            .bin_name(Self::SNARKVM_BIN_NAME)
            .current_version(env!("CARGO_PKG_VERSION"))
            .show_download_progress(show_output)
            .no_confirm(true)
            .show_output(show_output)
            .build()?
            .update()?;

        Ok(status)
    }

    /// Check if there is an available update for `snarkvm` and return the newest release.
    pub fn update_available() -> Result<String, UpdaterError> {
        let updater = github::Update::configure()
            .repo_owner(Self::SNARKVM_REPO_OWNER)
            .repo_name(Self::SNARKVM_REPO_NAME)
            .bin_name(Self::SNARKVM_BIN_NAME)
            .current_version(env!("CARGO_PKG_VERSION"))
            .build()?;

        let current_version = updater.current_version();
        let latest_release = updater.get_latest_release()?;

        if bump_is_greater(&current_version, &latest_release.version)? {
            Ok(latest_release.version)
        } else {
            Err(UpdaterError::OldReleaseVersion(current_version, latest_release.version))
        }
    }

    /// Display the CLI message.
    pub fn print_cli() -> String {
        if let Ok(latest_version) = Self::update_available() {
            let mut output = "🟢 A new version is available! Run".bold().green().to_string();
            output += &" `aleo update` ".bold().white();
            output += &format!("to update to v{latest_version}.").bold().green();
            output
        } else {
            "".to_string()
        }
    }
}
