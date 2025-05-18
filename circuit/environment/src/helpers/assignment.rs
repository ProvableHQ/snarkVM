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

use crate::{Constraint, LinearCombination, Variable};
use snarkvm_fields::PrimeField;

use indexmap::IndexMap;
use std::sync::Arc;

use super::R1CS;

/// A struct that contains public variable assignments, private variable assignments,
/// and constraint assignments.
#[derive(Clone, Debug)]
pub struct Assignment<F: PrimeField> {
    /// The public variables.
    public: Arc<[Variable<F>]>,
    /// The private variables.
    private: Arc<[Variable<F>]>,
    /// The constraints.
    constraints: Arc<[Arc<Constraint<F>>]>,
    /// The number of constants, public, and private variables in the assignment.
    num_variables: u64,
}

impl<F: PrimeField> From<crate::R1CS<F>> for Assignment<F> {
    /// Converts an R1CS to an assignment.
    fn from(r1cs: crate::R1CS<F>) -> Self {
        #[cfg(feature = "save_r1cs_hashes")]
        r1cs.save_hash();

        let R1CS { public, private, constraints, num_variables, .. } = r1cs;

        Self { public: public.into(), private: private.into(), constraints: constraints.into(), num_variables }
    }
}

impl<F: PrimeField> Assignment<F> {
    /// Returns the public inputs of the assignment.
    pub const fn public_inputs(&self) -> &Arc<[Variable<F>]> {
        &self.public
    }

    /// Returns the private inputs of the assignment.
    pub const fn private_inputs(&self) -> &Arc<[Variable<F>]> {
        &self.private
    }

    /// Returns the constraints of the assignment.
    pub const fn constraints(&self) -> &Arc<[Arc<Constraint<F>>]> {
        &self.constraints
    }

    /// Returns the number of public variables in the assignment.
    pub fn num_public(&self) -> u64 {
        self.public.len() as u64
    }

    /// Returns the number of private variables in the assignment.
    pub fn num_private(&self) -> u64 {
        self.private.len() as u64
    }

    /// Returns the number of constants, public, and private variables in the assignment.
    pub fn num_variables(&self) -> u64 {
        self.num_variables
    }

    /// Returns the number of constraints in the assignment.
    pub fn num_constraints(&self) -> u64 {
        self.constraints.len() as u64
    }

    /// Returns the number of nonzeros in the assignment.
    pub fn num_nonzeros(&self) -> (u64, u64, u64) {
        self.constraints
            .iter()
            .map(|constraint| {
                let (a, b, c) = constraint.to_terms();
                (a.num_nonzeros(), b.num_nonzeros(), c.num_nonzeros())
            })
            .fold((0, 0, 0), |(a, b, c), (x, y, z)| (a.saturating_add(x), b.saturating_add(y), c.saturating_add(z)))
    }
}

