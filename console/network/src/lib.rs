// Copyright 2024-2025 Aleo Network Foundation
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

#![forbid(unsafe_code)]
#![allow(clippy::too_many_arguments)]
#![warn(clippy::cast_possible_truncation)]

#[macro_use]
extern crate lazy_static;

pub use snarkvm_console_network_environment as environment;
pub use snarkvm_console_network_environment::*;

mod helpers;
pub use helpers::*;

mod canary_v0;
pub use canary_v0::*;

mod mainnet_v0;
pub use mainnet_v0::*;

mod testnet_v0;
pub use testnet_v0::*;

pub mod prelude {
    pub use crate::{ConsensusVersion, Network, consensus_config_value, environment::prelude::*};
}

use crate::environment::prelude::*;
use snarkvm_algorithms::{
    AlgebraicSponge,
    crypto_hash::PoseidonSponge,
    snark::varuna::{CircuitProvingKey, CircuitVerifyingKey, VarunaHidingMode},
    srs::{UniversalProver, UniversalVerifier},
};
use snarkvm_console_algorithms::{BHP512, BHP1024, Poseidon2, Poseidon4};
use snarkvm_console_collections::merkle_tree::{MerklePath, MerkleTree};
use snarkvm_console_types::{Field, Group, Scalar};
use snarkvm_curves::PairingEngine;

use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use std::sync::Arc;

/// A helper type for the BHP Merkle tree.
pub type BHPMerkleTree<N, const DEPTH: u8> = MerkleTree<N, BHP1024<N>, BHP512<N>, DEPTH>;
/// A helper type for the Poseidon Merkle tree.
pub type PoseidonMerkleTree<N, const DEPTH: u8> = MerkleTree<N, Poseidon4<N>, Poseidon2<N>, DEPTH>;

/// Helper types for the Varuna parameters.
type Fq<N> = <<N as Environment>::PairingCurve as PairingEngine>::Fq;
pub type FiatShamir<N> = PoseidonSponge<Fq<N>, 2, 1>;
pub type FiatShamirParameters<N> = <FiatShamir<N> as AlgebraicSponge<Fq<N>, 2>>::Parameters;

/// Helper types for the Varuna proving and verifying key.
pub(crate) type VarunaProvingKey<N> = CircuitProvingKey<<N as Environment>::PairingCurve, VarunaHidingMode>;
pub(crate) type VarunaVerifyingKey<N> = CircuitVerifyingKey<<N as Environment>::PairingCurve>;

/// The different consensus versions.
/// Documentation for what is changed at each version can be found in `N::CONSENSUS_VERSION`
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub enum ConsensusVersion {
    V1 = 1,
    V2 = 2,
    V3 = 3,
}

