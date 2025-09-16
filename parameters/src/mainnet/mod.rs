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

pub mod genesis;
pub use genesis::*;

pub mod powers;
pub use powers::*;

/// The restrictions list as a JSON-compatible string.
pub const RESTRICTIONS_LIST: &str = include_str!("./resources/restrictions.json");

const REMOTE_URL: &str = "https://parameters.provable.com/mainnet";

// Degrees
impl_local!(Degree15, "resources/", "powers-of-beta-15", "usrs");
impl_remote!(Degree16, REMOTE_URL, "resources/", "powers-of-beta-16", "usrs");
impl_remote!(Degree17, REMOTE_URL, "resources/", "powers-of-beta-17", "usrs");
impl_remote!(Degree18, REMOTE_URL, "resources/", "powers-of-beta-18", "usrs");
impl_remote!(Degree19, REMOTE_URL, "resources/", "powers-of-beta-19", "usrs");
impl_remote!(Degree20, REMOTE_URL, "resources/", "powers-of-beta-20", "usrs");
impl_remote!(Degree21, REMOTE_URL, "resources/", "powers-of-beta-21", "usrs");
impl_remote!(Degree22, REMOTE_URL, "resources/", "powers-of-beta-22", "usrs");
impl_remote!(Degree23, REMOTE_URL, "resources/", "powers-of-beta-23", "usrs");
impl_remote!(Degree24, REMOTE_URL, "resources/", "powers-of-beta-24", "usrs");
impl_remote!(Degree25, REMOTE_URL, "resources/", "powers-of-beta-25", "usrs");
// TODO (nkls): restore on CI.
// The SRS is only used for proving and we don't currently support provers of
// this size. When a users wants to create a proof, they load the appropriate
// powers for the circuit in `batch_circuit_setup` which calls `max_degree`
// based on the domain size.
#[cfg(feature = "large_params")]
impl_remote!(Degree26, REMOTE_URL, "resources/", "powers-of-beta-26", "usrs");
#[cfg(feature = "large_params")]
impl_remote!(Degree27, REMOTE_URL, "resources/", "powers-of-beta-27", "usrs");
#[cfg(feature = "large_params")]
impl_remote!(Degree28, REMOTE_URL, "resources/", "powers-of-beta-28", "usrs");

// Shifted Degrees
impl_local!(ShiftedDegree15, "resources/", "shifted-powers-of-beta-15", "usrs");
impl_local!(ShiftedDegree16, "resources/", "shifted-powers-of-beta-16", "usrs");
impl_remote!(ShiftedDegree17, REMOTE_URL, "resources/", "shifted-powers-of-beta-17", "usrs");
impl_remote!(ShiftedDegree18, REMOTE_URL, "resources/", "shifted-powers-of-beta-18", "usrs");
impl_remote!(ShiftedDegree19, REMOTE_URL, "resources/", "shifted-powers-of-beta-19", "usrs");
impl_remote!(ShiftedDegree20, REMOTE_URL, "resources/", "shifted-powers-of-beta-20", "usrs");
impl_remote!(ShiftedDegree21, REMOTE_URL, "resources/", "shifted-powers-of-beta-21", "usrs");
impl_remote!(ShiftedDegree22, REMOTE_URL, "resources/", "shifted-powers-of-beta-22", "usrs");
impl_remote!(ShiftedDegree23, REMOTE_URL, "resources/", "shifted-powers-of-beta-23", "usrs");
impl_remote!(ShiftedDegree24, REMOTE_URL, "resources/", "shifted-powers-of-beta-24", "usrs");
impl_remote!(ShiftedDegree25, REMOTE_URL, "resources/", "shifted-powers-of-beta-25", "usrs");
// TODO (nkls): restore on CI.
// See `Degree28` above for context.
#[cfg(feature = "large_params")]
impl_remote!(ShiftedDegree26, REMOTE_URL, "resources/", "shifted-powers-of-beta-26", "usrs");
#[cfg(feature = "large_params")]
impl_remote!(ShiftedDegree27, REMOTE_URL, "resources/", "shifted-powers-of-beta-27", "usrs");

// Powers of Beta Times Gamma * G
impl_local!(Gamma, "resources/", "powers-of-beta-gamma", "usrs");
// Negative Powers of Beta in G2
impl_local!(NegBeta, "resources/", "neg-powers-of-beta", "usrs");
// Negative Powers of Beta in G2
impl_local!(BetaH, "resources/", "beta-h", "usrs");

