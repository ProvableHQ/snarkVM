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

//! Produce challenge indices from a transcript.

use snarkvm_fields::Field;

use crate::snark::provekit::whir::{
    algebra::geometric_sequence,
    transcript::{Decoding, FieldElem, VerifierMessage},
};

pub fn geometric_challenge<T, F>(transcript: &mut T, count: usize) -> Vec<F>
where
    T: VerifierMessage,
    F: Field,
    FieldElem<F>: Decoding<[T::U]>,
{
    match count {
        0 => Vec::new(),
        1 => vec![F::one()],
        _ => {
            // Only source entropy when required
            let x = transcript.verifier_field();
            geometric_sequence(x, count)
        }
    }
}
