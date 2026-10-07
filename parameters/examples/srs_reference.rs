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

//! Synthesizes the 'credits.aleo' proving keys, writes each as a reference to
//! the universal SRS, reads it back with the SRS, and checks that it is the
//! shipped key.
//!
//! `cargo run --release --example srs_reference -- [--out <dir>] [function names...]`
//!
//! With no function names, every 'credits.aleo' function is checked. With
//! `--out`, each checked key is written to `<dir>/<function>.prover` as a
//! reference, which `ProvingKey::read_le_with_srs` reads.

use snarkvm_algorithms::crypto_hash::sha256::sha256;
use snarkvm_circuit::AleoV0;
use snarkvm_console::network::{MainnetV0, Network, prelude::ToBytes};
use snarkvm_synthesizer::{Process, Program, snark::ProvingKey};

use anyhow::{Context, Result, ensure};
use std::{fs, path::PathBuf, time::Instant};

type N = MainnetV0;

pub fn main() -> Result<()> {
    let mut out_dir = None;
    let mut requested = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out_dir = Some(PathBuf::from(args.next().context("'--out' needs a directory")?)),
            _ => requested.push(arg),
        }
    }
    if let Some(out_dir) = &out_dir {
        fs::create_dir_all(out_dir)?;
    }

    // Synthesizing a key trims its committer key from the process's SRS, so the key shares it.
    let rng = &mut snarkvm_utilities::TestRng::fixed(1245897092);
    let process = Process::<N>::setup::<AleoV0, _>(rng)?;
    let program = Program::<N>::credits()?;
    let program_id = program.id();

    println!("{:<28} {:>12} {:>12} {:>8} {:>10}", "function", "full", "reference", "ratio", "read");
    for function_name in program.functions().keys() {
        if !requested.is_empty() && !requested.contains(&function_name.to_string()) {
            continue;
        }

        let synthesized = process.get_proving_key(program_id, function_name)?;
        let reference = synthesized.to_bytes_le()?;

        let timer = Instant::now();
        let read = ProvingKey::<N>::read_le_with_srs(&reference[..], process.universal_srs())?;
        let elapsed = timer.elapsed();

        let shipped = ProvingKey::<N>::new(N::get_credits_proving_key(function_name.to_string())?.clone());
        let full = shipped.to_bytes_le_full()?;
        ensure!(read.to_bytes_le_full()? == full, "'{function_name}' does not read back as the shipped key");

        println!(
            "{:<28} {:>12} {:>12} {:>7.1}% {:>10.1?}",
            function_name.to_string(),
            full.len(),
            reference.len(),
            100.0 * reference.len() as f64 / full.len() as f64,
            elapsed,
        );

        if let Some(out_dir) = &out_dir {
            let path = out_dir.join(format!("{function_name}.prover"));
            fs::write(&path, &reference)?;
            println!("  wrote {} (sha256 {})", path.display(), hex::encode(sha256(&reference)));
        }
    }
    Ok(())
}
