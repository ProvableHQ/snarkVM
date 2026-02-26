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

use std::{env, fs, path::PathBuf};

fn main() {
    // Only create the CA bundle for mobile targets. The include_bytes! in macros.rs that
    // references cacert.pem is inside #[cfg(any(target_os = "android", target_os = "ios"))],
    // so non-mobile builds never read this file and we skip filesystem work entirely.
    let target = env::var("TARGET").unwrap_or_default();
    if !target.contains("android") && !target.contains("ios") {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let ca_bundle_path = out_dir.join("cacert.pem");
    let platform = if target.contains("android") { "Android" } else { "iOS" };

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:warning=Downloading CA certificate bundle for {platform}...");

    let ca_bundle_url = "https://curl.se/ca/cacert.pem";
    match download_ca_bundle(ca_bundle_url) {
        Ok(contents) => {
            fs::write(&ca_bundle_path, &contents).expect("Failed to write CA certificate bundle");
            println!("cargo:warning=CA certificate bundle downloaded successfully");
        }
        Err(e) => {
            panic!("Failed to download CA certificate bundle: {e}");
        }
    }
}

fn download_ca_bundle(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Use reqwest blocking client for the download
    let client = reqwest::blocking::Client::builder().build()?;
    let response = client.get(url).send()?;
    if response.status().is_success() {
        Ok(response.bytes()?.to_vec())
    } else {
        Err(format!("HTTP error: {}", response.status()).into())
    }
}
