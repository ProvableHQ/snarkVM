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

use std::{borrow::Borrow, collections::BTreeMap};

use crate::fft::domain::{FFTPrecomputation, IFFTPrecomputation};

/// A struct that helps multiply a batch of polynomials
use super::*;
use snarkvm_utilities::{cfg_into_iter, cfg_iter, cfg_iter_mut, cfg_reduce_with, ExecutionPool};

#[derive(Default)]
pub struct PolyMultiplier<'a, F: PrimeField> {
    polynomials: Vec<(String, Cow<'a, DensePolynomial<F>>)>,
    evaluations: Vec<(String, Cow<'a, crate::fft::Evaluations<F>>)>,
    fft_precomputation: Option<Cow<'a, FFTPrecomputation<F>>>,
    ifft_precomputation: Option<Cow<'a, IFFTPrecomputation<F>>>,
}

impl<'a, F: PrimeField> PolyMultiplier<'a, F> {
    #[inline]
    pub fn new() -> Self {
        Self { polynomials: Vec::new(), evaluations: Vec::new(), fft_precomputation: None, ifft_precomputation: None }
    }

    #[inline]
    pub fn add_precomputation(&mut self, fft_pc: &'a FFTPrecomputation<F>, ifft_pc: &'a IFFTPrecomputation<F>) {
        self.fft_precomputation = Some(Cow::Borrowed(fft_pc));
        self.ifft_precomputation = Some(Cow::Borrowed(ifft_pc));
    }

    #[inline]
    pub fn add_polynomial(&mut self, poly: DensePolynomial<F>, label: impl ToString) {
        self.polynomials.push((label.to_string(), Cow::Owned(poly)))
    }

    #[inline]
    pub fn add_evaluation(&mut self, evals: Evaluations<F>, label: impl ToString) {
        self.evaluations.push((label.to_string(), Cow::Owned(evals)))
    }

    #[inline]
    pub fn add_polynomial_ref(&mut self, poly: &'a DensePolynomial<F>, label: impl ToString) {
        self.polynomials.push((label.to_string(), Cow::Borrowed(poly)))
    }

    #[inline]
    pub fn add_evaluation_ref(&mut self, evals: &'a Evaluations<F>, label: impl ToString) {
        self.evaluations.push((label.to_string(), Cow::Borrowed(evals)))
    }

    /// Multiplies all polynomials stored in `self`.
    ///
    /// Returns `None` if any of the stored evaluations are over a domain that's
    /// insufficiently large to interpolate the product, or if `F` does not contain
    /// a sufficiently large subgroup for interpolation.
    #[allow(unused_mut)]
    pub fn multiply(mut self) -> Option<DensePolynomial<F>> {
        if self.polynomials.is_empty() && self.evaluations.is_empty() {
            Some(DensePolynomial::zero())
        } else {
            let degree = self.polynomials.iter().map(|(_, p)| p.degree() + 1).sum::<usize>();
            let domain = EvaluationDomain::new(degree)?;
            if self.evaluations.iter().any(|(_, e)| e.domain() != domain) {
                None
            } else {
                #[cfg(all(feature = "cuda", target_arch = "x86_64"))]
                {
                    let mut poly_slices = Vec::new();
                    for (_, p) in &self.polynomials {
                        poly_slices.push(p.coeffs().to_vec());
                    }
                    let mut eval_slices = Vec::new();
                    for (_, e) in &self.evaluations {
                        eval_slices.push(e.evaluations().to_vec());
                    }

                    let gpu_result_vec =
                        snarkvm_algorithms_cuda::polymul(domain.size(), &poly_slices, &eval_slices, &F::zero());
                    if let Ok(result) = gpu_result_vec {
                        return Some(DensePolynomial::from_coefficients_vec(result));
                    }
                }

                if self.fft_precomputation.is_none() {
                    self.fft_precomputation = Some(Cow::Owned(domain.precompute_fft()));
                }
                if self.ifft_precomputation.is_none() {
                    self.ifft_precomputation =
                        Some(Cow::Owned(self.fft_precomputation.as_ref().unwrap().to_ifft_precomputation()));
                }
                let fft_pc = &self.fft_precomputation.unwrap();
                let ifft_pc = &self.ifft_precomputation.unwrap();
                let mut pool = ExecutionPool::with_capacity(self.polynomials.len() + self.evaluations.len());
                for (_, p) in self.polynomials {
                    pool.add_job(move || {
                        let mut p = p.into_owned().coeffs;
                        p.resize(domain.size(), F::zero());
                        domain.out_order_fft_in_place_with_pc(&mut p, fft_pc);
                        p
                    })
                }
                for (_, e) in self.evaluations {
                    pool.add_job(move || {
                        let mut e = e.into_owned().evaluations;
                        e.resize(domain.size(), F::zero());
                        crate::fft::domain::derange(&mut e);
                        e
                    })
                }
                let results = pool.execute_all();
                let iter = cfg_into_iter!(results);
                let mut result = cfg_reduce_with!(iter, |mut a, b| {
                    cfg_iter_mut!(a).zip(b).for_each(|(a, b)| *a *= b);
                    a
                })
                .unwrap();
                domain.out_order_ifft_in_place_with_pc(&mut result, ifft_pc);
                Some(DensePolynomial::from_coefficients_vec(result))
            }
        }
    }

    pub fn element_wise_arithmetic_4_over_domain<T: Borrow<str>>(
        mut self,
        domain: EvaluationDomain<F>,
        labels: [T; 4],
        f: impl Fn(F, F, F, F) -> F + Sync,
    ) -> Option<DensePolynomial<F>> {
        if self.fft_precomputation.is_none() {
            self.fft_precomputation = Some(Cow::Owned(domain.precompute_fft()));
        }
        if self.ifft_precomputation.is_none() {
            self.ifft_precomputation =
                Some(Cow::Owned(self.fft_precomputation.as_ref().unwrap().to_ifft_precomputation()));
        }
        let fft_pc = self.fft_precomputation.as_ref().unwrap();
        let mut pool = ExecutionPool::with_capacity(self.polynomials.len() + self.evaluations.len());
        for (l, p) in self.polynomials {
            pool.add_job(move || {
                let mut p = p.clone().into_owned().coeffs;
                p.resize(domain.size(), F::zero());
                domain.out_order_fft_in_place_with_pc(&mut p, fft_pc);
                (l, p)
            })
        }
        for (l, e) in self.evaluations {
            pool.add_job(move || {
                let mut e = e.clone().into_owned().evaluations;
                e.resize(domain.size(), F::zero());
                crate::fft::domain::derange(&mut e);
                (l, e)
            })
        }
        let p = pool.execute_all().into_iter().collect::<BTreeMap<_, _>>();
        assert_eq!(p.len(), 4);
        let mut result = cfg_iter!(p[labels[0].borrow()])
            .zip(&p[labels[1].borrow()])
            .zip(&p[labels[2].borrow()])
            .zip(&p[labels[3].borrow()])
            .map(|(((a, b), c), d)| f(*a, *b, *c, *d))
            .collect::<Vec<_>>();
        drop(p);
        domain.out_order_ifft_in_place_with_pc(&mut result, &self.ifft_precomputation.unwrap());
        Some(DensePolynomial::from_coefficients_vec(result))
    }
}