// BondPublic
impl_remote!(BondPublicProver, REMOTE_URL, "resources/", "bond_public", "prover", "credits");
impl_local!(BondPublicVerifier, "resources/", "bond_public", "verifier", "credits");
// BondValidator
impl_remote!(BondValidatorProver, REMOTE_URL, "resources/", "bond_validator", "prover", "credits");
impl_local!(BondValidatorVerifier, "resources/", "bond_validator", "verifier", "credits");
// UnbondPublic
impl_remote!(UnbondPublicProver, REMOTE_URL, "resources/", "unbond_public", "prover", "credits");
impl_local!(UnbondPublicVerifier, "resources/", "unbond_public", "verifier", "credits");
// ClaimUnbondPublic
impl_remote!(ClaimUnbondPublicProver, REMOTE_URL, "resources/", "claim_unbond_public", "prover", "credits");
impl_local!(ClaimUnbondPublicVerifier, "resources/", "claim_unbond_public", "verifier", "credits");
// SetValidatorState
impl_remote!(SetValidatorStateProver, REMOTE_URL, "resources/", "set_validator_state", "prover", "credits");
impl_local!(SetValidatorStateVerifier, "resources/", "set_validator_state", "verifier", "credits");
// TransferPrivate
impl_remote!(TransferPrivateProver, REMOTE_URL, "resources/", "transfer_private", "prover", "credits");
impl_local!(TransferPrivateVerifier, "resources/", "transfer_private", "verifier", "credits");
// TransferPublic
impl_remote!(TransferPublicProver, REMOTE_URL, "resources/", "transfer_public", "prover", "credits");
impl_local!(TransferPublicVerifier, "resources/", "transfer_public", "verifier", "credits");
// TransferPublicAsSigner
impl_remote!(TransferPublicAsSignerProver, REMOTE_URL, "resources/", "transfer_public_as_signer", "prover", "credits");
impl_local!(TransferPublicAsSignerVerifier, "resources/", "transfer_public_as_signer", "verifier", "credits");
// TransferPrivateToPublic
impl_remote!(TransferPrivateToPublicProver, REMOTE_URL, "resources/", "transfer_private_to_public", "prover", "credits");
impl_local!(TransferPrivateToPublicVerifier, "resources/", "transfer_private_to_public", "verifier", "credits");
// TransferPublicToPrivate
impl_remote!(TransferPublicToPrivateProver, REMOTE_URL, "resources/", "transfer_public_to_private", "prover", "credits");
impl_local!(TransferPublicToPrivateVerifier, "resources/", "transfer_public_to_private", "verifier", "credits");
// Join
impl_remote!(JoinProver, REMOTE_URL, "resources/", "join", "prover", "credits");
impl_local!(JoinVerifier, "resources/", "join", "verifier", "credits");
// Split
impl_remote!(SplitProver, REMOTE_URL, "resources/", "split", "prover", "credits");
impl_local!(SplitVerifier, "resources/", "split", "verifier", "credits");
// FeePrivate
impl_remote!(FeePrivateProver, REMOTE_URL, "resources/", "fee_private", "prover", "credits");
impl_local!(FeePrivateVerifier, "resources/", "fee_private", "verifier", "credits");
// FeePublic
impl_remote!(FeePublicProver, REMOTE_URL, "resources/", "fee_public", "prover", "credits");
impl_local!(FeePublicVerifier, "resources/", "fee_public", "verifier", "credits");
// Upgrade
impl_remote!(UpgradeProver, REMOTE_URL, "resources/", "upgrade", "prover", "credits");
impl_local!(UpgradeVerifier, "resources/", "upgrade", "verifier", "credits");

// V0 Credits Keys