impl<F: PrimeField> snarkvm_algorithms::r1cs::ConstraintSynthesizer<F> for Assignment<F> {
    /// Synthesizes the constraints from the environment into a `snarkvm_algorithms::r1cs`-compliant constraint system.
    fn generate_constraints<CS: snarkvm_algorithms::r1cs::ConstraintSystem<F>>(
        &self,
        cs: &mut CS,
    ) -> Result<(), snarkvm_algorithms::r1cs::SynthesisError> {
        /// A struct for tracking the mapping of variables from the virtual machine (first) to the gadget constraint system (second).
        struct Converter {
            public: IndexMap<u64, snarkvm_algorithms::r1cs::Variable>,
            private: IndexMap<u64, snarkvm_algorithms::r1cs::Variable>,
        }

        let mut converter = Converter { public: Default::default(), private: Default::default() };

        // Ensure the given `cs` is starting off clean.
        assert_eq!(1, cs.num_public_variables());
        assert_eq!(0, cs.num_private_variables());
        assert_eq!(0, cs.num_constraints());

        let result = converter.public.insert(0, CS::one());
        assert!(result.is_none(), "Overwrote an existing public variable in the converter");

        // Allocate the public variables.
        // NOTE: we skip the first public `One` variable because we already allocated it in the `ConstraintSystem` constructor.
        for (i, variable) in self.public.iter().skip(1).enumerate() {
            let (index, value) = variable.index_value();
            assert_eq!((i + 1) as u64, index, "Public vars in first system must be processed in lexicographic order");

            let gadget = cs.alloc_input(|| format!("Public {i}"), || Ok(value))?;

            assert_eq!(
                snarkvm_algorithms::r1cs::Index::Public(index as usize),
                gadget.get_unchecked(),
                "Public variables in the second system must match the first system (with an off-by-1 for the public case)"
            );

            let result = converter.public.insert(index, gadget);

            assert!(result.is_none(), "Overwrote an existing public variable in the converter");
        }

        // Allocate the private variables.
        for (i, variable) in self.private.iter().enumerate() {
            let (index, value) = variable.index_value();
            assert_eq!(i as u64, index, "Private variables in first system must be processed in lexicographic order");

            let gadget = cs.alloc(|| format!("Private {i}"), || Ok(value))?;

            assert_eq!(
                snarkvm_algorithms::r1cs::Index::Private(i),
                gadget.get_unchecked(),
                "Private variables in the second system must match the first system"
            );

            let result = converter.private.insert(index, gadget);

            assert!(result.is_none(), "Overwrote an existing private variable in the converter");
        }

        // Enforce all of the constraints.
        for (i, constraint) in self.constraints.iter().enumerate() {
            let (a, b, c) = constraint.to_terms();
            // Converts terms from one linear combination in the first system to the second system.
            let convert_linear_combination =
                |lc: &LinearCombination<F>| -> snarkvm_algorithms::r1cs::LinearCombination<F> {
                    // Initialize a linear combination for the second system.
                    let mut linear_combination = snarkvm_algorithms::r1cs::LinearCombination::<F>::zero();

                    // Process every term in the linear combination.
                    for (variable, coefficient) in lc.to_terms() {
                        match variable {
                            Variable::Constant(_) => {
                                unreachable!(
                                    "Failed during constraint translation. The first system by definition cannot have constant variables in the terms"
                                )
                            }
                            Variable::Public(index_value) => {
                                let (index, _) = index_value.as_ref();
                                let gadget = converter.public.get(index).unwrap();
                                assert_eq!(
                                    snarkvm_algorithms::r1cs::Index::Public(*index as usize),
                                    gadget.get_unchecked(),
                                    "Failed during constraint translation. The public variable in the second system must match the first system (with an off-by-1 for the public case)"
                                );
                                linear_combination += (*coefficient, *gadget);
                            }
                            Variable::Private(index_value) => {
                                let (index, _) = index_value.as_ref();
                                let gadget = converter.private.get(index).unwrap();
                                assert_eq!(
                                    snarkvm_algorithms::r1cs::Index::Private(*index as usize),
                                    gadget.get_unchecked(),
                                    "Failed during constraint translation. The private variable in the second system must match the first system"
                                );
                                linear_combination += (*coefficient, *gadget);
                            }
                        }
                    }

                    // Finally, add the accumulated constant value to the linear combination.
                    if !lc.to_constant().is_zero() {
                        linear_combination += (
                            lc.to_constant(),
                            snarkvm_algorithms::r1cs::Variable::new_unchecked(snarkvm_algorithms::r1cs::Index::Public(
                                0,
                            )),
                        );
                    }

                    // Return the linear combination of the second system.
                    linear_combination
                };

            cs.enforce(
                || format!("Constraint {i}"),
                |lc| lc + convert_linear_combination(a),
                |lc| lc + convert_linear_combination(b),
                |lc| lc + convert_linear_combination(c),
            );
        }

        // Ensure the given `cs` matches in size with the first system.
        assert_eq!(self.num_public(), cs.num_public_variables() as u64);
        assert_eq!(self.num_private(), cs.num_private_variables() as u64);
        assert_eq!(self.num_constraints(), cs.num_constraints() as u64);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use snarkvm_algorithms::{AlgebraicSponge, SNARK, r1cs::ConstraintSynthesizer, snark::varuna::VarunaVersion};
    use snarkvm_circuit::prelude::*;
    use snarkvm_curves::bls12_377::Fr;

    /// Compute 2^EXPONENT - 1, in a purposefully constraint-inefficient manner for testing.
    fn create_example_circuit<E: Environment>() -> Field<E> {
        let one = snarkvm_console_types::Field::<E::Network>::one();
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

    #[test]
    fn test_constraint_converter() {
        let _candidate_output = create_example_circuit::<Circuit>();
        let assignment = Circuit::eject_assignment_and_reset();
        assert_eq!(0, Circuit::num_constants());
        assert_eq!(1, Circuit::num_public());
        assert_eq!(0, Circuit::num_private());
        assert_eq!(0, Circuit::num_constraints());

        let mut cs = snarkvm_algorithms::r1cs::TestConstraintSystem::new();
        assignment.generate_constraints(&mut cs).unwrap();
        {
            use snarkvm_algorithms::r1cs::ConstraintSystem;
            assert_eq!(assignment.num_public(), cs.num_public_variables() as u64);
            assert_eq!(assignment.num_private(), cs.num_private_variables() as u64);
            assert_eq!(assignment.num_constraints(), cs.num_constraints() as u64);
            assert!(cs.is_satisfied());
        }
    }

    #[test]
    fn test_varuna() {
        let _candidate_output = create_example_circuit::<Circuit>();
        let assignment = Circuit::eject_assignment_and_reset();
        assert_eq!(0, Circuit::num_constants());
        assert_eq!(1, Circuit::num_public());
        assert_eq!(0, Circuit::num_private());
        assert_eq!(0, Circuit::num_constraints());

        // Varuna setup, prove, and verify.

        use snarkvm_algorithms::{
            crypto_hash::PoseidonSponge,
            snark::varuna::{VarunaHidingMode, VarunaSNARK, ahp::AHPForR1CS},
        };
        use snarkvm_curves::bls12_377::{Bls12_377, Fq};
        use snarkvm_utilities::rand::TestRng;

        type FS = PoseidonSponge<Fq, 2, 1>;
        type VarunaInst = VarunaSNARK<Bls12_377, FS, VarunaHidingMode>;

        let rng = &mut TestRng::default();

        let max_degree = AHPForR1CS::<Fr, VarunaHidingMode>::max_degree(200, 200, 300).unwrap();
        let universal_srs = VarunaInst::universal_setup(max_degree).unwrap();
        let universal_prover = &universal_srs.to_universal_prover().unwrap();
        let universal_verifier = &universal_srs.to_universal_verifier().unwrap();
        let fs_pp = FS::sample_parameters();

        let (index_pk, index_vk) = VarunaInst::circuit_setup(&universal_srs, &assignment).unwrap();
        let varuna_version = VarunaVersion::V2;
        println!("Called circuit setup");

        let proof = VarunaInst::prove(universal_prover, &fs_pp, &index_pk, varuna_version, &assignment, rng).unwrap();
        println!("Called prover");

        let one = <Circuit as Environment>::BaseField::one();
        assert!(VarunaInst::verify(universal_verifier, &fs_pp, &index_vk, varuna_version, [one, one], &proof).unwrap());
        println!("Called verifier");
        println!("\nShould not verify (i.e. verifier messages should print below):");
        assert!(
            !VarunaInst::verify(universal_verifier, &fs_pp, &index_vk, varuna_version, [one, one + one], &proof)
                .unwrap()
        );
    }
}
