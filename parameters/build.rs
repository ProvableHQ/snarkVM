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

use std::{env, fs, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let ca_bundle_path = out_dir.join("cacert.pem");

    // Download CA bundle for mobile platforms (Android and iOS)
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("android") || target.contains("ios") {
        let platform = if target.contains("android") { "Android" } else { "iOS" };
        // Download Mozilla's CA certificate bundle
        println!("cargo:warning=Downloading CA certificate bundle for {platform}...");

        let ca_bundle_url = "https://curl.se/ca/cacert.pem";
        match download_ca_bundle(ca_bundle_url) {
            Ok(contents) => {
                // Write the CA bundle as a binary file that can be included with include_bytes!
                // This is simpler and more efficient than generating a Rust array
                fs::write(&ca_bundle_path, &contents).expect("Failed to write CA certificate bundle");

                println!("cargo:warning=CA certificate bundle downloaded successfully");
                println!("cargo:rerun-if-changed=build.rs");
            }
            Err(e) => {
                panic!(
                    "Failed to download CA certificate bundle: {e}\n\
                     You may need to download it manually from {ca_bundle_url} and place it at {ca_bundle_path:?}"
                );
            }
        }
    } else {
        // For non-mobile targets, write an empty file
        fs::write(&ca_bundle_path, b"").expect("Failed to write empty CA bundle");
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
