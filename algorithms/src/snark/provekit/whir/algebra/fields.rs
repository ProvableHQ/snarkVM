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

// Originally derived from WHIR (https://github.com/WizardOfMenlo/whir),
// licensed under Apache-2.0 OR MIT.

//! Field metadata used by WHIR configuration and serialization.

use serde::{Deserialize, Serialize};
use snarkvm_fields::{Field, PrimeField};

use crate::snark::provekit::whir::type_info::TypeInfo;

/// Bit-size of a field, used in WHIR soundness calculations.
pub trait FieldWithSize {
    fn field_size_bits() -> f64;
}

impl<F: Field> FieldWithSize for F {
    fn field_size_bits() -> f64 {
        // `2^64` as `f64`, matching the original WHIR limb accumulation.
        const BASE264: f64 = 18_446_744_073_709_551_616_f64;
        let modulus = F::BasePrimeField::modulus();
        let limbs = modulus.as_ref();
        let mut modulus = 0.0_f64;
        for limb in limbs.iter().rev() {
            modulus *= BASE264;
            modulus += *limb as f64;
        }
        modulus.log2()
    }
}

/// Type information for a finite field.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldInfo {
    /// Field characteristic in big-endian without leading zeros.
    #[serde(with = "crate::snark::provekit::whir::ark_serde::bytes")]
    characteristic: Vec<u8>,

    /// Extension degree of the field. Prime fields are degree 1.
    extension_degree: usize,
}

impl<F: Field> TypeInfo for F {
    type Info = FieldInfo;

    fn type_info() -> Self::Info {
        let mut le_bytes = Vec::with_capacity(F::characteristic().len() * 8);
        for limb in F::characteristic() {
            le_bytes.extend_from_slice(&limb.to_le_bytes());
        }
        let characteristic = le_bytes.iter().copied().rev().skip_while(|&b| b == 0).collect();
        FieldInfo { characteristic, extension_degree: 1 }
    }
}
