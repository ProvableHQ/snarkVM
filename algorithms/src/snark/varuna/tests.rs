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

#[cfg(any(test, feature = "test"))]
mod varuna {
    use crate::{
        snark::varuna::{
            mode::SNARKMode,
            test_circuit::TestCircuit,
            AHPForR1CS,
            CircuitVerifyingKey,
            VarunaHidingMode,
            VarunaNonHidingMode,
            VarunaSNARK,
        },
        traits::{AlgebraicSponge, SNARK},
    };

    use std::collections::BTreeMap;

    use snarkvm_curves::bls12_377::{Bls12_377, Fq, Fr};
    use snarkvm_utilities::{
        rand::{TestRng, Uniform},
        ToBytes,
    };

    type FS = crate::crypto_hash::PoseidonSponge<Fq, 2, 1>;

    type VarunaSonicInst = VarunaSNARK<Bls12_377, FS, VarunaHidingMode>;
    type VarunaSonicPoSWInst = VarunaSNARK<Bls12_377, FS, VarunaNonHidingMode>;

    macro_rules! impl_varuna_test {
        ($test_struct: ident, $snark_inst: tt, $snark_mode: tt) => {
            struct $test_struct {}
            impl $test_struct {
                pub(crate) fn test_circuit(num_constraints: usize, num_variables: usize, pk_size_expectation: usize) {
                    let rng = &mut snarkvm_utilities::rand::TestRng::default();
                    let random = Fr::rand(rng);

                    let max_degree = AHPForR1CS::<Fr, $snark_mode>::max_degree(100, 25, 300).unwrap();
                    let universal_srs = $snark_inst::universal_setup(max_degree).unwrap();
                    let universal_prover = &universal_srs.to_universal_prover().unwrap();
                    let universal_verifier = &universal_srs.to_universal_verifier().unwrap();
                    let fs_parameters = FS::sample_parameters();

                    for i in 0..5 {
                        let mul_depth = 1;
                        println!("running test with SM::ZK: {}, mul_depth: {}, num_constraints: {}, num_variables: {}", $snark_mode::ZK, mul_depth + i, num_constraints + i, num_variables + i);
                        let (circ, public_inputs) = TestCircuit::gen_rand(mul_depth + i, num_constraints + i, num_variables + i, rng);
                        let mut fake_inputs = public_inputs.clone();
                        fake_inputs[public_inputs.len() - 1] = random;

                        let (index_pk, index_vk) = $snark_inst::circuit_setup(&universal_srs, &circ).unwrap();
                        println!("Called circuit setup");

                        let certificate = $snark_inst::prove_vk(universal_prover, &fs_parameters, &index_vk, &index_pk).unwrap();
                        assert!($snark_inst::verify_vk(universal_verifier, &fs_parameters, &circ, &index_vk, &certificate).unwrap());
                        println!("verified vk");

                        if i == 0 {
                            assert_eq!(pk_size_expectation, index_pk.to_bytes_le().unwrap().len(), "Update me if serialization has changed");
                        }
                        assert_eq!(664, index_vk.to_bytes_le().unwrap().len(), "Update me if serialization has changed");

                        let proof = $snark_inst::prove(universal_prover, &fs_parameters, &index_pk, &circ, rng).unwrap();
                        println!("Called prover");

                        assert!($snark_inst::verify(universal_verifier, &fs_parameters, &index_vk, public_inputs, &proof).unwrap());
                        println!("Called verifier");
                        eprintln!("\nShould not verify (i.e. verifier messages should print below):");
                        assert!(!$snark_inst::verify(universal_verifier, &fs_parameters, &index_vk, fake_inputs, &proof).unwrap());
                    }

                    for circuit_batch_size in (0..4).map(|i| 2usize.pow(i)) {
                        for instance_batch_size in (0..4).map(|i| 2usize.pow(i)) {
                            println!("running test with circuit_batch_size: {circuit_batch_size} and instance_batch_size: {instance_batch_size}");
                            let mut constraints = BTreeMap::new();
                            let mut inputs = BTreeMap::new();

                            for i in 0..circuit_batch_size {
                                let (circuit_batch, input_batch): (Vec<_>, Vec<_>) = (0..instance_batch_size)
                                .map(|_| {
                                    let mul_depth = 2 + i;
                                    let (circ, inputs) = TestCircuit::gen_rand(mul_depth, num_constraints + 100*i, num_variables, rng);
                                    (circ, inputs)
                                })
                                .unzip();
                                let circuit_id = AHPForR1CS::<Fr, $snark_mode>::index(&circuit_batch[0]).unwrap().id;
                                constraints.insert(circuit_id, circuit_batch);
                                inputs.insert(circuit_id, input_batch);
                            }
                            let unique_instances = constraints.values().map(|instances| &instances[0]).collect::<Vec<_>>();

                            let index_keys =
                                $snark_inst::batch_circuit_setup(&universal_srs, unique_instances.as_slice()).unwrap();
                            println!("Called circuit setup");

                            let mut pks_to_constraints = BTreeMap::new();
                            let mut vks_to_inputs = BTreeMap::new();

                            for (index_pk, index_vk) in index_keys.iter() {
                                let certificate = $snark_inst::prove_vk(universal_prover, &fs_parameters, &index_vk, &index_pk).unwrap();
                                let circuits = constraints[&index_pk.circuit.id].as_slice();
                                assert!($snark_inst::verify_vk(universal_verifier, &fs_parameters, &circuits[0], &index_vk, &certificate).unwrap());
                                pks_to_constraints.insert(index_pk, circuits);
                                vks_to_inputs.insert(index_vk, inputs[&index_pk.circuit.id].as_slice());
                            }
                            println!("verified vks");

                            let proof =
                                $snark_inst::prove_batch(universal_prover, &fs_parameters, &pks_to_constraints, rng).unwrap();
                            println!("Called prover");

                            assert!(
                                $snark_inst::verify_batch(universal_verifier, &fs_parameters, &vks_to_inputs, &proof).unwrap(),
                                "Batch verification failed with {instance_batch_size} instances and {circuit_batch_size} circuits for circuits: {constraints:?}"
                            );
                            println!("Called verifier");
                            eprintln!("\nShould not verify (i.e. verifier messages should print below):");
                            let mut fake_instance_inputs = Vec::with_capacity(vks_to_inputs.len());
                            for instance_input in vks_to_inputs.values() {
                                let mut fake_instance_input = Vec::with_capacity(instance_input.len());
                                for input in instance_input.iter() {
                                    let mut fake_input = input.clone();
                                    fake_input[input.len() - 1] = Fr::rand(rng);
                                    fake_instance_input.push(fake_input);
                                }
                                fake_instance_inputs.push(fake_instance_input);
                            }
                            let mut vks_to_fake_inputs = BTreeMap::new();
                            for (i, vk) in vks_to_inputs.keys().enumerate() {
                                vks_to_fake_inputs.insert(*vk, fake_instance_inputs[i].as_slice());
                            }
                            assert!(
                                !$snark_inst::verify_batch(
                                    universal_verifier,
                                    &fs_parameters,
                                    &vks_to_fake_inputs,
                                    &proof
                                )
                                .unwrap()
                            );
                        }
                    }
                }

                pub(crate) fn test_serde_json(num_constraints: usize, num_variables: usize) {
                    use std::str::FromStr;

                    let rng = &mut TestRng::default();

                    let max_degree = AHPForR1CS::<Fr, $snark_mode>::max_degree(100, 25, 300).unwrap();
                    let universal_srs = $snark_inst::universal_setup(max_degree).unwrap();

                    let mul_depth = 1;
                    let (circ, _) = TestCircuit::gen_rand(mul_depth, num_constraints, num_variables, rng);

                    let (_index_pk, index_vk) = $snark_inst::circuit_setup(&universal_srs, &circ).unwrap();
                    println!("Called circuit setup");

                    // Serialize
                    let expected_string = index_vk.to_string();
                    let candidate_string = serde_json::to_string(&index_vk).unwrap();
                    assert_eq!(
                        expected_string,
                        serde_json::Value::from_str(&candidate_string).unwrap().as_str().unwrap()
                    );

                    // Deserialize
                    assert_eq!(index_vk, CircuitVerifyingKey::from_str(&expected_string).unwrap());
                    assert_eq!(index_vk, serde_json::from_str(&candidate_string).unwrap());
                }

                pub(crate) fn test_bincode(num_constraints: usize, num_variables: usize) {
                    use snarkvm_utilities::{FromBytes, ToBytes};

                    let rng = &mut TestRng::default();

                    let max_degree = AHPForR1CS::<Fr, $snark_mode>::max_degree(100, 25, 300).unwrap();
                    let universal_srs = $snark_inst::universal_setup(max_degree).unwrap();

                    let mul_depth = 1;
                    let (circ, _) = TestCircuit::gen_rand(mul_depth, num_constraints, num_variables, rng);

                    let (_index_pk, index_vk) = $snark_inst::circuit_setup(&universal_srs, &circ).unwrap();
                    println!("Called circuit setup");

                    // Serialize
                    let expected_bytes = index_vk.to_bytes_le().unwrap();
                    let candidate_bytes = bincode::serialize(&index_vk).unwrap();
                    // TODO (howardwu): Serialization - Handle the inconsistency between ToBytes and Serialize (off by a length encoding).
                    assert_eq!(&expected_bytes[..], &candidate_bytes[8..]);

                    // Deserialize
                    assert_eq!(index_vk, CircuitVerifyingKey::read_le(&expected_bytes[..]).unwrap());
                    assert_eq!(index_vk, bincode::deserialize(&candidate_bytes[..]).unwrap());
                }
            }
        };
    }

