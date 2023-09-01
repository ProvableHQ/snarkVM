// Copyright (C) 2019-2023 Aleo Systems Inc.
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

use crate::{
    snark::varuna::TestCircuit,
    traits::{AlgebraicSponge, SNARK},
};
use std::collections::BTreeMap;

mod varuna {
    use super::*;
    use crate::snark::varuna::{
        mode::SNARKMode,
        AHPForR1CS,
        CircuitVerifyingKey,
        VarunaHidingMode,
        VarunaNonHidingMode,
        VarunaSNARK,
    };
    use itertools::Itertools;
    use snarkvm_curves::bls12_377::{Bls12_377, Fq, Fr};
    use snarkvm_utilities::{
        rand::{TestRng, Uniform},
        ToBytes,
    };
    use std::collections::HashSet;

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

                    let universal_verifier = &universal_srs.to_universal_verifier().unwrap();
                    let fs_parameters = FS::sample_parameters();

                    let hiding_bound = AHPForR1CS::<Fr, $snark_mode>::zk_bound().unwrap_or(0);

                    for i in 0..5 {
                        let mul_depth = 1;
                        println!("running test with MM::ZK: {}, mul_depth: {}, num_constraints: {}, num_variables: {}", $snark_mode::ZK, mul_depth + i, num_constraints + i, num_variables + i);
                        let (circ, public_inputs) = TestCircuit::gen_rand(mul_depth + i, num_constraints + i, num_variables + i, rng);

                        let (index_pk, index_vk) = $snark_inst::circuit_setup(&universal_srs, &circ).unwrap();
                        println!("Called circuit setup");

                        let max_domain_size = index_pk.circuit.max_domain_size();
                        let max_degree = index_pk.circuit.max_degree();
                        let coefficient_support = index_pk.circuit.index_info.get_degree_bounds::<Fr>();
                        let universal_prover = &universal_srs.to_universal_prover(
                            max_degree,
                            max_domain_size,
                            Some(&coefficient_support),
                            None,
                            hiding_bound,
                        ).unwrap();

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
                        assert!(!$snark_inst::verify(universal_verifier, &fs_parameters, &index_vk, [random, random], &proof).unwrap());
                    }

                    for circuit_batch_size in (0..4).map(|i| 2usize.pow(i)) {
                        for instance_batch_size in (0..4).map(|i| 2usize.pow(i)) {
                            println!("running test with circuit_batch_size: {circuit_batch_size} and instance_batch_size: {instance_batch_size}");
                            let mut constraints = BTreeMap::new();
                            let mut inputs = BTreeMap::new();
                            let mut max_degree = 0;
                            let mut max_domain_size = 0;
                            let mut coefficient_support = HashSet::new();

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
                                let max_degree_i = index_pk.circuit.max_degree();
                                let max_domain_size_i = index_pk.circuit.max_domain_size();
                                let coefficient_support_i = index_pk.circuit.index_info.get_degree_bounds::<Fr>();
                                let universal_prover = &universal_srs.to_universal_prover(
                                    max_degree_i,
                                    max_domain_size_i,
                                    Some(&coefficient_support_i),
                                    None,
                                    hiding_bound,
                                ).unwrap();

                                let certificate = $snark_inst::prove_vk(universal_prover, &fs_parameters, &index_vk, &index_pk).unwrap();
                                let circuits = constraints[&index_pk.circuit.id].as_slice();
                                assert!($snark_inst::verify_vk(universal_verifier, &fs_parameters, &circuits[0], &index_vk, &certificate).unwrap());
                                pks_to_constraints.insert(index_pk, circuits);
                                vks_to_inputs.insert(index_vk, inputs[&index_pk.circuit.id].as_slice());

                                // collect max degree and domain size and union of coefficient supports
                                max_degree = max_degree.max(max_degree_i);
                                max_domain_size = max_domain_size.max(max_domain_size_i);
                                for coeff in coefficient_support_i {
                                    coefficient_support.insert(coeff);
                                }
                            }
                            println!("verified vks");

                            let coefficient_support = coefficient_support.drain().collect_vec();
                            let universal_prover = &universal_srs.to_universal_prover(
                                max_degree,
                                max_domain_size,
                                Some(coefficient_support.as_ref()),
                                None,
                                hiding_bound,
                            ).unwrap();

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
                                    let fake_input = vec![Fr::rand(rng); input.len()];
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
        let pk_size_zk = 53767;
        let pk_size_posw = 53623;

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
        let pk_size_zk = 15463;
        let pk_size_posw = 15319;

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
        let pk_size_zk = 15319;
        let pk_size_posw = 15175;

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
        let pk_size_zk = 15319;
        let pk_size_posw = 15175;

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
        let pk_size_zk = 15319;
        let pk_size_posw = 15175;

        SonicPCTest::test_circuit(num_constraints, num_variables, pk_size_zk);
        SonicPCPoswTest::test_circuit(num_constraints, num_variables, pk_size_posw);

        SonicPCTest::test_serde_json(num_constraints, num_variables);
        SonicPCPoswTest::test_serde_json(num_constraints, num_variables);

        SonicPCTest::test_bincode(num_constraints, num_variables);
        SonicPCPoswTest::test_bincode(num_constraints, num_variables);
    }
}