pub trait Network:
    'static
    + Environment
    + Copy
    + Clone
    + Debug
    + Eq
    + PartialEq
    + core::hash::Hash
    + Serialize
    + DeserializeOwned
    + for<'a> Deserialize<'a>
    + Send
    + Sync
{
    /// The network ID.
    const ID: u16;
    /// The network name.
    const NAME: &'static str;
    /// The network edition.
    const EDITION: u16;

    /// The function name for the inclusion circuit.
    const INCLUSION_FUNCTION_NAME: &'static str;

    /// The fixed timestamp of the genesis block.
    const GENESIS_TIMESTAMP: i64;
    /// The genesis block coinbase target.
    const GENESIS_COINBASE_TARGET: u64;
    /// The genesis block proof target.
    const GENESIS_PROOF_TARGET: u64;
    /// The maximum number of solutions that can be included per block as a power of 2.
    const MAX_SOLUTIONS_AS_POWER_OF_TWO: u8 = 2; // 4 solutions
    /// The maximum number of solutions that can be included per block.
    const MAX_SOLUTIONS: usize = 1 << Self::MAX_SOLUTIONS_AS_POWER_OF_TWO; // 4 solutions

    /// The starting supply of Aleo credits.
    const STARTING_SUPPLY: u64 = 1_500_000_000_000_000; // 1.5B credits
    /// The cost in microcredits per byte for the deployment transaction.
    const DEPLOYMENT_FEE_MULTIPLIER: u64 = 1_000; // 1 millicredit per byte
    /// The constant that divides the storage polynomial.
    const EXECUTION_STORAGE_FEE_SCALING_FACTOR: u64 = 5000;
    /// The maximum size execution transactions can be before a quadratic storage penalty applies.
    const EXECUTION_STORAGE_PENALTY_THRESHOLD: u64 = 5000;
    /// The cost in microcredits per constraint for the deployment transaction.
    const SYNTHESIS_FEE_MULTIPLIER: u64 = 25; // 25 microcredits per constraint
    /// The maximum number of variables in a deployment.
    const MAX_DEPLOYMENT_VARIABLES: u64 = 1 << 20; // 1,048,576 variables
    /// The maximum number of constraints in a deployment.
    const MAX_DEPLOYMENT_CONSTRAINTS: u64 = 1 << 20; // 1,048,576 constraints
    /// The maximum number of microcredits that can be spent as a fee.
    const MAX_FEE: u64 = 1_000_000_000_000_000;
    /// The maximum number of microcredits that can be spent on a finalize block.
    const TRANSACTION_SPEND_LIMIT: u64 = 100_000_000;

    /// The anchor height, defined as the expected number of blocks to reach the coinbase target.
    const ANCHOR_HEIGHT: u32 = Self::ANCHOR_TIME as u32 / Self::BLOCK_TIME as u32;
    /// The anchor time in seconds.
    const ANCHOR_TIME: u16 = 25;
    /// The expected time per block in seconds.
    const BLOCK_TIME: u16 = 10;
    /// The number of blocks per epoch.
    const NUM_BLOCKS_PER_EPOCH: u32 = 3600 / Self::BLOCK_TIME as u32; // 360 blocks == ~1 hour

    /// The maximum number of entries in data.
    const MAX_DATA_ENTRIES: usize = 32;
    /// The maximum recursive depth of an entry.
    /// Note: This value must be strictly less than u8::MAX.
    const MAX_DATA_DEPTH: usize = 32;
    /// The maximum number of fields in data (must not exceed u16::MAX).
    #[allow(clippy::cast_possible_truncation)]
    const MAX_DATA_SIZE_IN_FIELDS: u32 = ((128 * 1024 * 8) / Field::<Self>::SIZE_IN_DATA_BITS) as u32;

    /// The minimum number of entries in a struct.
    const MIN_STRUCT_ENTRIES: usize = 1; // This ensures the struct is not empty.
    /// The maximum number of entries in a struct.
    const MAX_STRUCT_ENTRIES: usize = Self::MAX_DATA_ENTRIES;

    /// The minimum number of elements in an array.
    const MIN_ARRAY_ELEMENTS: usize = 1; // This ensures the array is not empty.
    /// The maximum number of elements in an array.
    const MAX_ARRAY_ELEMENTS: usize = Self::MAX_DATA_ENTRIES;

    /// The minimum number of entries in a record.
    const MIN_RECORD_ENTRIES: usize = 1; // This accounts for 'record.owner'.
    /// The maximum number of entries in a record.
    const MAX_RECORD_ENTRIES: usize = Self::MIN_RECORD_ENTRIES.saturating_add(Self::MAX_DATA_ENTRIES);

    /// The maximum program size by number of characters.
    const MAX_PROGRAM_SIZE: usize = 100_000; // 100 KB

    /// The maximum number of mappings in a program.
    const MAX_MAPPINGS: usize = 31;
    /// The maximum number of functions in a program.
    const MAX_FUNCTIONS: usize = 31;
    /// The maximum number of structs in a program.
    const MAX_STRUCTS: usize = 10 * Self::MAX_FUNCTIONS;
    /// The maximum number of records in a program.
    const MAX_RECORDS: usize = 10 * Self::MAX_FUNCTIONS;
    /// The maximum number of closures in a program.
    const MAX_CLOSURES: usize = 2 * Self::MAX_FUNCTIONS;
    /// The maximum number of operands in an instruction.
    const MAX_OPERANDS: usize = Self::MAX_INPUTS;
    /// The maximum number of instructions in a closure or function.
    const MAX_INSTRUCTIONS: usize = u16::MAX as usize;
    /// The maximum number of commands in finalize.
    const MAX_COMMANDS: usize = u16::MAX as usize;
    /// The maximum number of write commands in finalize.
    const MAX_WRITES: u16 = 16;

    /// The maximum number of inputs per transition.
    const MAX_INPUTS: usize = 16;
    /// The maximum number of outputs per transition.
    const MAX_OUTPUTS: usize = 16;

    /// The maximum number of imports.
    const MAX_IMPORTS: usize = 64;

    /// The maximum number of bytes in a transaction.
    // Note: This value must **not** be decreased as it would invalidate existing transactions.
    const MAX_TRANSACTION_SIZE: usize = 128_000; // 128 kB

    /// The state root type.
    type StateRoot: Bech32ID<Field<Self>>;
    /// The block hash type.
    type BlockHash: Bech32ID<Field<Self>>;
    /// The ratification ID type.
    type RatificationID: Bech32ID<Field<Self>>;
    /// The transaction ID type.
    type TransactionID: Bech32ID<Field<Self>>;
    /// The transition ID type.
    type TransitionID: Bech32ID<Field<Self>>;
    /// The transmission checksum type.
    type TransmissionChecksum: IntegerType;

    /// A list of (consensus_version, block_height) pairs indicating when each consensus version takes effect.
    /// Documentation for what is changed at each version can be found in `N::CONSENSUS_VERSION`
    const CONSENSUS_VERSION_HEIGHTS: [(ConsensusVersion, u32); 3];
    ///  A list of (consensus_version, size) pairs indicating the maximum number of validators in a committee.
    //  Note: This value must **not** decrease without considering the impact on serialization.
    //  Decreasing this value will break backwards compatibility of serialization without explicit
    //  declaration of migration based on round number rather than block height.
    //  Increasing this value will require a migration to prevent forking during network upgrades.
    const MAX_CERTIFICATES: [(ConsensusVersion, u16); 2];

    /// Returns the consensus version which is active at the given height.
    ///
    /// V1: The initial genesis consensus version.
    ///
    /// V2: Update to the block reward and execution cost algorithms.
    ///
    /// V3: Update to the number of validators and finalize scope RNG seed.
    #[allow(non_snake_case)]
    fn CONSENSUS_VERSION(seek_height: u32) -> anyhow::Result<ConsensusVersion> {
        match Self::CONSENSUS_VERSION_HEIGHTS.binary_search_by(|(_, height)| height.cmp(&seek_height)) {
            // If a consensus version was found at this height, return it.
            Ok(index) => Ok(Self::CONSENSUS_VERSION_HEIGHTS[index].0),
            // If the specified height was not found, determine whether to return an appropriate version.
            Err(index) => {
                if index == 0 {
                    Err(anyhow!("Expected consensus version 1 to exist at height 0."))
                } else {
                    // Return the appropriate version belonging to the height *lower* than the sought height.
                    Ok(Self::CONSENSUS_VERSION_HEIGHTS[index - 1].0)
                }
            }
        }
    }
    /// Returns the height at which a specified consensus version becomes active.
    #[allow(non_snake_case)]
    fn CONSENSUS_HEIGHT(version: ConsensusVersion) -> Result<u32> {
        Ok(Self::CONSENSUS_VERSION_HEIGHTS.get(version as usize - 1).ok_or(anyhow!("Invalid consensus version"))?.1)
    }
    /// Returns the last `MAX_CERTIFICATES` value.
    #[allow(non_snake_case)]
    fn LATEST_MAX_CERTIFICATES() -> Result<u16> {
        Self::MAX_CERTIFICATES.last().map_or(Err(anyhow!("No MAX_CERTIFICATES defined.")), |(_, value)| Ok(*value))
    }

    /// Returns the genesis block bytes.
    fn genesis_bytes() -> &'static [u8];

    /// Returns the restrictions list as a JSON-compatible string.
    fn restrictions_list_as_str() -> &'static str;

    /// Returns the proving key for the given function name in `credits.aleo`.
    fn get_credits_proving_key(function_name: String) -> Result<&'static Arc<VarunaProvingKey<Self>>>;

    /// Returns the verifying key for the given function name in `credits.aleo`.
    fn get_credits_verifying_key(function_name: String) -> Result<&'static Arc<VarunaVerifyingKey<Self>>>;

    /// Returns the `proving key` for the inclusion circuit.
    fn inclusion_proving_key() -> &'static Arc<VarunaProvingKey<Self>>;

    /// Returns the `verifying key` for the inclusion circuit.
    fn inclusion_verifying_key() -> &'static Arc<VarunaVerifyingKey<Self>>;

    /// Returns the powers of `G`.
    fn g_powers() -> &'static Vec<Group<Self>>;

    /// Returns the scalar multiplication on the generator `G`.
    fn g_scalar_multiply(scalar: &Scalar<Self>) -> Group<Self>;

    /// Returns the Varuna universal prover.
    fn varuna_universal_prover() -> &'static UniversalProver<Self::PairingCurve>;

    /// Returns the Varuna universal verifier.
    fn varuna_universal_verifier() -> &'static UniversalVerifier<Self::PairingCurve>;

    /// Returns the sponge parameters for Varuna.
    fn varuna_fs_parameters() -> &'static FiatShamirParameters<Self>;

    /// Returns the encryption domain as a constant field element.
    fn encryption_domain() -> Field<Self>;

    /// Returns the graph key domain as a constant field element.
    fn graph_key_domain() -> Field<Self>;

    /// Returns the serial number domain as a constant field element.
    fn serial_number_domain() -> Field<Self>;

    /// Returns a BHP commitment with an input hasher of 256-bits and randomizer.
    fn commit_bhp256(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>>;

    /// Returns a BHP commitment with an input hasher of 512-bits and randomizer.
    fn commit_bhp512(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>>;

    /// Returns a BHP commitment with an input hasher of 768-bits and randomizer.
    fn commit_bhp768(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>>;

    /// Returns a BHP commitment with an input hasher of 1024-bits and randomizer.
    fn commit_bhp1024(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>>;

    /// Returns a Pedersen commitment for the given (up to) 64-bit input and randomizer.
    fn commit_ped64(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>>;

    /// Returns a Pedersen commitment for the given (up to) 128-bit input and randomizer.
    fn commit_ped128(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>>;

    /// Returns a BHP commitment with an input hasher of 256-bits and randomizer.
    fn commit_to_group_bhp256(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>>;

    /// Returns a BHP commitment with an input hasher of 512-bits and randomizer.
    fn commit_to_group_bhp512(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>>;

    /// Returns a BHP commitment with an input hasher of 768-bits and randomizer.
    fn commit_to_group_bhp768(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>>;

    /// Returns a BHP commitment with an input hasher of 1024-bits and randomizer.
    fn commit_to_group_bhp1024(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>>;

    /// Returns a Pedersen commitment for the given (up to) 64-bit input and randomizer.
    fn commit_to_group_ped64(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>>;

    /// Returns a Pedersen commitment for the given (up to) 128-bit input and randomizer.
    fn commit_to_group_ped128(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>>;

    /// Returns the BHP hash with an input hasher of 256-bits.
    fn hash_bhp256(input: &[bool]) -> Result<Field<Self>>;

    /// Returns the BHP hash with an input hasher of 512-bits.
    fn hash_bhp512(input: &[bool]) -> Result<Field<Self>>;

    /// Returns the BHP hash with an input hasher of 768-bits.
    fn hash_bhp768(input: &[bool]) -> Result<Field<Self>>;

    /// Returns the BHP hash with an input hasher of 1024-bits.
    fn hash_bhp1024(input: &[bool]) -> Result<Field<Self>>;

    /// Returns the Keccak hash with a 256-bit output.
    fn hash_keccak256(input: &[bool]) -> Result<Vec<bool>>;

    /// Returns the Keccak hash with a 384-bit output.
    fn hash_keccak384(input: &[bool]) -> Result<Vec<bool>>;

    /// Returns the Keccak hash with a 512-bit output.
    fn hash_keccak512(input: &[bool]) -> Result<Vec<bool>>;

    /// Returns the Pedersen hash for a given (up to) 64-bit input.
    fn hash_ped64(input: &[bool]) -> Result<Field<Self>>;

    /// Returns the Pedersen hash for a given (up to) 128-bit input.
    fn hash_ped128(input: &[bool]) -> Result<Field<Self>>;

    /// Returns the Poseidon hash with an input rate of 2.
    fn hash_psd2(input: &[Field<Self>]) -> Result<Field<Self>>;

    /// Returns the Poseidon hash with an input rate of 4.
    fn hash_psd4(input: &[Field<Self>]) -> Result<Field<Self>>;

    /// Returns the Poseidon hash with an input rate of 8.
    fn hash_psd8(input: &[Field<Self>]) -> Result<Field<Self>>;

    /// Returns the SHA-3 hash with a 256-bit output.
    fn hash_sha3_256(input: &[bool]) -> Result<Vec<bool>>;

    /// Returns the SHA-3 hash with a 384-bit output.
    fn hash_sha3_384(input: &[bool]) -> Result<Vec<bool>>;

    /// Returns the SHA-3 hash with a 512-bit output.
    fn hash_sha3_512(input: &[bool]) -> Result<Vec<bool>>;

    /// Returns the extended Poseidon hash with an input rate of 2.
    fn hash_many_psd2(input: &[Field<Self>], num_outputs: u16) -> Vec<Field<Self>>;

    /// Returns the extended Poseidon hash with an input rate of 4.
    fn hash_many_psd4(input: &[Field<Self>], num_outputs: u16) -> Vec<Field<Self>>;

    /// Returns the extended Poseidon hash with an input rate of 8.
    fn hash_many_psd8(input: &[Field<Self>], num_outputs: u16) -> Vec<Field<Self>>;

    /// Returns the BHP hash with an input hasher of 256-bits.
    fn hash_to_group_bhp256(input: &[bool]) -> Result<Group<Self>>;

    /// Returns the BHP hash with an input hasher of 512-bits.
    fn hash_to_group_bhp512(input: &[bool]) -> Result<Group<Self>>;

    /// Returns the BHP hash with an input hasher of 768-bits.
    fn hash_to_group_bhp768(input: &[bool]) -> Result<Group<Self>>;

    /// Returns the BHP hash with an input hasher of 1024-bits.
    fn hash_to_group_bhp1024(input: &[bool]) -> Result<Group<Self>>;

    /// Returns the Pedersen hash for a given (up to) 64-bit input.
    fn hash_to_group_ped64(input: &[bool]) -> Result<Group<Self>>;

    /// Returns the Pedersen hash for a given (up to) 128-bit input.
    fn hash_to_group_ped128(input: &[bool]) -> Result<Group<Self>>;

    /// Returns the Poseidon hash with an input rate of 2 on the affine curve.
    fn hash_to_group_psd2(input: &[Field<Self>]) -> Result<Group<Self>>;

    /// Returns the Poseidon hash with an input rate of 4 on the affine curve.
    fn hash_to_group_psd4(input: &[Field<Self>]) -> Result<Group<Self>>;

    /// Returns the Poseidon hash with an input rate of 8 on the affine curve.
    fn hash_to_group_psd8(input: &[Field<Self>]) -> Result<Group<Self>>;

    /// Returns the Poseidon hash with an input rate of 2 on the scalar field.
    fn hash_to_scalar_psd2(input: &[Field<Self>]) -> Result<Scalar<Self>>;

    /// Returns the Poseidon hash with an input rate of 4 on the scalar field.
    fn hash_to_scalar_psd4(input: &[Field<Self>]) -> Result<Scalar<Self>>;

    /// Returns the Poseidon hash with an input rate of 8 on the scalar field.
    fn hash_to_scalar_psd8(input: &[Field<Self>]) -> Result<Scalar<Self>>;

    /// Returns a Merkle tree with a BHP leaf hasher of 1024-bits and a BHP path hasher of 512-bits.
    fn merkle_tree_bhp<const DEPTH: u8>(leaves: &[Vec<bool>]) -> Result<BHPMerkleTree<Self, DEPTH>>;

    /// Returns a Merkle tree with a Poseidon leaf hasher with input rate of 4 and a Poseidon path hasher with input rate of 2.
    fn merkle_tree_psd<const DEPTH: u8>(leaves: &[Vec<Field<Self>>]) -> Result<PoseidonMerkleTree<Self, DEPTH>>;

    /// Returns `true` if the given Merkle path is valid for the given root and leaf.
    #[allow(clippy::ptr_arg)]
    fn verify_merkle_path_bhp<const DEPTH: u8>(
        path: &MerklePath<Self, DEPTH>,
        root: &Field<Self>,
        leaf: &Vec<bool>,
    ) -> bool;

    /// Returns `true` if the given Merkle path is valid for the given root and leaf.
    #[allow(clippy::ptr_arg)]
    fn verify_merkle_path_psd<const DEPTH: u8>(
        path: &MerklePath<Self, DEPTH>,
        root: &Field<Self>,
        leaf: &Vec<Field<Self>>,
    ) -> bool;
}

/// Returns the consensus configuration value for the specified height.
///
/// Arguments:
/// - `$network`: The network to use the constant of.
/// - `$constant`: The constant to search a value of.
/// - `$seek_height`: The block height to search the value for.
#[macro_export]
macro_rules! consensus_config_value {
    ($network:ident, $constant:ident, $seek_height:expr) => {
        // Search the consensus version enacted at the specified height.
        $network::CONSENSUS_VERSION($seek_height).map_or(None, |seek_version| {
            // Search the consensus value for the specified version.
            match $network::$constant.binary_search_by(|(version, _)| version.cmp(&seek_version)) {
                // If a value was found for this consensus version, return it.
                Ok(index) => Some($network::$constant[index].1),
                // If the specified version was not found exactly, determine whether to return an appropriate value anyway.
                Err(index) => {
                    // This constant is not yet in effect at this consensus version.
                    if index == 0 {
                        None
                    // Return the appropriate value belonging to the consensus version *lower* than the sought version.
                    } else {
                        Some($network::$constant[index - 1].1)
                    }
                }
            }
        })
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ensure that the consensus constants are defined and correct at genesis.
    /// It is possible this invariant no longer holds in the future, e.g. due to pruning or novel types of constants.
    fn consensus_constants_at_genesis<N: Network>() {
        let height = N::CONSENSUS_VERSION_HEIGHTS.first().unwrap().1;
        assert_eq!(height, 0);
        let consensus_version = N::CONSENSUS_VERSION_HEIGHTS.first().unwrap().0;
        assert_eq!(consensus_version, ConsensusVersion::V1);
        assert_eq!(consensus_version as usize, 1);
    }

    /// Ensure that the consensus *versions* are unique, incrementing and start with 1.
    fn consensus_versions<N: Network>() {
        let mut previous_version = N::CONSENSUS_VERSION_HEIGHTS.first().unwrap().0;
        // Ensure that the consensus versions start with 1.
        assert_eq!(previous_version as usize, 1);
        // Ensure that the consensus versions are unique and incrementing by 1.
        for (version, _) in N::CONSENSUS_VERSION_HEIGHTS.iter().skip(1) {
            assert_eq!(*version as usize, previous_version as usize + 1);
            previous_version = *version;
        }
        // Ensure that the consensus versions are unique and incrementing.
        let mut previous_version = N::MAX_CERTIFICATES.first().unwrap().0;
        for (version, _) in N::MAX_CERTIFICATES.iter().skip(1) {
            assert!(*version > previous_version);
            previous_version = *version;
        }
    }

    /// Ensure that consensus *heights* are unique and incrementing.
    fn consensus_constants_increasing_heights<N: Network>() {
        let mut previous_height = N::CONSENSUS_VERSION_HEIGHTS.first().unwrap().1;
        for (version, height) in N::CONSENSUS_VERSION_HEIGHTS.iter().skip(1) {
            assert!(*height > previous_height);
            previous_height = *height;
            // Ensure that N::CONSENSUS_VERSION returns the expected value.
            assert_eq!(N::CONSENSUS_VERSION(*height).unwrap(), *version);
            // Ensure that N::CONSENSUS_HEIGHT returns the expected value.
            assert_eq!(N::CONSENSUS_HEIGHT(*version).unwrap(), *height);
        }
    }

    /// Ensure that version of all consensus-relevant constants are present in the consensus version heights.
    fn consensus_constants_valid_heights<N: Network>() {
        for (version, value) in N::MAX_CERTIFICATES.iter() {
            // Ensure that the height at which an update occurs are present in CONSENSUS_VERSION_HEIGHTS.
            let height = N::CONSENSUS_VERSION_HEIGHTS.iter().find(|(c_version, _)| *c_version == *version).unwrap().1;
            // Double-check that consensus_config_value returns the correct value.
            assert_eq!(consensus_config_value!(N, MAX_CERTIFICATES, height).unwrap(), *value);
        }
    }

    /// Ensure that `MAX_CERTIFICATES` increases and is correctly defined.
    /// See the constant declaration for an explanation why.
    fn max_certificates_increasing<N: Network>() {
        let mut previous_value = N::MAX_CERTIFICATES.first().unwrap().1;
        for (_, value) in N::MAX_CERTIFICATES.iter().skip(1) {
            assert!(*value >= previous_value);
            previous_value = *value;
        }
    }

    /// Ensure that the number of constant definitions is the same across networks.
    fn constants_equal_length<N1: Network, N2: Network, N3: Network>() {
        // If we can construct an array, that means the underlying types must be the same.
        let _ = [N1::CONSENSUS_VERSION_HEIGHTS, N2::CONSENSUS_VERSION_HEIGHTS, N3::CONSENSUS_VERSION_HEIGHTS];
        let _ = [N1::MAX_CERTIFICATES, N2::MAX_CERTIFICATES, N3::MAX_CERTIFICATES];
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_consensus_constants() {
        consensus_constants_at_genesis::<MainnetV0>();
        consensus_constants_at_genesis::<TestnetV0>();
        consensus_constants_at_genesis::<CanaryV0>();

        consensus_versions::<MainnetV0>();
        consensus_versions::<TestnetV0>();
        consensus_versions::<CanaryV0>();

        consensus_constants_increasing_heights::<MainnetV0>();
        consensus_constants_increasing_heights::<TestnetV0>();
        consensus_constants_increasing_heights::<CanaryV0>();

        consensus_constants_valid_heights::<MainnetV0>();
        consensus_constants_valid_heights::<TestnetV0>();
        consensus_constants_valid_heights::<CanaryV0>();

        max_certificates_increasing::<MainnetV0>();
        max_certificates_increasing::<TestnetV0>();
        max_certificates_increasing::<CanaryV0>();

        constants_equal_length::<MainnetV0, TestnetV0, CanaryV0>();
    }
}
