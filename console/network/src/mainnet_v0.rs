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

use super::*;
use snarkvm_console_algorithms::{
    BHP256,
    BHP512,
    BHP768,
    BHP1024,
    Blake2Xs,
    Keccak256,
    Keccak384,
    Keccak512,
    Pedersen64,
    Pedersen128,
    Poseidon2,
    Poseidon4,
    Poseidon8,
    Sha3_256,
    Sha3_384,
    Sha3_512,
};

lazy_static! {
    /// The group bases for the Aleo signature and encryption schemes.
    pub static ref GENERATOR_G: Vec<Group<MainnetV0 >> = MainnetV0::new_bases("AleoAccountEncryptionAndSignatureScheme0");

    /// The Varuna sponge parameters.
    pub static ref VARUNA_FS_PARAMETERS: FiatShamirParameters<MainnetV0> = FiatShamir::<MainnetV0>::sample_parameters();

    /// The encryption domain as a constant field element.
    pub static ref ENCRYPTION_DOMAIN: Field<MainnetV0> = Field::<MainnetV0>::new_domain_separator("AleoSymmetricEncryption0");
    /// The graph key domain as a constant field element.
    pub static ref GRAPH_KEY_DOMAIN: Field<MainnetV0> = Field::<MainnetV0>::new_domain_separator("AleoGraphKey0");
    /// The serial number domain as a constant field element.
    pub static ref SERIAL_NUMBER_DOMAIN: Field<MainnetV0> = Field::<MainnetV0>::new_domain_separator("AleoSerialNumber0");

    /// The BHP hash function, which can take an input of up to 256 bits.
    pub static ref BHP_256: BHP256<MainnetV0> = BHP256::<MainnetV0>::setup("AleoBHP256").expect("Failed to setup BHP256");
    /// The BHP hash function, which can take an input of up to 512 bits.
    pub static ref BHP_512: BHP512<MainnetV0> = BHP512::<MainnetV0>::setup("AleoBHP512").expect("Failed to setup BHP512");
    /// The BHP hash function, which can take an input of up to 768 bits.
    pub static ref BHP_768: BHP768<MainnetV0> = BHP768::<MainnetV0>::setup("AleoBHP768").expect("Failed to setup BHP768");
    /// The BHP hash function, which can take an input of up to 1024 bits.
    pub static ref BHP_1024: BHP1024<MainnetV0> = BHP1024::<MainnetV0>::setup("AleoBHP1024").expect("Failed to setup BHP1024");

    /// The Pedersen hash function, which can take an input of up to 64 bits.
    pub static ref PEDERSEN_64: Pedersen64<MainnetV0> = Pedersen64::<MainnetV0>::setup("AleoPedersen64");
    /// The Pedersen hash function, which can take an input of up to 128 bits.
    pub static ref PEDERSEN_128: Pedersen128<MainnetV0> = Pedersen128::<MainnetV0>::setup("AleoPedersen128");

    /// The Poseidon hash function, using a rate of 2.
    pub static ref POSEIDON_2: Poseidon2<MainnetV0> = Poseidon2::<MainnetV0>::setup("AleoPoseidon2").expect("Failed to setup Poseidon2");
    /// The Poseidon hash function, using a rate of 4.
    pub static ref POSEIDON_4: Poseidon4<MainnetV0> = Poseidon4::<MainnetV0>::setup("AleoPoseidon4").expect("Failed to setup Poseidon4");
    /// The Poseidon hash function, using a rate of 8.
    pub static ref POSEIDON_8: Poseidon8<MainnetV0> = Poseidon8::<MainnetV0>::setup("AleoPoseidon8").expect("Failed to setup Poseidon8");

    pub static ref CREDITS_PROVING_KEYS: IndexMap<String, Arc<VarunaProvingKey<Console>>> = {
        let mut map = IndexMap::new();
        snarkvm_parameters::insert_credit_keys!(map, VarunaProvingKey<Console>, Prover);
        map
    };
    pub static ref CREDITS_VERIFYING_KEYS: IndexMap<String, Arc<VarunaVerifyingKey<Console>>> = {
        let mut map = IndexMap::new();
        snarkvm_parameters::insert_credit_keys!(map, VarunaVerifyingKey<Console>, Verifier);
        map
    };
}