// BondPublic
impl_remote!(BondPublicV0Prover, REMOTE_URL, "resources/", "bond_public", "prover", "credits_v0");
impl_local!(BondPublicV0Verifier, "resources/", "bond_public", "verifier", "credits_v0");
// BondValidator
impl_remote!(BondValidatorV0Prover, REMOTE_URL, "resources/", "bond_validator", "prover", "credits_v0");
impl_local!(BondValidatorV0Verifier, "resources/", "bond_validator", "verifier", "credits_v0");
// UnbondPublic
impl_remote!(UnbondPublicV0Prover, REMOTE_URL, "resources/", "unbond_public", "prover", "credits_v0");
impl_local!(UnbondPublicV0Verifier, "resources/", "unbond_public", "verifier", "credits_v0");
// ClaimUnbondPublic
impl_remote!(ClaimUnbondPublicV0Prover, REMOTE_URL, "resources/", "claim_unbond_public", "prover", "credits_v0");
impl_local!(ClaimUnbondPublicV0Verifier, "resources/", "claim_unbond_public", "verifier", "credits_v0");
// SetValidatorState
impl_remote!(SetValidatorStateV0Prover, REMOTE_URL, "resources/", "set_validator_state", "prover", "credits_v0");
impl_local!(SetValidatorStateV0Verifier, "resources/", "set_validator_state", "verifier", "credits_v0");
// TransferPrivate
impl_remote!(TransferPrivateV0Prover, REMOTE_URL, "resources/", "transfer_private", "prover", "credits_v0");
impl_local!(TransferPrivateV0Verifier, "resources/", "transfer_private", "verifier", "credits_v0");
// TransferPublic
impl_remote!(TransferPublicV0Prover, REMOTE_URL, "resources/", "transfer_public", "prover", "credits_v0");
impl_local!(TransferPublicV0Verifier, "resources/", "transfer_public", "verifier", "credits_v0");
// TransferPublicAsSigner
impl_remote!(TransferPublicAsSignerV0Prover, REMOTE_URL, "resources/", "transfer_public_as_signer", "prover", "credits_v0");
impl_local!(TransferPublicAsSignerV0Verifier, "resources/", "transfer_public_as_signer", "verifier", "credits_v0");
// TransferPrivateToPublic
impl_remote!(TransferPrivateToPublicV0Prover, REMOTE_URL, "resources/", "transfer_private_to_public", "prover", "credits_v0");
impl_local!(TransferPrivateToPublicV0Verifier, "resources/", "transfer_private_to_public", "verifier", "credits_v0");
// TransferPublicToPrivate
impl_remote!(TransferPublicToPrivateV0Prover, REMOTE_URL, "resources/", "transfer_public_to_private", "prover", "credits_v0");
impl_local!(TransferPublicToPrivateV0Verifier, "resources/", "transfer_public_to_private", "verifier", "credits_v0");
// Join
impl_remote!(JoinV0Prover, REMOTE_URL, "resources/", "join", "prover", "credits_v0");
impl_local!(JoinV0Verifier, "resources/", "join", "verifier", "credits_v0");
// Split
impl_remote!(SplitV0Prover, REMOTE_URL, "resources/", "split", "prover", "credits_v0");
impl_local!(SplitV0Verifier, "resources/", "split", "verifier", "credits_v0");
// FeePrivate
impl_remote!(FeePrivateV0Prover, REMOTE_URL, "resources/", "fee_private", "prover", "credits_v0");
impl_local!(FeePrivateV0Verifier, "resources/", "fee_private", "verifier", "credits_v0");
// FeePublic
impl_remote!(FeePublicV0Prover, REMOTE_URL, "resources/", "fee_public", "prover", "credits_v0");
impl_local!(FeePublicV0Verifier, "resources/", "fee_public", "verifier", "credits_v0");
// Upgrade
impl_remote!(UpgradeV0Prover, REMOTE_URL, "resources/", "upgrade", "prover", "credits_v0");
impl_local!(UpgradeV0Verifier, "resources/", "upgrade", "verifier", "credits_v0");

