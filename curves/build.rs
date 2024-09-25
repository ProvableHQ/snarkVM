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

// Detect the rustc channel
use rustc_version::{version_meta, Channel};

fn main() {
    // Set cfg flags depending on release channel
    match version_meta().unwrap().channel {
        Channel::Stable => println!("cargo:rustc-cfg=stable"),
        Channel::Beta => println!("cargo:rustc-cfg=beta"),
        Channel::Nightly => println!("cargo:rustc-cfg=nightly"),
        Channel::Dev => println!("cargo:rustc-cfg=rustc_dev"),
    }
}
