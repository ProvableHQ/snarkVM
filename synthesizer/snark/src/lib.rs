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

#![forbid(unsafe_code)]
#![allow(clippy::too_many_arguments)]
#![warn(clippy::cast_possible_truncation)]
#![cfg_attr(not(feature = "aleo-cli"), allow(unused_variables))]

use console::network::{prelude::*, FiatShamir};
use snarkvm_algorithms::{snark::varuna, traits::SNARK};

use once_cell::sync::OnceCell;
use std::sync::Arc;

#[cfg(feature = "aleo-cli")]
use colored::Colorize;

type Varuna<N> = varuna::VarunaSNARK<<N as Environment>::PairingCurve, FiatShamir<N>, varuna::VarunaHidingMode>;

mod certificate;
pub use certificate::Certificate;

mod proof;
pub use proof::Proof;

mod proving_key;
pub use proving_key::ProvingKey;

mod universal_srs;
pub use universal_srs::UniversalSRS;

mod verifying_key;
pub use verifying_key::VerifyingKey;

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use circuit::{
        environment::{Assignment, Circuit, Eject, Environment, Inject, Mode, One},
        types::Field,
    };
    use console::{network::MainnetV0, prelude::One as _};

    use once_cell::sync::OnceCell;

    type CurrentNetwork = MainnetV0;

    /// Compute 2^EXPONENT - 1, in a purposefully constraint-inefficient manner for testing.
    fn create_example_circuit<E: Environment>() -> Field<E> {
        let one = console::types::Field::<E::Network>::one();
        let two = one + one;

        const EXPONENT: u64 = 64;

        // Compute 2^EXPONENT - 1, in a purposefully constraint-inefficient manner for testing.
        let mut candidate = Field::<E>::new(Mode::Public, one);
        let mut accumulator = Field::new(Mode::Private, two);
        for _ in 0..EXPONENT {
            candidate += &accumulator;
            accumulator *= Field::new(Mode::Private, two);
        }

        assert_eq!((accumulator - Field::one()).eject_value(), candidate.eject_value());
        assert_eq!(2, E::num_public());
        assert_eq!(2 * EXPONENT + 1, E::num_private());
        assert_eq!(EXPONENT, E::num_constraints());
        assert!(E::is_satisfied());

        candidate
    }

    /// Returns a sample assignment for the example circuit.
    pub(crate) fn sample_assignment() -> Assignment<<Circuit as Environment>::BaseField> {
        static INSTANCE: OnceCell<Assignment<<Circuit as Environment>::BaseField>> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                let _candidate_output = create_example_circuit::<Circuit>();
                let assignment = Circuit::eject_assignment_and_reset();
                assert_eq!(0, Circuit::num_constants());
                assert_eq!(1, Circuit::num_public());
                assert_eq!(0, Circuit::num_private());
                assert_eq!(0, Circuit::num_constraints());
                assignment
            })
            .clone()
    }

    /// Returns the sample circuit keys for the example circuit.
    pub(crate) fn sample_keys() -> (ProvingKey<CurrentNetwork>, VerifyingKey<CurrentNetwork>) {
        static INSTANCE: OnceCell<(ProvingKey<CurrentNetwork>, VerifyingKey<CurrentNetwork>)> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                let assignment = sample_assignment();
                let srs = UniversalSRS::load().unwrap();
                let (proving_key, verifying_key) = srs.to_circuit_key("test", &assignment).unwrap();
                (proving_key, verifying_key)
            })
            .clone()
    }

    /// Returns a sample proof for the example circuit.
    pub(crate) fn sample_proof() -> Proof<CurrentNetwork> {
        static INSTANCE: OnceCell<Proof<CurrentNetwork>> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                let assignment = sample_assignment();
                let (proving_key, _) = sample_keys();
                proving_key.prove("test", &assignment, &mut TestRng::default()).unwrap()
            })
            .clone()
    }

    /// Returns a sample certificate for the example circuit.
    pub(super) fn sample_certificate() -> Certificate<CurrentNetwork> {
        static INSTANCE: OnceCell<Certificate<CurrentNetwork>> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                let (proving_key, verifying_key) = sample_keys();
                // Return the certificate.
                Certificate::certify("test", &proving_key, &verifying_key).unwrap()
            })
            .clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use circuit::environment::{Circuit, Environment};
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_varuna() {
        let assignment = crate::test_helpers::sample_assignment();

        // Varuna setup, prove, and verify.
        let srs = UniversalSRS::<CurrentNetwork>::load().unwrap();
        let (proving_key, verifying_key) = srs.to_circuit_key("test", &assignment).unwrap();
        println!("Called circuit setup");

        let proof = proving_key.prove("test", &assignment, &mut TestRng::default()).unwrap();
        println!("Called prover");

        let one = <Circuit as Environment>::BaseField::one();
        assert!(verifying_key.verify("test", &[one, one], &proof));
        println!("Called verifier");
        println!("\nShould not verify (i.e. verifier messages should print below):");
        assert!(!verifying_key.verify("test", &[one, one + one], &proof));
    }

    #[test]
    fn test_varuna_verify_public_input_size() {
        /// Creates a simple circuit: a * b.
        fn create_assignment() -> circuit::Assignment<<CurrentNetwork as console::prelude::Environment>::Field> {
            use circuit::{environment::Mode, types::Field, Inject};

            // Ensure the circuit environment is clean.
            Circuit::reset();

            // Inject a field element.
            let console_field = console::types::Field::<CurrentNetwork>::one();
            let circuit_field_0 = Field::<Circuit>::new(Mode::Private, console_field);

            // Inject another field element.
            let console_field = console_field.double();
            let circuit_field_1 = Field::<Circuit>::new(Mode::Private, console_field);

            // Multiply the two field elements.
            let _circuit_field_2 = circuit_field_0 * circuit_field_1;

            // Eject the assignment.
            Circuit::eject_assignment_and_reset()
        }

        let assignment = create_assignment();
        assert_eq!(assignment.num_public(), 1);
        assert_eq!(assignment.num_private(), 3);

        let srs = UniversalSRS::<CurrentNetwork>::load().unwrap();
        let (proving_key, verifying_key) = srs.to_circuit_key("test", &assignment).unwrap();
        println!("Called circuit setup");

        let proof = proving_key.prove("test", &assignment, &mut TestRng::default()).unwrap();
        println!("Called prover");

        // Should pass.
        let one = <Circuit as Environment>::BaseField::one();
        assert!(verifying_key.verify("test", &[one], &proof));

        // Should fail.
        assert!(!verifying_key.verify("test", &[one, one], &proof));
        assert!(!verifying_key.verify("test", &[one, one + one], &proof));
        assert!(!verifying_key.verify("test", &[one, one, one], &proof));
        assert!(!verifying_key.verify("test", &[one, one, one + one], &proof));
        assert!(!verifying_key.verify("test", &[one, one, one, one], &proof));

        println!("Called verifier");
    }
}