    impl_varuna_test!(SonicPCTest, VarunaSonicInst, VarunaHidingMode);
    impl_varuna_test!(SonicPCPoswTest, VarunaSonicPoSWInst, VarunaNonHidingMode);

    #[test]
    fn prove_and_verify_with_tall_matrix_big() {
        let num_constraints = 100;
        let num_variables = 25;
        let pk_size_zk = 91971;
        let pk_size_posw = 91633;

        SonicPCTest::test_circuit(num_constraints, num_variables, pk_size_zk);
        SonicPCPoswTest::test_circuit(num_constraints, num_variables, pk_size_posw);

        SonicPCTest::test_serde_json(num_constraints, num_variables);
        SonicPCPoswTest::test_serde_json(num_constraints, num_variables);

        SonicPCTest::test_bincode(num_constraints, num_variables);
        SonicPCPoswTest::test_bincode(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_tall_matrix_small() {
        let num_constraints = 26;
        let num_variables = 25;
        let pk_size_zk = 25428;
        let pk_size_posw = 25090;

        SonicPCTest::test_circuit(num_constraints, num_variables, pk_size_zk);
        SonicPCPoswTest::test_circuit(num_constraints, num_variables, pk_size_posw);

        SonicPCTest::test_serde_json(num_constraints, num_variables);
        SonicPCPoswTest::test_serde_json(num_constraints, num_variables);

        SonicPCTest::test_bincode(num_constraints, num_variables);
        SonicPCPoswTest::test_bincode(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_squat_matrix_big() {
        let num_constraints = 25;
        let num_variables = 100;
        let pk_size_zk = 53523;
        let pk_size_posw = 53185;

        SonicPCTest::test_circuit(num_constraints, num_variables, pk_size_zk);
        SonicPCPoswTest::test_circuit(num_constraints, num_variables, pk_size_posw);

        SonicPCTest::test_serde_json(num_constraints, num_variables);
        SonicPCPoswTest::test_serde_json(num_constraints, num_variables);

        SonicPCTest::test_bincode(num_constraints, num_variables);
        SonicPCPoswTest::test_bincode(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_squat_matrix_small() {
        let num_constraints = 25;
        let num_variables = 26;
        let pk_size_zk = 25284;
        let pk_size_posw = 24946;

        SonicPCTest::test_circuit(num_constraints, num_variables, pk_size_zk);
        SonicPCPoswTest::test_circuit(num_constraints, num_variables, pk_size_posw);

        SonicPCTest::test_serde_json(num_constraints, num_variables);
        SonicPCPoswTest::test_serde_json(num_constraints, num_variables);

        SonicPCTest::test_bincode(num_constraints, num_variables);
        SonicPCPoswTest::test_bincode(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_square_matrix() {
        let num_constraints = 25;
        let num_variables = 25;
        let pk_size_zk = 25284;
        let pk_size_posw = 24946;

        SonicPCTest::test_circuit(num_constraints, num_variables, pk_size_zk);
        SonicPCPoswTest::test_circuit(num_constraints, num_variables, pk_size_posw);

        SonicPCTest::test_serde_json(num_constraints, num_variables);
        SonicPCPoswTest::test_serde_json(num_constraints, num_variables);

        SonicPCTest::test_bincode(num_constraints, num_variables);
        SonicPCPoswTest::test_bincode(num_constraints, num_variables);
    }
}

#[cfg(any(test, feature = "test"))]
mod varuna_hiding {
    use crate::{
        crypto_hash::PoseidonSponge,
        snark::varuna::{
            ahp::AHPForR1CS,
            test_circuit::TestCircuit,
            CircuitVerifyingKey,
            VarunaHidingMode,
            VarunaSNARK,
        },
        traits::{AlgebraicSponge, SNARK},
    };
    use snarkvm_curves::bls12_377::{Bls12_377, Fq, Fr};
    use snarkvm_utilities::{
        rand::{TestRng, Uniform},
        FromBytes,
        ToBytes,
    };

    use std::str::FromStr;

    type VarunaInst = VarunaSNARK<Bls12_377, FS, VarunaHidingMode>;
    type FS = PoseidonSponge<Fq, 2, 1>;

    fn test_circuit_n_times(num_constraints: usize, num_variables: usize, num_times: usize) {
        let rng = &mut TestRng::default();

        let max_degree = AHPForR1CS::<Fr, VarunaHidingMode>::max_degree(100, 25, 300).unwrap();
        let universal_srs = VarunaInst::universal_setup(max_degree).unwrap();
        let universal_prover = &universal_srs.to_universal_prover().unwrap();
        let universal_verifier = &universal_srs.to_universal_verifier().unwrap();
        let fs_parameters = FS::sample_parameters();

        for _ in 0..num_times {
            let mul_depth = 2;
            let (circuit, public_inputs) = TestCircuit::gen_rand(mul_depth, num_constraints, num_variables, rng);
            let mut fake_inputs = public_inputs.clone();
            fake_inputs[public_inputs.len() - 1] = Fr::rand(rng);

            let (index_pk, index_vk) = VarunaInst::circuit_setup(&universal_srs, &circuit).unwrap();
            println!("Called circuit setup");

            let proof = VarunaInst::prove(universal_prover, &fs_parameters, &index_pk, &circuit, rng).unwrap();
            println!("Called prover");

            assert!(VarunaInst::verify(universal_verifier, &fs_parameters, &index_vk, public_inputs, &proof).unwrap());
            println!("Called verifier");
            eprintln!("\nShould not verify (i.e. verifier messages should print below):");
            assert!(!VarunaInst::verify(universal_verifier, &fs_parameters, &index_vk, fake_inputs, &proof).unwrap());
        }
    }

    fn test_circuit(num_constraints: usize, num_variables: usize) {
        test_circuit_n_times(num_constraints, num_variables, 100)
    }

    fn test_serde_json(num_constraints: usize, num_variables: usize) {
        let rng = &mut TestRng::default();

        let max_degree = AHPForR1CS::<Fr, VarunaHidingMode>::max_degree(100, 25, 300).unwrap();
        let universal_srs = VarunaInst::universal_setup(max_degree).unwrap();

        let mul_depth = 1;
        let (circuit, _) = TestCircuit::gen_rand(mul_depth, num_constraints, num_variables, rng);

        let (_index_pk, index_vk) = VarunaInst::circuit_setup(&universal_srs, &circuit).unwrap();
        println!("Called circuit setup");

        // Serialize
        let expected_string = index_vk.to_string();
        let candidate_string = serde_json::to_string(&index_vk).unwrap();
        assert_eq!(expected_string, serde_json::Value::from_str(&candidate_string).unwrap().as_str().unwrap());

        // Deserialize
        assert_eq!(index_vk, CircuitVerifyingKey::from_str(&expected_string).unwrap());
        assert_eq!(index_vk, serde_json::from_str(&candidate_string).unwrap());
    }

    fn test_bincode(num_constraints: usize, num_variables: usize) {
        let rng = &mut TestRng::default();

        let max_degree = AHPForR1CS::<Fr, VarunaHidingMode>::max_degree(100, 25, 300).unwrap();
        let universal_srs = VarunaInst::universal_setup(max_degree).unwrap();

        let mul_depth = 1;
        let (circuit, _) = TestCircuit::gen_rand(mul_depth, num_constraints, num_variables, rng);

        let (_index_pk, index_vk) = VarunaInst::circuit_setup(&universal_srs, &circuit).unwrap();
        println!("Called circuit setup");

        // Serialize
        let expected_bytes = index_vk.to_bytes_le().unwrap();
        let candidate_bytes = bincode::serialize(&index_vk).unwrap();
        // TODO (howardwu): Serialization - Handle the inconsistency between ToBytes and Serialize (off by a length encoding).
        assert_eq!(&expected_bytes[..], &candidate_bytes[8..]);

        // Deserialize
        assert_eq!(index_vk, CircuitVerifyingKey::read_le(&expected_bytes[..]).unwrap());
        assert_eq!(index_vk, bincode::deserialize(&candidate_bytes[..]).unwrap());
    }

    #[test]
    fn prove_and_verify_with_tall_matrix_big() {
        let num_constraints = 100;
        let num_variables = 25;

        test_circuit(num_constraints, num_variables);
        test_serde_json(num_constraints, num_variables);
        test_bincode(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_tall_matrix_small() {
        let num_constraints = 26;
        let num_variables = 25;

        test_circuit(num_constraints, num_variables);
        test_serde_json(num_constraints, num_variables);
        test_bincode(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_squat_matrix_big() {
        let num_constraints = 25;
        let num_variables = 100;

        test_circuit(num_constraints, num_variables);
        test_serde_json(num_constraints, num_variables);
        test_bincode(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_squat_matrix_small() {
        let num_constraints = 25;
        let num_variables = 26;

        test_circuit(num_constraints, num_variables);
        test_serde_json(num_constraints, num_variables);
        test_bincode(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_square_matrix() {
        let num_constraints = 25;
        let num_variables = 25;

        test_circuit(num_constraints, num_variables);
        test_serde_json(num_constraints, num_variables);
        test_bincode(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_large_matrix() {
        let num_constraints = 1 << 16;
        let num_variables = 1 << 16;

        test_circuit_n_times(num_constraints, num_variables, 1);
    }

    #[test]
    fn check_indexing() {
        let rng = &mut TestRng::default();
        let mul_depth = 2;
        let num_constraints = 1 << 13;
        let num_variables = 1 << 13;
        let (circuit, public_inputs) = TestCircuit::gen_rand(mul_depth, num_constraints, num_variables, rng);

        let max_degree = AHPForR1CS::<Fr, VarunaHidingMode>::max_degree(100, 25, 300).unwrap();
        let universal_srs = VarunaInst::universal_setup(max_degree).unwrap();
        let universal_prover = &universal_srs.to_universal_prover().unwrap();
        let universal_verifier = &universal_srs.to_universal_verifier().unwrap();
        let fs_parameters = FS::sample_parameters();

        let (index_pk, index_vk) = VarunaInst::circuit_setup(&universal_srs, &circuit).unwrap();
        println!("Called circuit setup");

        let proof = VarunaInst::prove(universal_prover, &fs_parameters, &index_pk, &circuit, rng).unwrap();
        println!("Called prover");

        universal_srs.download_powers_for(0..2usize.pow(18)).unwrap();
        let (new_pk, new_vk) = VarunaInst::circuit_setup(&universal_srs, &circuit).unwrap();
        assert_eq!(index_pk, new_pk);
        assert_eq!(index_vk, new_vk);
        assert!(
            VarunaInst::verify(universal_verifier, &fs_parameters, &index_vk, public_inputs.clone(), &proof).unwrap()
        );
        assert!(VarunaInst::verify(universal_verifier, &fs_parameters, &new_vk, public_inputs, &proof).unwrap());
    }

    #[test]
    fn test_srs_downloads() {
        let rng = &mut TestRng::default();

        let max_degree = AHPForR1CS::<Fr, VarunaHidingMode>::max_degree(100, 25, 300).unwrap();
        let universal_srs = VarunaInst::universal_setup(max_degree).unwrap();
        let universal_prover = &universal_srs.to_universal_prover().unwrap();
        let universal_verifier = &universal_srs.to_universal_verifier().unwrap();
        let fs_parameters = FS::sample_parameters();

        // Indexing, proving, and verifying for a circuit with 1 << 15 constraints and 1 << 15 variables.
        let mul_depth = 2;
        let num_constraints = 2usize.pow(15) - 10;
        let num_variables = 2usize.pow(15) - 10;
        let (circuit1, public_inputs1) = TestCircuit::gen_rand(mul_depth, num_constraints, num_variables, rng);
        let (pk1, vk1) = VarunaInst::circuit_setup(&universal_srs, &circuit1).unwrap();
        println!("Called circuit setup");

        let proof1 = VarunaInst::prove(universal_prover, &fs_parameters, &pk1, &circuit1, rng).unwrap();
        println!("Called prover");
        assert!(VarunaInst::verify(universal_verifier, &fs_parameters, &vk1, public_inputs1.clone(), &proof1).unwrap());

        /*****************************************************************************/

        // Indexing, proving, and verifying for a circuit with 1 << 19 constraints and 1 << 19 variables.
        let mul_depth = 2;
        let num_constraints = 2usize.pow(19) - 10;
        let num_variables = 2usize.pow(19) - 10;
        let (circuit2, public_inputs2) = TestCircuit::gen_rand(mul_depth, num_constraints, num_variables, rng);
        let (pk2, vk2) = VarunaInst::circuit_setup(&universal_srs, &circuit2).unwrap();
        println!("Called circuit setup");

        let proof2 = VarunaInst::prove(universal_prover, &fs_parameters, &pk2, &circuit2, rng).unwrap();
        println!("Called prover");
        assert!(VarunaInst::verify(universal_verifier, &fs_parameters, &vk2, public_inputs2, &proof2).unwrap());
        /*****************************************************************************/
        assert!(VarunaInst::verify(universal_verifier, &fs_parameters, &vk1, public_inputs1, &proof1).unwrap());
    }
}

mod varuna_test_vectors {
    use crate::{
        fft::EvaluationDomain,
        snark::varuna::{ahp::verifier, AHPForR1CS, TestCircuit, VarunaNonHidingMode, VarunaSNARK},
        traits::snark::SNARK,
    };
    use snarkvm_curves::bls12_377::{Bls12_377, Fq, Fr};
    use snarkvm_fields::One;
    use std::{collections::BTreeMap, fs, ops::Deref, path::PathBuf, str::FromStr, sync::Arc};

    type FS = crate::crypto_hash::PoseidonSponge<Fq, 2, 1>;
    type MM = VarunaNonHidingMode;
    type VarunaSonicInst = VarunaSNARK<Bls12_377, FS, MM>;

    // Create the path for the `resources` folder.
    fn resources_path(create_dir: bool) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("src");
        path.push("snark");
        path.push("varuna");
        path.push("resources");

        // Create the `resources` folder, if it does not exist.
        if !path.exists() {
            if create_dir {
                fs::create_dir(&path).unwrap_or_else(|_| panic!("Failed to create resources folder: {path:?}"));
            } else {
                panic!("Resources folder does not exist: {path:?}");
            }
        }

        path
    }

    // Create the file path.
    fn test_vector_path(folder: &str, file: &str, circuit: &str, create_dir: bool) -> PathBuf {
        let mut path = resources_path(create_dir);

        // Construct the path where the test data lives.
        path.push(circuit);
        path.push(folder);

        // Create the test folder if it does not exist if specified, otherwise panic.
        if !path.exists() {
            if create_dir {
                fs::create_dir(&path).unwrap_or_else(|_| panic!("Failed to create resources folder: {path:?}"));
            } else {
                panic!("Resources folder does not exist: {path:?}");
            }
        }

        // Construct the path for the test file.
        path.push(file);
        path.set_extension("txt");

        path
    }

    // Loads the given `test_folder/test_file` and asserts the given `candidate` matches the expected values.
    #[track_caller]
    fn assert_test_vector_equality(test_folder: &str, test_file: &str, candidate: &str, circuit: &str) {
        // Get the path to the test file.
        let path = test_vector_path(test_folder, test_file, circuit, false);

        // Assert the test file is equal to the expected value.
        expect_test::expect_file![path].assert_eq(candidate);
    }

    // Create a test vector from a trusted revision of Varuna.
    fn create_test_vector(folder: &str, file: &str, data: &str, circuit: &str) {
        // Get the path to the test file.
        let path = test_vector_path(folder, file, circuit, true);

        // Write the test vector to file.
        fs::write(&path, data).unwrap_or_else(|_| panic!("Failed to write to file: {:?}", path));
    }

    // Tests varuna against the test vectors in all circuits in the resources folder.
    fn test_varuna_with_all_circuits(create_test_vectors: bool) {
        let entries = fs::read_dir(resources_path(create_test_vectors)).expect("Failed to read resources folder");
        entries.into_iter().for_each(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                let circuit = path.file_name().unwrap().to_str().unwrap();
                test_circuit_with_test_vectors(create_test_vectors, circuit);
            }
        });
    }

    // Test Varuna against test vectors for a specific circuit.
    fn test_circuit_with_test_vectors(create_test_vectors: bool, circuit: &str) {
        // Initialize the parts of the witness used in the multiplicative constraints.
        let witness_path = format!("src/snark/varuna/resources/{}/witness.input", circuit);
        let instance_file = fs::read_to_string(witness_path).expect("Could not read the file");
        let witness: Vec<u128> = serde_json::from_str(instance_file.lines().next().unwrap()).unwrap();
        let (a, b) = (witness[0], witness[1]);

        // Initialize challenges from file.
        let challenges_path = format!("src/snark/varuna/resources/{}/challenges.input", circuit);
        let challenges_file = fs::read_to_string(challenges_path).expect("Could not read the file");
        let mut challenges = Vec::new();
        for line in challenges_file.lines() {
            challenges.push(line)
        }
        let (alpha, _eta_a, eta_b, eta_c, beta, delta_a, delta_b, delta_c, _gamma) = (
            Fr::from_str(challenges[0]).unwrap(),
            Fr::from_str(challenges[1]).unwrap(),
            Fr::from_str(challenges[2]).unwrap(),
            Fr::from_str(challenges[3]).unwrap(),
            Fr::from_str(challenges[4]).unwrap(),
            vec![Fr::from_str(challenges[5]).unwrap()],
            vec![Fr::from_str(challenges[6]).unwrap()],
            vec![Fr::from_str(challenges[7]).unwrap()],
            Fr::from_str(challenges[8]).unwrap(),
        );

        let circuit_combiner = Fr::one();
        let instance_combiners = vec![Fr::one()];

        // Create sample circuit which corresponds to instance.input file.
        let mul_depth = 3;
        let num_constraints = 7;
        let num_variables = 7;

        // Create a fixed seed rng that matches those the test vectors were generated with.
        let rng = &mut snarkvm_utilities::rand::TestRng::fixed(4730);
        let max_degree =
            AHPForR1CS::<Fr, MM>::max_degree(num_constraints, num_variables, num_variables * num_constraints).unwrap();
        let universal_srs = VarunaSonicInst::universal_setup(max_degree).unwrap();
        let (circ, _) =
            TestCircuit::generate_circuit_with_fixed_witness(a, b, mul_depth, num_constraints, num_variables);
        println!("Circuit: {:?}", circ);
        let (index_pk, _index_vk) = VarunaSonicInst::circuit_setup(&universal_srs, &circ).unwrap();
        let mut keys_to_constraints = BTreeMap::new();
        keys_to_constraints.insert(index_pk.circuit.deref(), std::slice::from_ref(&circ));

        // Begin the Varuna protocol execution.
        let prover_state = AHPForR1CS::<_, MM>::init_prover(&keys_to_constraints, rng).unwrap();
        let mut prover_state = AHPForR1CS::<_, MM>::prover_first_round(prover_state, rng).unwrap();
        let first_round_oracles = Arc::new(prover_state.first_round_oracles.as_ref().unwrap());

        // Get private witness polynomial coefficients.
        let (_, w_poly) = first_round_oracles.batches.iter().next().unwrap();
        let w_lde = format!("{:?}", w_poly[0].0.coeffs().map(|(_, coeff)| coeff).collect::<Vec<_>>());
        if create_test_vectors {
            create_test_vector("polynomials", "w_lde", &w_lde, circuit);
        }

        // Generate test vectors from assignments.
        let assignments = AHPForR1CS::<_, MM>::calculate_assignments(&mut prover_state).unwrap();

        // Get full witness polynomial coefficients.
        let (_, z_poly) = assignments.iter().next().unwrap();
        let z_lde = format!("{:?}", z_poly[0].coeffs().iter().collect::<Vec<_>>());
        if create_test_vectors {
            create_test_vector("polynomials", "z_lde", &z_lde, circuit);
        }

        let combiners = verifier::BatchCombiners::<Fr> { circuit_combiner, instance_combiners };
        let batch_combiners = BTreeMap::from_iter([(index_pk.circuit.id, combiners)]);
        let verifier_first_msg = verifier::FirstMessage::<Fr> { batch_combiners };

        let (second_oracles, prover_state) =
            AHPForR1CS::<_, MM>::prover_second_round::<_>(&verifier_first_msg, prover_state, rng).unwrap();

        // Get round 2 rowcheck polynomial oracle coefficients.
        let h_0 = format!("{:?}", second_oracles.h_0.coeffs().map(|(_, coeff)| coeff).collect::<Vec<_>>());
        if create_test_vectors {
            create_test_vector("polynomials", "h_0", &h_0, circuit);
        }

        let verifier_second_msg = verifier::SecondMessage::<Fr> { alpha, eta_b, eta_c };
        let (_prover_third_message, third_oracles, prover_state) =
            AHPForR1CS::<_, MM>::prover_third_round(&verifier_first_msg, &verifier_second_msg, prover_state, rng)
                .unwrap();

        // Get coefficients round 3 univariate rowcheck polynomial oracles.
        let g_1 = format!("{:?}", third_oracles.g_1.coeffs().map(|(_, coeff)| coeff).collect::<Vec<_>>());
        if create_test_vectors {
            create_test_vector("polynomials", "g_1", &g_1, circuit);
        }
        let h_1 = format!("{:?}", third_oracles.h_1.coeffs().map(|(_, coeff)| coeff).collect::<Vec<_>>());
        if create_test_vectors {
            create_test_vector("polynomials", "h_1", &h_1, circuit);
        }

        let verifier_third_msg = verifier::ThirdMessage::<Fr> { beta };
        let (_prover_fourth_message, fourth_oracles, prover_state) =
            AHPForR1CS::<_, MM>::prover_fourth_round(&verifier_second_msg, &verifier_third_msg, prover_state, rng)
                .unwrap();

        // Create round 4 rational sumcheck oracle polynomials.
        let (_, gm_polys) = fourth_oracles.gs.iter().next().unwrap();
        let g_a = format!("{:?}", gm_polys.g_a.coeffs().map(|(_, coeff)| coeff).collect::<Vec<_>>());
        let g_b = format!("{:?}", gm_polys.g_b.coeffs().map(|(_, coeff)| coeff).collect::<Vec<_>>());
        let g_c = format!("{:?}", gm_polys.g_b.coeffs().map(|(_, coeff)| coeff).collect::<Vec<_>>());
        if create_test_vectors {
            create_test_vector("polynomials", "g_a", &g_a, circuit);
            create_test_vector("polynomials", "g_b", &g_b, circuit);
            create_test_vector("polynomials", "g_c", &g_c, circuit);
        }

        // Create the verifier's fourth message.
        let verifier_fourth_msg = verifier::FourthMessage::<Fr> { delta_a, delta_b, delta_c };

        let mut public_inputs = BTreeMap::new();
        let public_input = prover_state.public_inputs(&index_pk.circuit).unwrap();
        public_inputs.insert(index_pk.circuit.id, public_input);
        let non_zero_a_domain = EvaluationDomain::<Fr>::new(index_pk.circuit.index_info.num_non_zero_a).unwrap();
        let non_zero_b_domain = EvaluationDomain::<Fr>::new(index_pk.circuit.index_info.num_non_zero_b).unwrap();
        let non_zero_c_domain = EvaluationDomain::<Fr>::new(index_pk.circuit.index_info.num_non_zero_c).unwrap();
        let variable_domain =
            EvaluationDomain::<Fr>::new(index_pk.circuit.index_info.num_public_and_private_variables).unwrap();
        let constraint_domain = EvaluationDomain::<Fr>::new(index_pk.circuit.index_info.num_constraints).unwrap();
        let input_domain = EvaluationDomain::<Fr>::new(index_pk.circuit.index_info.num_public_inputs).unwrap();

        // Get constraint domain elements.
        let mut constraint_domain_elements = Vec::with_capacity(constraint_domain.size());
        for el in constraint_domain.elements() {
            constraint_domain_elements.push(el);
        }
        if create_test_vectors {
            create_test_vector("domain", "R", &format!("{:?}", constraint_domain_elements), circuit);
        }

        // Get non_zero_domain elements.
        let non_zero_domain = *[&non_zero_a_domain, &non_zero_b_domain, &non_zero_c_domain]
            .iter()
            .max_by_key(|domain| domain.size)
            .unwrap();
        let mut non_zero_domain_elements = Vec::with_capacity(non_zero_domain.size());
        for el in non_zero_domain.elements() {
            non_zero_domain_elements.push(el);
        }
        if create_test_vectors {
            create_test_vector("domain", "K", &format!("{:?}", non_zero_domain_elements), circuit);
        }

        // Get variable domain elements.
        let mut variable_domain_elements = Vec::with_capacity(input_domain.size());
        for el in variable_domain.elements() {
            variable_domain_elements.push(el);
        }
        if create_test_vectors {
            create_test_vector("domain", "C", &format!("{:?}", variable_domain_elements), circuit);
        }

        let fifth_oracles = AHPForR1CS::<_, MM>::prover_fifth_round(verifier_fourth_msg, prover_state, rng).unwrap();

        // Get coefficients of final oracle polynomial from round 5.
        let h_2 = format!("{:?}", fifth_oracles.h_2.coeffs().map(|(_, coeff)| coeff).collect::<Vec<_>>());
        if create_test_vectors {
            create_test_vector("polynomials", "h_2", &h_2, circuit);
        }

        // Check the intermediate oracle polynomials against the test vectors.
        assert_test_vector_equality("polynomials", "w_lde", &w_lde, circuit);
        assert_test_vector_equality("polynomials", "z_lde", &z_lde, circuit);
        assert_test_vector_equality("polynomials", "h_0", &h_0, circuit);
        assert_test_vector_equality("polynomials", "h_1", &h_1, circuit);
        assert_test_vector_equality("polynomials", "g_1", &g_1, circuit);
        assert_test_vector_equality("polynomials", "h_2", &h_2, circuit);
        assert_test_vector_equality("polynomials", "g_a", &g_a, circuit);
        assert_test_vector_equality("polynomials", "g_b", &g_b, circuit);
        assert_test_vector_equality("polynomials", "g_c", &g_c, circuit);

        // Check that the domains match the test vectors.
        assert_test_vector_equality("domain", "R", &format!("{:?}", constraint_domain_elements), circuit);
        assert_test_vector_equality("domain", "K", &format!("{:?}", non_zero_domain_elements), circuit);
        assert_test_vector_equality("domain", "C", &format!("{:?}", variable_domain_elements), circuit);
    }

    #[test]
    fn test_varuna_with_prover_test_vectors() {
        test_varuna_with_all_circuits(false);
    }
}
