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

pub mod genesis;
pub use genesis::*;

/// The restrictions list as a JSON-compatible string.
pub const RESTRICTIONS_LIST: &str = include_str!("./resources/restrictions.json");

const REMOTE_URL: &str = "https://parameters.aleo.org/canary";

// BondPublic
impl_remote!(BondPublicProver, REMOTE_URL, "resources/", "bond_public", "prover");
impl_local!(BondPublicVerifier, "resources/", "bond_public", "verifier");
// BondValidator
impl_remote!(BondValidatorProver, REMOTE_URL, "resources/", "bond_validator", "prover");
impl_local!(BondValidatorVerifier, "resources/", "bond_validator", "verifier");
// UnbondPublic
impl_remote!(UnbondPublicProver, REMOTE_URL, "resources/", "unbond_public", "prover");
impl_local!(UnbondPublicVerifier, "resources/", "unbond_public", "verifier");
// ClaimUnbondPublic
impl_remote!(ClaimUnbondPublicProver, REMOTE_URL, "resources/", "claim_unbond_public", "prover");
impl_local!(ClaimUnbondPublicVerifier, "resources/", "claim_unbond_public", "verifier");
// SetValidatorState
impl_remote!(SetValidatorStateProver, REMOTE_URL, "resources/", "set_validator_state", "prover");
impl_local!(SetValidatorStateVerifier, "resources/", "set_validator_state", "verifier");
// TransferPrivate
impl_remote!(TransferPrivateProver, REMOTE_URL, "resources/", "transfer_private", "prover");
impl_local!(TransferPrivateVerifier, "resources/", "transfer_private", "verifier");
// TransferPublic
impl_remote!(TransferPublicProver, REMOTE_URL, "resources/", "transfer_public", "prover");
impl_local!(TransferPublicVerifier, "resources/", "transfer_public", "verifier");
// TransferPublicAsSigner
impl_remote!(TransferPublicAsSignerProver, REMOTE_URL, "resources/", "transfer_public_as_signer", "prover");
impl_local!(TransferPublicAsSignerVerifier, "resources/", "transfer_public_as_signer", "verifier");
// TransferPrivateToPublic
impl_remote!(TransferPrivateToPublicProver, REMOTE_URL, "resources/", "transfer_private_to_public", "prover");
impl_local!(TransferPrivateToPublicVerifier, "resources/", "transfer_private_to_public", "verifier");
// TransferPublicToPrivate
impl_remote!(TransferPublicToPrivateProver, REMOTE_URL, "resources/", "transfer_public_to_private", "prover");
impl_local!(TransferPublicToPrivateVerifier, "resources/", "transfer_public_to_private", "verifier");
// Join
impl_remote!(JoinProver, REMOTE_URL, "resources/", "join", "prover");
impl_local!(JoinVerifier, "resources/", "join", "verifier");
// Split
impl_remote!(SplitProver, REMOTE_URL, "resources/", "split", "prover");
impl_local!(SplitVerifier, "resources/", "split", "verifier");
// FeePrivate
impl_remote!(FeePrivateProver, REMOTE_URL, "resources/", "fee_private", "prover");
impl_local!(FeePrivateVerifier, "resources/", "fee_private", "verifier");
// FeePublic
impl_remote!(FeePublicProver, REMOTE_URL, "resources/", "fee_public", "prover");
impl_local!(FeePublicVerifier, "resources/", "fee_public", "verifier");

#[macro_export]
macro_rules! insert_canary_credit_keys {
    ($map:ident, $type:ident<$network:ident>, $variant:ident) => {{
        paste::paste! {
            let string = stringify!([<$variant:lower>]);
            $crate::insert_canary_key!($map, string, $type<$network>, ("bond_public", $crate::canary::[<BondPublic $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("bond_validator", $crate::canary::[<BondValidator $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("unbond_public", $crate::canary::[<UnbondPublic $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("claim_unbond_public", $crate::canary::[<ClaimUnbondPublic $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("set_validator_state", $crate::canary::[<SetValidatorState $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("transfer_private", $crate::canary::[<TransferPrivate $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("transfer_public", $crate::canary::[<TransferPublic $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("transfer_public_as_signer", $crate::canary::[<TransferPublicAsSigner $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("transfer_private_to_public", $crate::canary::[<TransferPrivateToPublic $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("transfer_public_to_private", $crate::canary::[<TransferPublicToPrivate $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("join", $crate::canary::[<Join $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("split", $crate::canary::[<Split $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("fee_private", $crate::canary::[<FeePrivate $variant>]::load_bytes()));
            $crate::insert_canary_key!($map, string, $type<$network>, ("fee_public", $crate::canary::[<FeePublic $variant>]::load_bytes()));
        }
    }};
}

#[macro_export]
macro_rules! insert_canary_key {
    ($map:ident, $string:tt, $type:ident<$network:ident>, ($name:tt, $circuit_key:expr)) => {{
        // Load the circuit key bytes.
        let key_bytes: Vec<u8> = $circuit_key.expect(&format!("Failed to load {} bytes", $string));
        // Recover the circuit key.
        let key = $type::<$network>::from_bytes_le(&key_bytes[1..]).expect(&format!("Failed to recover {}", $string));
        // Insert the circuit key.
        $map.insert($name.to_string(), std::sync::Arc::new(key));
    }};
}

// Inclusion
impl_remote!(InclusionProver, REMOTE_URL, "resources/", "inclusion", "prover");
impl_local!(InclusionVerifier, "resources/", "inclusion", "verifier");

/// The function name for the inclusion circuit.
pub const NETWORK_INCLUSION_FUNCTION_NAME: &str = "inclusion";

lazy_static! {
    pub static ref INCLUSION_PROVING_KEY: Vec<u8> =
        InclusionProver::load_bytes().expect("Failed to load inclusion proving key");
    pub static ref INCLUSION_VERIFYING_KEY: Vec<u8> =
        InclusionVerifier::load_bytes().expect("Failed to load inclusion verifying key");
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_load_bytes() {
        BondPublicVerifier::load_bytes().expect("Failed to load bond_public verifier");
        BondValidatorVerifier::load_bytes().expect("Failed to load bond_validator verifier");
        UnbondPublicVerifier::load_bytes().expect("Failed to load unbond_public verifier");
        ClaimUnbondPublicVerifier::load_bytes().expect("Failed to load claim_unbond_public verifier");
        SetValidatorStateVerifier::load_bytes().expect("Failed to load set_validator_state verifier");
        TransferPrivateVerifier::load_bytes().expect("Failed to load transfer_private verifier");
        TransferPublicVerifier::load_bytes().expect("Failed to load transfer_public verifier");
        TransferPublicAsSignerVerifier::load_bytes().expect("Failed to load transfer_public_as_signer verifier");
        TransferPrivateToPublicVerifier::load_bytes().expect("Failed to load transfer_private_to_public verifier");
        TransferPublicToPrivateVerifier::load_bytes().expect("Failed to load transfer_public_to_private verifier");
        FeePrivateProver::load_bytes().expect("Failed to load fee_private prover");
        FeePrivateVerifier::load_bytes().expect("Failed to load fee_private verifier");
        FeePublicProver::load_bytes().expect("Failed to load fee_public prover");
        FeePublicVerifier::load_bytes().expect("Failed to load fee_public verifier");
        InclusionProver::load_bytes().expect("Failed to load inclusion prover");
        InclusionVerifier::load_bytes().expect("Failed to load inclusion verifier");
    }
}
