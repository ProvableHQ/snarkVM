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

use std::collections::BTreeMap;

use crate::snark::varuna::{verifier::BatchCombiners, CircuitId};
use snarkvm_fields::PrimeField;
use snarkvm_utilities::{error, serialize::*, ToBytes, Write};

#[derive(Clone, Debug, Default, PartialEq, Eq, CanonicalSerialize, CanonicalDeserialize)]
pub struct MatrixSums<F: PrimeField> {
    pub sum_a: F,
    pub sum_b: F,
    pub sum_c: F,
}

impl<F: PrimeField> MatrixSums<F> {
    /// Iterate over the sums
    pub fn iter(&self) -> impl ExactSizeIterator<Item = F> {
        [self.sum_a, self.sum_b, self.sum_c].into_iter()
    }
}

/// The prover message in the third round.
#[derive(Clone, Debug, Default, PartialEq, Eq, CanonicalSerialize, CanonicalDeserialize)]
pub struct ThirdMessage<F: PrimeField> {
    pub sums: Vec<Vec<MatrixSums<F>>>,
}

impl<F: PrimeField> ThirdMessage<F> {
    pub(crate) fn sum(&self, batch_combiners: &BTreeMap<CircuitId, BatchCombiners<F>>, eta_b: F, eta_c: F) -> F {
        self.sums
            .iter()
            .zip(batch_combiners.values())
            .map(|(circuit_sums, combiners)| {
                combiners.circuit_combiner
                    * circuit_sums
                        .iter()
                        .zip(&combiners.instance_combiners)
                        .map(|(sums, combiner)| (sums.sum_a + eta_b * sums.sum_b + eta_c * sums.sum_c) * combiner)
                        .sum::<F>()
            })
            .sum()
    }
}

impl<F: PrimeField> ToBytes for ThirdMessage<F> {
    fn write_le<W: Write>(&self, mut w: W) -> io::Result<()> {
        CanonicalSerialize::serialize_compressed(self, &mut w).map_err(|_| error("Could not serialize ThirdMessage"))
    }
}

/// The prover message in the fourth round.
#[derive(Clone, Debug, Default, PartialEq, Eq, CanonicalSerialize, CanonicalDeserialize)]
pub struct FourthMessage<F: PrimeField> {
    pub sums: Vec<MatrixSums<F>>,
}

impl<F: PrimeField> ToBytes for FourthMessage<F> {
    fn write_le<W: Write>(&self, mut w: W) -> io::Result<()> {
        CanonicalSerialize::serialize_compressed(self, &mut w).map_err(|_| error("Could not serialize FourthMessage"))
    }
}
