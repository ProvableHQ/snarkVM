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

use crate::{crypto_hash::PoseidonSponge, AlgebraicSponge, DuplexSpongeMode};
use snarkvm_curves::bls12_377::Fr;
use snarkvm_fields::{PoseidonDefaultField, PoseidonGrainLFSR};

use anyhow::Result;
use itertools::Itertools;
use std::{path::PathBuf, sync::Arc};

#[track_caller]
fn expect_file_with_name(name: impl ToString, val: impl std::fmt::Debug) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR").to_string());
    path.push("src");
    path.push("crypto_hash");
    path.push("resources");
    path.push("poseidon");
    if !path.exists() {
        std::fs::create_dir_all(&path).expect("failed to create directory");
    }
    let file_name: String = module_path!().split("::").chain([name.to_string().as_ref()]).join("_");
    path.push(file_name);
    path.set_extension("snap");
    if !path.exists() {
        std::fs::File::create(&path).expect("failed to create file");
    }
    expect_test::expect_file![path].assert_eq(&format!("{val:?}"));
}

#[test]
fn test_grain_lfsr_consistency() -> Result<()> {
    let mut lfsr = PoseidonGrainLFSR::new(false, 253, 3, 8, 31);
    expect_file_with_name("first sample", lfsr.get_field_elements_rejection_sampling::<Fr>(1)?);
    expect_file_with_name("second sample", lfsr.get_field_elements_rejection_sampling::<Fr>(1)?);
    Ok(())
}

#[test]
fn test_poseidon_sponge_consistency() {
    const RATE: usize = 2;
    let sponge_param = Arc::new(Fr::default_poseidon_parameters::<RATE>().unwrap());
    for absorb in 0..10 {
        for squeeze in 0..10 {
            let iteration_name = format!("Absorb {absorb} and Squeeze {squeeze}");
            let mut sponge = PoseidonSponge::<Fr, RATE, 1>::new_with_parameters(&sponge_param);
            sponge.absorb_native_field_elements(&vec![Fr::from(1237812u64); absorb]);
            let next_absorb_index = if absorb % RATE != 0 || absorb == 0 { absorb % RATE } else { RATE };
            assert_eq!(sponge.mode, DuplexSpongeMode::Absorbing { next_absorb_index }, "{iteration_name}");
            expect_file_with_name(&iteration_name, sponge.squeeze_native_field_elements(squeeze));
            let next_squeeze_index = if squeeze % RATE != 0 || squeeze == 0 { squeeze % RATE } else { RATE };
            if squeeze == 0 {
                assert_eq!(sponge.mode, DuplexSpongeMode::Absorbing { next_absorb_index }, "{iteration_name}");
            } else {
                assert_eq!(sponge.mode, DuplexSpongeMode::Squeezing { next_squeeze_index }, "{iteration_name}");
            }
        }
    }
}

#[test]
fn bls12_377_fr_poseidon_default_parameters_test() {
    fn single_rate_test<const RATE: usize>() {
        let params = Fr::default_poseidon_parameters::<RATE>().unwrap();
        let name = format!("rate {RATE} and optimize_for_weights false");
        expect_file_with_name("Ark for ".to_string() + &name, params.ark);
        expect_file_with_name("MDS for ".to_string() + &name, params.mds);
    }
    // Optimize for constraints
    single_rate_test::<2>();
    single_rate_test::<3>();
    single_rate_test::<4>();
    single_rate_test::<5>();
    single_rate_test::<6>();
    single_rate_test::<7>();
    single_rate_test::<8>();
}