mod varuna_hiding {
    use super::*;
    use crate::{
        crypto_hash::PoseidonSponge,
        snark::varuna::{ahp::AHPForR1CS, CircuitVerifyingKey, VarunaHidingMode, VarunaSNARK},
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
        let universal_verifier = &universal_srs.to_universal_verifier().unwrap();
        let fs_parameters = FS::sample_parameters();
        let hiding_bound = AHPForR1CS::<Fr, VarunaHidingMode>::zk_bound().unwrap_or(0);

        for _ in 0..num_times {
            let mul_depth = 2;
            let (circuit, public_inputs) = TestCircuit::gen_rand(mul_depth, num_constraints, num_variables, rng);

            let (index_pk, index_vk) = VarunaInst::circuit_setup(&universal_srs, &circuit).unwrap();
            println!("Called circuit setup");

            let max_degree = index_pk.circuit.max_degree();
            let max_domain_size = index_pk.circuit.max_domain_size();
            let coefficient_support = index_pk.circuit.index_info.get_degree_bounds::<Fr>();
            let universal_prover = &universal_srs
                .to_universal_prover(max_degree, max_domain_size, Some(&coefficient_support), None, hiding_bound)
                .unwrap();

            let proof = VarunaInst::prove(universal_prover, &fs_parameters, &index_pk, &circuit, rng).unwrap();
            println!("Called prover");

            assert!(VarunaInst::verify(universal_verifier, &fs_parameters, &index_vk, public_inputs, &proof).unwrap());
            println!("Called verifier");
            eprintln!("\nShould not verify (i.e. verifier messages should print below):");
            assert!(
                !VarunaInst::verify(
                    universal_verifier,
                    &fs_parameters,
                    &index_vk,
                    [Fr::rand(rng), Fr::rand(rng)],
                    &proof
                )
                .unwrap()
            );
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
        let universal_verifier = &universal_srs.to_universal_verifier().unwrap();
        let fs_parameters = FS::sample_parameters();
        let hiding_bound = AHPForR1CS::<Fr, VarunaHidingMode>::zk_bound().unwrap_or(0);

        let (index_pk, index_vk) = VarunaInst::circuit_setup(&universal_srs, &circuit).unwrap();
        println!("Called circuit setup");

        let max_degree = index_pk.circuit.max_degree();
        let max_domain_size = index_pk.circuit.max_domain_size();
        let coefficient_support = index_pk.circuit.index_info.get_degree_bounds::<Fr>();
        let universal_prover = &universal_srs
            .to_universal_prover(max_degree, max_domain_size, Some(&coefficient_support), None, hiding_bound)
            .unwrap();

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
        let universal_verifier = &universal_srs.to_universal_verifier().unwrap();
        let fs_parameters = FS::sample_parameters();
        let hiding_bound = AHPForR1CS::<Fr, VarunaHidingMode>::zk_bound().unwrap_or(0);

        // Indexing, proving, and verifying for a circuit with 1 << 15 constraints and 1 << 15 variables.
        let mul_depth = 2;
        let num_constraints = 2usize.pow(15) - 10;
        let num_variables = 2usize.pow(15) - 10;
        let (circuit1, public_inputs1) = TestCircuit::gen_rand(mul_depth, num_constraints, num_variables, rng);
        let (pk1, vk1) = VarunaInst::circuit_setup(&universal_srs, &circuit1).unwrap();
        println!("Called circuit setup");

        let max_degree = pk1.circuit.max_degree();
        let max_domain_size = pk1.circuit.max_domain_size();
        let coefficient_support = pk1.circuit.index_info.get_degree_bounds::<Fr>();
        let universal_prover = &universal_srs
            .to_universal_prover(max_degree, max_domain_size, Some(&coefficient_support), None, hiding_bound)
            .unwrap();

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

        let max_degree = pk2.circuit.max_degree();
        let max_domain_size = pk2.circuit.max_domain_size();
        let coefficient_support = pk2.circuit.index_info.get_degree_bounds::<Fr>();
        let universal_prover = &universal_srs
            .to_universal_prover(max_degree, max_domain_size, Some(&coefficient_support), None, hiding_bound)
            .unwrap();

        let proof2 = VarunaInst::prove(universal_prover, &fs_parameters, &pk2, &circuit2, rng).unwrap();
        println!("Called prover");
        assert!(VarunaInst::verify(universal_verifier, &fs_parameters, &vk2, public_inputs2, &proof2).unwrap());
        /*****************************************************************************/
        assert!(VarunaInst::verify(universal_verifier, &fs_parameters, &vk1, public_inputs1, &proof1).unwrap());
    }
}