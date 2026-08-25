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

use crate::Assignment;
#[cfg(feature = "save_r1cs")]
use crate::{LinearCombination, Variable};
use snarkvm_fields::PrimeField;

#[cfg(feature = "save_r1cs")]
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

/// A canonical prime-field element represented as little-endian `u64` limbs.
#[cfg(feature = "save_r1cs")]
pub type CapturedField = Vec<u64>;

/// A variable reference in a captured R1CS linear combination.
#[cfg(feature = "save_r1cs")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapturedVariable {
    Public(u64),
    Private(u64),
}

/// A captured linear combination, split into its constant and variable terms.
#[cfg(feature = "save_r1cs")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedLinearCombination {
    pub constant: CapturedField,
    pub terms: Vec<(CapturedVariable, CapturedField)>,
    pub value: CapturedField,
}

/// One captured R1CS constraint `(A * B) = C`.
#[cfg(feature = "save_r1cs")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedConstraint {
    pub a: CapturedLinearCombination,
    pub b: CapturedLinearCombination,
    pub c: CapturedLinearCombination,
}

/// A field-agnostic snapshot of an assignment selected for certificate verification.
#[cfg(feature = "save_r1cs")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedR1CS {
    pub label: String,
    pub public_inputs: Vec<CapturedField>,
    pub private_inputs: Vec<CapturedField>,
    pub constraints: Vec<CapturedConstraint>,
    /// Constants, public variables, and private variables allocated during synthesis.
    pub num_variables: u64,
}

#[cfg(feature = "save_r1cs")]
static R1CS_CAPTURE_ENABLED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "save_r1cs")]
static CAPTURED_R1CS: Mutex<Vec<CapturedR1CS>> = Mutex::new(Vec::new());

/// Enables or disables assignment capture. Capture is disabled by default.
#[cfg(feature = "save_r1cs")]
pub fn set_r1cs_capture_enabled(enabled: bool) {
    R1CS_CAPTURE_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Takes all captured assignments, leaving the global collection empty.
#[cfg(feature = "save_r1cs")]
pub fn take_captured_r1cs() -> Vec<CapturedR1CS> {
    std::mem::take(&mut *CAPTURED_R1CS.lock().expect("R1CS capture lock poisoned"))
}

/// Captures an assignment when the `save_r1cs` feature and runtime switch are enabled.
///
/// This function intentionally exists as a no-op without the feature so callers in
/// higher-level crates do not need to mirror the dependency's feature configuration.
pub fn capture_r1cs_assignment<F: PrimeField>(label: String, assignment: &Assignment<F>) {
    #[cfg(feature = "save_r1cs")]
    {
        if !R1CS_CAPTURE_ENABLED.load(Ordering::Relaxed) {
            return;
        }

        let capture = CapturedR1CS {
            label,
            public_inputs: assignment.public_inputs().iter().map(|variable| capture_field(variable.value())).collect(),
            private_inputs: assignment
                .private_inputs()
                .iter()
                .map(|variable| capture_field(variable.value()))
                .collect(),
            constraints: assignment
                .constraints()
                .iter()
                .map(|constraint| {
                    let (a, b, c) = constraint.to_terms();
                    CapturedConstraint {
                        a: capture_linear_combination(a),
                        b: capture_linear_combination(b),
                        c: capture_linear_combination(c),
                    }
                })
                .collect(),
            num_variables: assignment.num_variables(),
        };
        CAPTURED_R1CS.lock().expect("R1CS capture lock poisoned").push(capture);
    }

    #[cfg(not(feature = "save_r1cs"))]
    let _ = (label, assignment);
}

#[cfg(feature = "save_r1cs")]
fn capture_field<F: PrimeField>(field: F) -> CapturedField {
    field.to_bigint().as_ref().to_vec()
}

#[cfg(feature = "save_r1cs")]
fn capture_linear_combination<F: PrimeField>(linear_combination: &LinearCombination<F>) -> CapturedLinearCombination {
    let terms = linear_combination
        .to_terms()
        .iter()
        .map(|(variable, coefficient)| {
            let variable = match variable {
                Variable::Public(index_value) => CapturedVariable::Public(index_value.0),
                Variable::Private(index_value) => CapturedVariable::Private(index_value.0),
                Variable::Constant(_) => unreachable!("R1CS terms cannot contain constant variables"),
            };
            (variable, capture_field(*coefficient))
        })
        .collect();
    CapturedLinearCombination {
        constant: capture_field(linear_combination.to_constant()),
        terms,
        value: capture_field(linear_combination.value()),
    }
}

#[cfg(all(test, feature = "save_r1cs"))]
mod tests {
    use super::*;
    use crate::{Circuit, Environment, Mode};
    use serial_test::serial;
    use snarkvm_fields::One;

    #[test]
    #[serial]
    fn test_capture_switch_and_labels() {
        type BaseField = <Circuit as Environment>::BaseField;

        let public = Circuit::new_variable(Mode::Public, BaseField::one());
        let private = Circuit::new_variable(Mode::Private, BaseField::one());
        Circuit::enforce(|| (public, private.clone(), private)).unwrap();
        let assignment = Circuit::eject_assignment_and_reset();

        set_r1cs_capture_enabled(false);
        let _ = take_captured_r1cs();
        capture_r1cs_assignment("disabled".to_string(), &assignment);
        assert!(take_captured_r1cs().is_empty());

        set_r1cs_capture_enabled(true);
        capture_r1cs_assignment("program.aleo/function:main".to_string(), &assignment);
        set_r1cs_capture_enabled(false);

        let captures = take_captured_r1cs();
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].label, "program.aleo/function:main");
        assert_eq!(captures[0].public_inputs.len(), 2);
        assert_eq!(captures[0].private_inputs.len(), 1);
        assert_eq!(captures[0].constraints.len(), 1);
    }
}