#[macro_export]
macro_rules! insert_credit_keys {
    ($map:ident, $type:ident<$network:ident>, $variant:ident) => {{
        paste::paste! {
            let string = stringify!([<$variant:lower>]);
            $crate::insert_key!($map, string, $type<$network>, ("bond_public", $crate::mainnet::[<BondPublic $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("bond_validator", $crate::mainnet::[<BondValidator $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("unbond_public", $crate::mainnet::[<UnbondPublic $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("claim_unbond_public", $crate::mainnet::[<ClaimUnbondPublic $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("set_validator_state", $crate::mainnet::[<SetValidatorState $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("transfer_private", $crate::mainnet::[<TransferPrivate $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("transfer_public", $crate::mainnet::[<TransferPublic $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("transfer_public_as_signer", $crate::mainnet::[<TransferPublicAsSigner $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("transfer_private_to_public", $crate::mainnet::[<TransferPrivateToPublic $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("transfer_public_to_private", $crate::mainnet::[<TransferPublicToPrivate $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("join", $crate::mainnet::[<Join $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("split", $crate::mainnet::[<Split $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("fee_private", $crate::mainnet::[<FeePrivate $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("fee_public", $crate::mainnet::[<FeePublic $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("upgrade", $crate::mainnet::[<Upgrade $variant>]::load_bytes()));
        }
    }};
}

#[macro_export]
macro_rules! insert_credit_v0_keys {
    ($map:ident, $type:ident<$network:ident>, $variant:ident) => {{
        paste::paste! {
            let string = stringify!([<$variant:lower>]);
            $crate::insert_key!($map, string, $type<$network>, ("bond_public", $crate::mainnet::[<BondPublicV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("bond_validator", $crate::mainnet::[<BondValidatorV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("unbond_public", $crate::mainnet::[<UnbondPublicV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("claim_unbond_public", $crate::mainnet::[<ClaimUnbondPublicV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("set_validator_state", $crate::mainnet::[<SetValidatorStateV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("transfer_private", $crate::mainnet::[<TransferPrivateV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("transfer_public", $crate::mainnet::[<TransferPublicV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("transfer_public_as_signer", $crate::mainnet::[<TransferPublicAsSignerV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("transfer_private_to_public", $crate::mainnet::[<TransferPrivateToPublicV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("transfer_public_to_private", $crate::mainnet::[<TransferPublicToPrivateV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("join", $crate::mainnet::[<JoinV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("split", $crate::mainnet::[<SplitV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("fee_private", $crate::mainnet::[<FeePrivateV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("fee_public", $crate::mainnet::[<FeePublicV0 $variant>]::load_bytes()));
            $crate::insert_key!($map, string, $type<$network>, ("upgrade", $crate::mainnet::[<UpgradeV0 $variant>]::load_bytes()));
        }
    }};
}

#[macro_export]
macro_rules! insert_key {
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
impl_remote!(InclusionV0Prover, REMOTE_URL, "resources/", "inclusion", "prover", "credits_v0");
impl_local!(InclusionV0Verifier, "resources/", "inclusion", "verifier", "credits_v0");
impl_remote!(InclusionProver, REMOTE_URL, "resources/", "inclusion", "prover", "credits");
impl_local!(InclusionVerifier, "resources/", "inclusion", "verifier", "credits");

/// The function name for the inclusion circuit.
pub const NETWORK_INCLUSION_FUNCTION_NAME: &str = "inclusion";

lazy_static! {
    pub static ref INCLUSION_V0_PROVING_KEY: Vec<u8> =
        InclusionV0Prover::load_bytes().expect("Failed to load inclusion_v0 proving key");
    pub static ref INCLUSION_V0_VERIFYING_KEY: Vec<u8> =
        InclusionV0Verifier::load_bytes().expect("Failed to load inclusion_v0 verifying key");
    pub static ref INCLUSION_PROVING_KEY: Vec<u8> = InclusionProver::load_bytes().expect("Failed to load inclusion proving key");
    pub static ref INCLUSION_VERIFYING_KEY: Vec<u8> =
        InclusionVerifier::load_bytes().expect("Failed to load inclusion verifying key");
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    wasm_bindgen_test_configure!(run_in_browser);

    #[ignore]
    #[test]
    fn test_load_bytes_mini() {
        Degree16::load_bytes().expect("Failed to load degree 16");
        BondPublicVerifier::load_bytes().expect("Failed to load bond_public verifier");
        FeePublicProver::load_bytes().expect("Failed to load fee_public prover");
        FeePublicVerifier::load_bytes().expect("Failed to load fee_public verifier");
        InclusionProver::load_bytes().expect("Failed to load inclusion prover");
        InclusionVerifier::load_bytes().expect("Failed to load inclusion verifier");
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_load_bytes() {
        Degree16::load_bytes().expect("Failed to load degree 16");
        Degree17::load_bytes().expect("Failed to load degree 17");
        Degree18::load_bytes().expect("Failed to load degree 18");
        Degree19::load_bytes().expect("Failed to load degree 19");
        Degree20::load_bytes().expect("Failed to load degree 20");
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
        UpgradeProver::load_bytes().expect("Failed to load upgrade prover");
        UpgradeVerifier::load_bytes().expect("Failed to load upgrade verifier");
        InclusionProver::load_bytes().expect("Failed to load inclusion prover");
        InclusionVerifier::load_bytes().expect("Failed to load inclusion verifier");
    }
}