pub const TRANSACTION_PREFIX: &str = "at";

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MainnetV0;

impl MainnetV0 {
    /// Initializes a new instance of group bases from a given input domain message.
    fn new_bases(message: &str) -> Vec<Group<Self>> {
        // Hash the given message to a point on the curve, to initialize the starting base.
        let (base, _, _) = Blake2Xs::hash_to_curve::<<Self as Environment>::Affine>(message);

        // Compute the bases up to the size of the scalar field (in bits).
        let mut g = Group::<Self>::new(base);
        let mut g_bases = Vec::with_capacity(Scalar::<Self>::size_in_bits());
        for _ in 0..Scalar::<Self>::size_in_bits() {
            g_bases.push(g);
            g = g.double();
        }
        g_bases
    }
}

impl Environment for MainnetV0 {
    type Affine = <Console as Environment>::Affine;
    type BigInteger = <Console as Environment>::BigInteger;
    type Field = <Console as Environment>::Field;
    type PairingCurve = <Console as Environment>::PairingCurve;
    type Projective = <Console as Environment>::Projective;
    type Scalar = <Console as Environment>::Scalar;

    /// The coefficient `A` of the twisted Edwards curve.
    const EDWARDS_A: Self::Field = Console::EDWARDS_A;
    /// The coefficient `D` of the twisted Edwards curve.
    const EDWARDS_D: Self::Field = Console::EDWARDS_D;
    /// The coefficient `A` of the Montgomery curve.
    const MONTGOMERY_A: Self::Field = Console::MONTGOMERY_A;
    /// The coefficient `B` of the Montgomery curve.
    const MONTGOMERY_B: Self::Field = Console::MONTGOMERY_B;
}

impl Network for MainnetV0 {
    /// The block hash type.
    type BlockHash = AleoID<Field<Self>, { hrp2!("ab") }>;
    /// The ratification ID type.
    type RatificationID = AleoID<Field<Self>, { hrp2!("ar") }>;
    /// The state root type.
    type StateRoot = AleoID<Field<Self>, { hrp2!("sr") }>;
    /// The transaction ID type.
    type TransactionID = AleoID<Field<Self>, { hrp2!(TRANSACTION_PREFIX) }>;
    /// The transition ID type.
    type TransitionID = AleoID<Field<Self>, { hrp2!("au") }>;
    /// The transmission checksum type.
    type TransmissionChecksum = u128;

    /// The block height from which consensus V2 rules apply.
    #[cfg(not(any(test, feature = "test")))]
    const CONSENSUS_V2_HEIGHT: u32 = 2_800_000;
    // TODO (raychu86): Update this value based on the desired mainnet height.
    /// The block height from which consensus V2 rules apply.
    #[cfg(any(test, feature = "test"))]
    const CONSENSUS_V2_HEIGHT: u32 = 10;
    /// The network edition.
    const EDITION: u16 = 0;
    /// The genesis block coinbase target.
    #[cfg(not(feature = "test"))]
    const GENESIS_COINBASE_TARGET: u64 = (1u64 << 29).saturating_sub(1);
    /// The genesis block coinbase target.
    /// This is deliberately set to a low value (32) for testing purposes only.
    #[cfg(feature = "test")]
    const GENESIS_COINBASE_TARGET: u64 = (1u64 << 5).saturating_sub(1);
    /// The genesis block proof target.
    #[cfg(not(feature = "test"))]
    const GENESIS_PROOF_TARGET: u64 = 1u64 << 27;
    /// The genesis block proof target.
    /// This is deliberately set to a low value (8) for testing purposes only.
    #[cfg(feature = "test")]
    const GENESIS_PROOF_TARGET: u64 = 1u64 << 3;
    /// The fixed timestamp of the genesis block.
    const GENESIS_TIMESTAMP: i64 = 1725462000 /* 2024-09-04 11:00:00 UTC */;
    /// The network ID.
    const ID: u16 = 0;
    /// The function name for the inclusion circuit.
    const INCLUSION_FUNCTION_NAME: &'static str = snarkvm_parameters::mainnet::NETWORK_INCLUSION_FUNCTION_NAME;
    /// The maximum number of certificates in a batch.
    const MAX_CERTIFICATES: u16 = 16;
    /// The network name.
    const NAME: &'static str = "Aleo Mainnet (v0)";

