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

use std::collections::HashSet;

#[derive(Clone)]
pub struct DegreeInfo {
    /// The maximum degree of the required SRS to commit to the polynomials.
    pub max_degree: usize,
    /// The maximum IOP poly degree used for (i)fft_precomputation.
    pub max_fft_size: usize,
    /// TODO (howardwu): This should never be happening... Use a BTreeSet.
    /// The degree bounds on IOP polynomials.
    pub degree_bounds: Option<HashSet<usize>>,
    /// The hiding bound for polynomial queries.
    pub hiding_bound: usize,
    /// The supported sizes for the lagrange-basis SRS.
    pub lagrange_sizes: Option<HashSet<usize>>,
}

impl DegreeInfo {
    /// Initializes a new degree info.
    pub const fn new(
        max_degree: usize,
        max_fft_size: usize,
        degree_bounds: Option<HashSet<usize>>,
        hiding_bound: usize,
        lagrange_sizes: Option<HashSet<usize>>,
    ) -> Self {
        Self { max_degree, max_fft_size, degree_bounds, hiding_bound, lagrange_sizes }
    }
}

impl DegreeInfo {
    pub fn union(self, other: &Self) -> Self {
        let max_degree = self.max_degree.max(other.max_degree);
        let max_fft_size = self.max_fft_size.max(other.max_fft_size);
        let degree_bounds = match (&self.degree_bounds, &other.degree_bounds) {
            (Some(a), Some(b)) => Some(a | b),
            (Some(a), None) | (None, Some(a)) => Some(a.clone()),
            (None, None) => None,
        };
        let hiding_bound = self.hiding_bound.max(other.hiding_bound);
        let lagrange_sizes = match (&self.lagrange_sizes, &other.lagrange_sizes) {
            (Some(a), Some(b)) => Some(a | b),
            (Some(a), None) | (None, Some(a)) => Some(a.clone()),
            (None, None) => None,
        };
        Self::new(max_degree, max_fft_size, degree_bounds, hiding_bound, lagrange_sizes)
    }
}