    /// Returns the genesis block bytes.
    fn genesis_bytes() -> &'static [u8] {
        snarkvm_parameters::mainnet::GenesisBytes::load_bytes()
    }

    /// Returns the restrictions list as a JSON-compatible string.
    fn restrictions_list_as_str() -> &'static str {
        snarkvm_parameters::mainnet::RESTRICTIONS_LIST
    }

    /// Returns the proving key for the given function name in `credits.aleo`.
    fn get_credits_proving_key(function_name: String) -> Result<&'static Arc<VarunaProvingKey<Self>>> {
        CREDITS_PROVING_KEYS
            .get(&function_name)
            .ok_or_else(|| anyhow!("Proving key for credits.aleo/{function_name}' not found"))
    }

    /// Returns the verifying key for the given function name in `credits.aleo`.
    fn get_credits_verifying_key(function_name: String) -> Result<&'static Arc<VarunaVerifyingKey<Self>>> {
        CREDITS_VERIFYING_KEYS
            .get(&function_name)
            .ok_or_else(|| anyhow!("Verifying key for credits.aleo/{function_name}' not found"))
    }

    /// Returns the `proving key` for the inclusion circuit.
    fn inclusion_proving_key() -> &'static Arc<VarunaProvingKey<Self>> {
        static INSTANCE: OnceCell<Arc<VarunaProvingKey<Console>>> = OnceCell::new();
        INSTANCE.get_or_init(|| {
            // Skipping the first byte, which is the encoded version.
            Arc::new(
                CircuitProvingKey::from_bytes_le(&snarkvm_parameters::mainnet::INCLUSION_PROVING_KEY[1..])
                    .expect("Failed to load inclusion proving key."),
            )
        })
    }

    /// Returns the `verifying key` for the inclusion circuit.
    fn inclusion_verifying_key() -> &'static Arc<VarunaVerifyingKey<Self>> {
        static INSTANCE: OnceCell<Arc<VarunaVerifyingKey<Console>>> = OnceCell::new();
        INSTANCE.get_or_init(|| {
            // Skipping the first byte, which is the encoded version.
            Arc::new(
                CircuitVerifyingKey::from_bytes_le(&snarkvm_parameters::mainnet::INCLUSION_VERIFYING_KEY[1..])
                    .expect("Failed to load inclusion verifying key."),
            )
        })
    }

    /// Returns the powers of `G`.
    fn g_powers() -> &'static Vec<Group<Self>> {
        &GENERATOR_G
    }

    /// Returns the scalar multiplication on the generator `G`.
    fn g_scalar_multiply(scalar: &Scalar<Self>) -> Group<Self> {
        GENERATOR_G
            .iter()
            .zip_eq(&scalar.to_bits_le())
            .filter_map(|(base, bit)| match bit {
                true => Some(base),
                false => None,
            })
            .sum()
    }

    /// Returns the Varuna universal prover.
    fn varuna_universal_prover() -> &'static UniversalProver<Self::PairingCurve> {
        static INSTANCE: OnceCell<UniversalProver<<Console as Environment>::PairingCurve>> = OnceCell::new();
        INSTANCE.get_or_init(|| {
            snarkvm_algorithms::polycommit::kzg10::UniversalParams::load()
                .expect("Failed to load universal SRS (KZG10).")
                .to_universal_prover()
                .expect("Failed to convert universal SRS (KZG10) to the prover.")
        })
    }

    /// Returns the Varuna universal verifier.
    fn varuna_universal_verifier() -> &'static UniversalVerifier<Self::PairingCurve> {
        static INSTANCE: OnceCell<UniversalVerifier<<Console as Environment>::PairingCurve>> = OnceCell::new();
        INSTANCE.get_or_init(|| {
            snarkvm_algorithms::polycommit::kzg10::UniversalParams::load()
                .expect("Failed to load universal SRS (KZG10).")
                .to_universal_verifier()
                .expect("Failed to convert universal SRS (KZG10) to the verifier.")
        })
    }

    /// Returns the sponge parameters used for the sponge in the Varuna SNARK.
    fn varuna_fs_parameters() -> &'static FiatShamirParameters<Self> {
        &VARUNA_FS_PARAMETERS
    }

    /// Returns the encryption domain as a constant field element.
    fn encryption_domain() -> Field<Self> {
        *ENCRYPTION_DOMAIN
    }

    /// Returns the graph key domain as a constant field element.
    fn graph_key_domain() -> Field<Self> {
        *GRAPH_KEY_DOMAIN
    }

    /// Returns the serial number domain as a constant field element.
    fn serial_number_domain() -> Field<Self> {
        *SERIAL_NUMBER_DOMAIN
    }

    /// Returns a BHP commitment with an input hasher of 256-bits and randomizer.
    fn commit_bhp256(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>> {
        BHP_256.commit(input, randomizer)
    }

    /// Returns a BHP commitment with an input hasher of 512-bits and randomizer.
    fn commit_bhp512(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>> {
        BHP_512.commit(input, randomizer)
    }

    /// Returns a BHP commitment with an input hasher of 768-bits and randomizer.
    fn commit_bhp768(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>> {
        BHP_768.commit(input, randomizer)
    }

    /// Returns a BHP commitment with an input hasher of 1024-bits and randomizer.
    fn commit_bhp1024(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>> {
        BHP_1024.commit(input, randomizer)
    }

    /// Returns a Pedersen commitment for the given (up to) 64-bit input and randomizer.
    fn commit_ped64(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>> {
        PEDERSEN_64.commit(input, randomizer)
    }

    /// Returns a Pedersen commitment for the given (up to) 128-bit input and randomizer.
    fn commit_ped128(input: &[bool], randomizer: &Scalar<Self>) -> Result<Field<Self>> {
        PEDERSEN_128.commit(input, randomizer)
    }

    /// Returns a BHP commitment with an input hasher of 256-bits and randomizer.
    fn commit_to_group_bhp256(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>> {
        BHP_256.commit_uncompressed(input, randomizer)
    }

    /// Returns a BHP commitment with an input hasher of 512-bits and randomizer.
    fn commit_to_group_bhp512(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>> {
        BHP_512.commit_uncompressed(input, randomizer)
    }

    /// Returns a BHP commitment with an input hasher of 768-bits and randomizer.
    fn commit_to_group_bhp768(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>> {
        BHP_768.commit_uncompressed(input, randomizer)
    }

    /// Returns a BHP commitment with an input hasher of 1024-bits and randomizer.
    fn commit_to_group_bhp1024(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>> {
        BHP_1024.commit_uncompressed(input, randomizer)
    }

    /// Returns a Pedersen commitment for the given (up to) 64-bit input and randomizer.
    fn commit_to_group_ped64(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>> {
        PEDERSEN_64.commit_uncompressed(input, randomizer)
    }

    /// Returns a Pedersen commitment for the given (up to) 128-bit input and randomizer.
    fn commit_to_group_ped128(input: &[bool], randomizer: &Scalar<Self>) -> Result<Group<Self>> {
        PEDERSEN_128.commit_uncompressed(input, randomizer)
    }

    /// Returns the BHP hash with an input hasher of 256-bits.
    fn hash_bhp256(input: &[bool]) -> Result<Field<Self>> {
        BHP_256.hash(input)
    }

    /// Returns the BHP hash with an input hasher of 512-bits.
    fn hash_bhp512(input: &[bool]) -> Result<Field<Self>> {
        BHP_512.hash(input)
    }

    /// Returns the BHP hash with an input hasher of 768-bits.
    fn hash_bhp768(input: &[bool]) -> Result<Field<Self>> {
        BHP_768.hash(input)
    }

    /// Returns the BHP hash with an input hasher of 1024-bits.
    fn hash_bhp1024(input: &[bool]) -> Result<Field<Self>> {
        BHP_1024.hash(input)
    }

    /// Returns the Keccak hash with a 256-bit output.
    fn hash_keccak256(input: &[bool]) -> Result<Vec<bool>> {
        Keccak256::default().hash(input)
    }

    /// Returns the Keccak hash with a 384-bit output.
    fn hash_keccak384(input: &[bool]) -> Result<Vec<bool>> {
        Keccak384::default().hash(input)
    }

    /// Returns the Keccak hash with a 512-bit output.
    fn hash_keccak512(input: &[bool]) -> Result<Vec<bool>> {
        Keccak512::default().hash(input)
    }

    /// Returns the Pedersen hash for a given (up to) 64-bit input.
    fn hash_ped64(input: &[bool]) -> Result<Field<Self>> {
        PEDERSEN_64.hash(input)
    }

    /// Returns the Pedersen hash for a given (up to) 128-bit input.
    fn hash_ped128(input: &[bool]) -> Result<Field<Self>> {
        PEDERSEN_128.hash(input)
    }

    /// Returns the Poseidon hash with an input rate of 2.
    fn hash_psd2(input: &[Field<Self>]) -> Result<Field<Self>> {
        POSEIDON_2.hash(input)
    }

    /// Returns the Poseidon hash with an input rate of 4.
    fn hash_psd4(input: &[Field<Self>]) -> Result<Field<Self>> {
        POSEIDON_4.hash(input)
    }

    /// Returns the Poseidon hash with an input rate of 8.
    fn hash_psd8(input: &[Field<Self>]) -> Result<Field<Self>> {
        POSEIDON_8.hash(input)
    }

    /// Returns the SHA-3 hash with a 256-bit output.
    fn hash_sha3_256(input: &[bool]) -> Result<Vec<bool>> {
        Sha3_256::default().hash(input)
    }

    /// Returns the SHA-3 hash with a 384-bit output.
    fn hash_sha3_384(input: &[bool]) -> Result<Vec<bool>> {
        Sha3_384::default().hash(input)
    }

    /// Returns the SHA-3 hash with a 512-bit output.
    fn hash_sha3_512(input: &[bool]) -> Result<Vec<bool>> {
        Sha3_512::default().hash(input)
    }

    /// Returns the extended Poseidon hash with an input rate of 2.
    fn hash_many_psd2(input: &[Field<Self>], num_outputs: u16) -> Vec<Field<Self>> {
        POSEIDON_2.hash_many(input, num_outputs)
    }

    /// Returns the extended Poseidon hash with an input rate of 4.
    fn hash_many_psd4(input: &[Field<Self>], num_outputs: u16) -> Vec<Field<Self>> {
        POSEIDON_4.hash_many(input, num_outputs)
    }

    /// Returns the extended Poseidon hash with an input rate of 8.
    fn hash_many_psd8(input: &[Field<Self>], num_outputs: u16) -> Vec<Field<Self>> {
        POSEIDON_8.hash_many(input, num_outputs)
    }

    /// Returns the BHP hash with an input hasher of 256-bits.
    fn hash_to_group_bhp256(input: &[bool]) -> Result<Group<Self>> {
        BHP_256.hash_uncompressed(input)
    }

    /// Returns the BHP hash with an input hasher of 512-bits.
    fn hash_to_group_bhp512(input: &[bool]) -> Result<Group<Self>> {
        BHP_512.hash_uncompressed(input)
    }

    /// Returns the BHP hash with an input hasher of 768-bits.
    fn hash_to_group_bhp768(input: &[bool]) -> Result<Group<Self>> {
        BHP_768.hash_uncompressed(input)
    }

    /// Returns the BHP hash with an input hasher of 1024-bits.
    fn hash_to_group_bhp1024(input: &[bool]) -> Result<Group<Self>> {
        BHP_1024.hash_uncompressed(input)
    }

    /// Returns the Pedersen hash for a given (up to) 64-bit input.
    fn hash_to_group_ped64(input: &[bool]) -> Result<Group<Self>> {
        PEDERSEN_64.hash_uncompressed(input)
    }

    /// Returns the Pedersen hash for a given (up to) 128-bit input.
    fn hash_to_group_ped128(input: &[bool]) -> Result<Group<Self>> {
        PEDERSEN_128.hash_uncompressed(input)
    }

    /// Returns the Poseidon hash with an input rate of 2 on the affine curve.
    fn hash_to_group_psd2(input: &[Field<Self>]) -> Result<Group<Self>> {
        POSEIDON_2.hash_to_group(input)
    }

    /// Returns the Poseidon hash with an input rate of 4 on the affine curve.
    fn hash_to_group_psd4(input: &[Field<Self>]) -> Result<Group<Self>> {
        POSEIDON_4.hash_to_group(input)
    }

    /// Returns the Poseidon hash with an input rate of 8 on the affine curve.
    fn hash_to_group_psd8(input: &[Field<Self>]) -> Result<Group<Self>> {
        POSEIDON_8.hash_to_group(input)
    }

    /// Returns the Poseidon hash with an input rate of 2 on the scalar field.
    fn hash_to_scalar_psd2(input: &[Field<Self>]) -> Result<Scalar<Self>> {
        POSEIDON_2.hash_to_scalar(input)
    }

    /// Returns the Poseidon hash with an input rate of 4 on the scalar field.
    fn hash_to_scalar_psd4(input: &[Field<Self>]) -> Result<Scalar<Self>> {
        POSEIDON_4.hash_to_scalar(input)
    }

    /// Returns the Poseidon hash with an input rate of 8 on the scalar field.
    fn hash_to_scalar_psd8(input: &[Field<Self>]) -> Result<Scalar<Self>> {
        POSEIDON_8.hash_to_scalar(input)
    }

    /// Returns a Merkle tree with a BHP leaf hasher of 1024-bits and a BHP path hasher of 512-bits.
    fn merkle_tree_bhp<const DEPTH: u8>(leaves: &[Vec<bool>]) -> Result<BHPMerkleTree<Self, DEPTH>> {
        MerkleTree::new(&*BHP_1024, &*BHP_512, leaves)
    }

    /// Returns a Merkle tree with a Poseidon leaf hasher with input rate of 4 and a Poseidon path hasher with input rate of 2.
    fn merkle_tree_psd<const DEPTH: u8>(leaves: &[Vec<Field<Self>>]) -> Result<PoseidonMerkleTree<Self, DEPTH>> {
        MerkleTree::new(&*POSEIDON_4, &*POSEIDON_2, leaves)
    }

    /// Returns `true` if the given Merkle path is valid for the given root and leaf.
    fn verify_merkle_path_bhp<const DEPTH: u8>(
        path: &MerklePath<Self, DEPTH>,
        root: &Field<Self>,
        leaf: &Vec<bool>,
    ) -> bool {
        path.verify(&*BHP_1024, &*BHP_512, root, leaf)
    }

    /// Returns `true` if the given Merkle path is valid for the given root and leaf.
    fn verify_merkle_path_psd<const DEPTH: u8>(
        path: &MerklePath<Self, DEPTH>,
        root: &Field<Self>,
        leaf: &Vec<Field<Self>>,
    ) -> bool {
        path.verify(&*POSEIDON_4, &*POSEIDON_2, root, leaf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_g_scalar_multiply() {
        // Compute G^r.
        let scalar = Scalar::rand(&mut TestRng::default());
        let group = CurrentNetwork::g_scalar_multiply(&scalar);
        assert_eq!(group, CurrentNetwork::g_powers()[0] * scalar);
    }
}
