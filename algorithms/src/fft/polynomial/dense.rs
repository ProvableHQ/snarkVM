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

//! A polynomial represented in coefficient form.

use super::PolyMultiplier;
use crate::fft::{EvaluationDomain, Evaluations, Polynomial};
use snarkvm_fields::{Field, PrimeField};
use snarkvm_utilities::{cfg_iter_mut, serialize::*};

use anyhow::Result;
use num_traits::CheckedDiv;
use rand::Rng;
use std::{
    fmt,
    ops::{Add, AddAssign, Deref, DerefMut, Div, Mul, MulAssign, Neg, Sub, SubAssign},
};

use itertools::Itertools;

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

/// Stores a polynomial in coefficient form.
#[derive(Clone, PartialEq, Eq, Hash, Default, CanonicalSerialize, CanonicalDeserialize)]
#[must_use]
pub struct DensePolynomial<F: Field> {
    /// The coefficient of `x^i` is stored at location `i` in `self.coeffs`.
    pub coeffs: Vec<F>,
}

impl<F: Field> fmt::Debug for DensePolynomial<F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        for (i, coeff) in self.coeffs.iter().enumerate().filter(|(_, c)| !c.is_zero()) {
            if i == 0 {
                write!(f, "\n{coeff:?}",)?;
            } else if i == 1 {
                write!(f, " + \n{coeff:?} * x")?;
            } else {
                write!(f, " + \n{coeff:?} * x^{i}")?;
            }
        }
        Ok(())
    }
}

impl<F: Field> DensePolynomial<F> {
    /// Returns the zero polynomial.
    pub fn zero() -> Self {
        Self { coeffs: Vec::new() }
    }

    /// Checks if the given polynomial is zero.
    pub fn is_zero(&self) -> bool {
        self.coeffs.is_empty() || self.coeffs.iter().all(|coeff| coeff.is_zero())
    }

    /// Constructs a new polynomial from a list of coefficients.
    pub fn from_coefficients_slice(coeffs: &[F]) -> Self {
        Self::from_coefficients_vec(coeffs.to_vec())
    }

    /// Constructs a new polynomial from a list of coefficients.
    pub fn from_coefficients_vec(mut coeffs: Vec<F>) -> Self {
        // While there are zeros at the end of the coefficient vector, pop them off.
        while let Some(true) = coeffs.last().map(|c| c.is_zero()) {
            coeffs.pop();
        }
        // Check that either the coefficients vec are empty or that the last coeff is non-zero.
        assert!(coeffs.last().map_or(true, |coeff| !coeff.is_zero()));

        Self { coeffs }
    }

    /// Returns the degree of the polynomial.
    pub fn degree(&self) -> usize {
        if self.is_zero() {
            0
        } else {
            assert!(self.coeffs.last().map_or(false, |coeff| !coeff.is_zero()));
            self.coeffs.len() - 1
        }
    }

    /// Evaluates `self` at the given `point` in the field.
    pub fn evaluate(&self, point: F) -> F {
        if self.is_zero() {
            return F::zero();
        } else if point.is_zero() {
            return self.coeffs[0];
        }
        let mut powers_of_point = Vec::with_capacity(1 + self.degree());
        powers_of_point.push(F::one());
        let mut cur = point;
        for _ in 0..self.degree() {
            powers_of_point.push(cur);
            cur *= point;
        }
        let zero = F::zero();
        let mapping = crate::cfg_into_iter!(powers_of_point).zip_eq(&self.coeffs).map(|(power, coeff)| power * coeff);
        crate::cfg_reduce!(mapping, || zero, |a, b| a + b)
    }

    /// Outputs a univariate polynomial of degree `d` where each non-leading
    /// coefficient is sampled uniformly at random from R and the leading
    /// coefficient is sampled uniformly at random from among the non-zero
    /// elements of R.
    pub fn rand<R: Rng>(d: usize, rng: &mut R) -> Self {
        let mut random_coeffs = (0..(d + 1)).map(|_| F::rand(rng)).collect_vec();
        while random_coeffs[d].is_zero() {
            // In the extremely unlikely event, sample again.
            random_coeffs[d] = F::rand(rng);
        }
        Self::from_coefficients_vec(random_coeffs)
    }

    /// Returns the coefficients of `self`.
    pub fn coeffs(&self) -> &[F] {
        &self.coeffs
    }

    /// Perform a naive n^2 multiplication of `self` by `other`.
    #[cfg(test)]
    fn naive_mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            DensePolynomial::zero()
        } else {
            let mut result = vec![F::zero(); self.degree() + other.degree() + 1];
            for (i, self_coeff) in self.coeffs.iter().enumerate() {
                for (j, other_coeff) in other.coeffs.iter().enumerate() {
                    result[i + j] += *self_coeff * other_coeff;
                }
            }
            DensePolynomial::from_coefficients_vec(result)
        }
    }
}

impl<F: PrimeField> DensePolynomial<F> {
    /// Multiply `self` by the vanishing polynomial for the domain `domain`.
    pub fn mul_by_vanishing_poly(&self, domain: EvaluationDomain<F>) -> DensePolynomial<F> {
        let mut shifted = vec![F::zero(); domain.size()];
        shifted.extend_from_slice(&self.coeffs);
        crate::cfg_iter_mut!(shifted[..self.coeffs.len()]).zip_eq(&self.coeffs).for_each(|(s, c)| *s -= c);
        DensePolynomial::from_coefficients_vec(shifted)
    }

    /// Divide `self` by the vanishing polynomial for the domain `domain`.
    /// Returns the quotient and remainder of the division.
    pub fn divide_by_vanishing_poly(
        &self,
        domain: EvaluationDomain<F>,
    ) -> Result<(DensePolynomial<F>, DensePolynomial<F>)> {
        let self_poly = Polynomial::from(self);
        let vanishing_poly = Polynomial::from(domain.vanishing_polynomial());
        self_poly.divide_with_q_and_r(&vanishing_poly)
    }

    /// Evaluate `self` over `domain`.
    pub fn evaluate_over_domain_by_ref(&self, domain: EvaluationDomain<F>) -> Evaluations<F> {
        let poly: Polynomial<'_, F> = self.into();
        Polynomial::<F>::evaluate_over_domain(poly, domain)
    }

    /// Evaluate `self` over `domain`.
    pub fn evaluate_over_domain(self, domain: EvaluationDomain<F>) -> Evaluations<F> {
        let poly: Polynomial<'_, F> = self.into();
        Polynomial::<F>::evaluate_over_domain(poly, domain)
    }
}

impl<F: Field> From<super::SparsePolynomial<F>> for DensePolynomial<F> {
    fn from(other: super::SparsePolynomial<F>) -> Self {
        let mut result = vec![F::zero(); other.degree() + 1];
        for (i, coeff) in other.coeffs() {
            result[*i] = *coeff;
        }
        DensePolynomial::from_coefficients_vec(result)
    }
}

impl<'a, 'b, F: Field> Add<&'a DensePolynomial<F>> for &'b DensePolynomial<F> {
    type Output = DensePolynomial<F>;

    fn add(self, other: &'a DensePolynomial<F>) -> DensePolynomial<F> {
        let mut result = if self.is_zero() {
            other.clone()
        } else if other.is_zero() {
            self.clone()
        } else if self.degree() >= other.degree() {
            let mut result = self.clone();
            // Zip safety: `result` and `other` could have different lengths.
            cfg_iter_mut!(result.coeffs).zip(&other.coeffs).for_each(|(a, b)| *a += b);
            result
        } else {
            let mut result = other.clone();
            // Zip safety: `result` and `other` could have different lengths.
            cfg_iter_mut!(result.coeffs).zip(&self.coeffs).for_each(|(a, b)| *a += b);
            result
        };
        // If the leading coefficient ends up being zero, pop it off.
        while let Some(true) = result.coeffs.last().map(|c| c.is_zero()) {
            result.coeffs.pop();
        }
        result
    }
}

impl<'a, F: Field> AddAssign<&'a DensePolynomial<F>> for DensePolynomial<F> {
    fn add_assign(&mut self, other: &'a DensePolynomial<F>) {
        if self.is_zero() {
            self.coeffs.clear();
            self.coeffs.extend_from_slice(&other.coeffs);
        } else if other.is_zero() {
            // return
        } else if self.degree() >= other.degree() {
            // Zip safety: `self` and `other` could have different lengths.
            cfg_iter_mut!(self.coeffs).zip(&other.coeffs).for_each(|(a, b)| *a += b);
        } else {
            // Add the necessary number of zero coefficients.
            self.coeffs.resize(other.coeffs.len(), F::zero());
            // Zip safety: `self` and `other` have the same length.
            cfg_iter_mut!(self.coeffs).zip(&other.coeffs).for_each(|(a, b)| *a += b);
        }
        // If the leading coefficient ends up being zero, pop it off.
        while let Some(true) = self.coeffs.last().map(|c| c.is_zero()) {
            self.coeffs.pop();
        }
    }
}

impl<'a, F: Field> AddAssign<&'a Polynomial<'a, F>> for DensePolynomial<F> {
    fn add_assign(&mut self, other: &'a Polynomial<F>) {
        match other {
            Polynomial::Sparse(p) => *self += &Self::from(p.clone().into_owned()),
            Polynomial::Dense(p) => *self += p.as_ref(),
        }
    }
}

impl<'a, F: Field> AddAssign<(F, &'a Polynomial<'a, F>)> for DensePolynomial<F> {
    fn add_assign(&mut self, (f, other): (F, &'a Polynomial<F>)) {
        match other {
            Polynomial::Sparse(p) => *self += (f, &Self::from(p.clone().into_owned())),
            Polynomial::Dense(p) => *self += (f, p.as_ref()),
        }
    }
}

impl<'a, F: Field> AddAssign<(F, &'a DensePolynomial<F>)> for DensePolynomial<F> {
    #[allow(clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, (f, other): (F, &'a DensePolynomial<F>)) {
        if self.is_zero() {
            self.coeffs.clear();
            self.coeffs.extend_from_slice(&other.coeffs);
            self.coeffs.iter_mut().for_each(|c| *c *= &f);
        } else if other.is_zero() {
            // return
        } else if self.degree() >= other.degree() {
            // Zip safety: `self` and `other` could have different lengths.
            cfg_iter_mut!(self.coeffs).zip(&other.coeffs).for_each(|(a, b)| {
                *a += f * b;
            });
        } else {
            // Add the necessary number of zero coefficients.
            self.coeffs.resize(other.coeffs.len(), F::zero());
            // Zip safety: `self` and `other` have the same length after the resize.
            cfg_iter_mut!(self.coeffs).zip(&other.coeffs).for_each(|(a, b)| {
                *a += f * b;
            });
        }
        // If the leading coefficient ends up being zero, pop it off.
        while let Some(true) = self.coeffs.last().map(|c| c.is_zero()) {
            self.coeffs.pop();
        }
    }
}

impl<F: Field> Neg for DensePolynomial<F> {
    type Output = DensePolynomial<F>;

    #[inline]
    fn neg(mut self) -> DensePolynomial<F> {
        for coeff in &mut self.coeffs {
            *coeff = -*coeff;
        }
        self
    }
}

impl<'a, 'b, F: Field> Sub<&'a DensePolynomial<F>> for &'b DensePolynomial<F> {
    type Output = DensePolynomial<F>;

    #[inline]
    fn sub(self, other: &'a DensePolynomial<F>) -> DensePolynomial<F> {
        let mut result = if self.is_zero() {
            let mut result = other.clone();
            for coeff in &mut result.coeffs {
                *coeff = -(*coeff);
            }
            result
        } else if other.is_zero() {
            self.clone()
        } else if self.degree() >= other.degree() {
            let mut result = self.clone();
            // Zip safety: `result` and `other` could have different degrees.
            cfg_iter_mut!(result.coeffs).zip(&other.coeffs).for_each(|(a, b)| *a -= b);
            result
        } else {
            let mut result = self.clone();
            result.coeffs.resize(other.coeffs.len(), F::zero());
            // Zip safety: `result` and `other` have the same length after the resize.
            cfg_iter_mut!(result.coeffs).zip(&other.coeffs).for_each(|(a, b)| {
                *a -= b;
            });
            result
        };
        // If the leading coefficient ends up being zero, pop it off.
        while let Some(true) = result.coeffs.last().map(|c| c.is_zero()) {
            result.coeffs.pop();
        }
        result
    }
}

impl<'a, F: Field> SubAssign<&'a DensePolynomial<F>> for DensePolynomial<F> {
    #[inline]
    fn sub_assign(&mut self, other: &'a DensePolynomial<F>) {
        if self.is_zero() {
            self.coeffs.resize(other.coeffs.len(), F::zero());
            for (i, coeff) in other.coeffs.iter().enumerate() {
                self.coeffs[i] -= coeff;
            }
        } else if other.is_zero() {
            // return
        } else if self.degree() >= other.degree() {
            // Zip safety: self and other could have different lengths.
            cfg_iter_mut!(self.coeffs).zip(&other.coeffs).for_each(|(a, b)| *a -= b);
        } else {
            // Add the necessary number of zero coefficients.
            self.coeffs.resize(other.coeffs.len(), F::zero());
            // Zip safety: self and other have the same length after the resize.
            cfg_iter_mut!(self.coeffs).zip(&other.coeffs).for_each(|(a, b)| *a -= b);
        }
        // If the leading coefficient ends up being zero, pop it off.
        while let Some(true) = self.coeffs.last().map(|c| c.is_zero()) {
            self.coeffs.pop();
        }
    }
}

impl<'a, F: Field> AddAssign<&'a super::SparsePolynomial<F>> for DensePolynomial<F> {
    #[inline]
    fn add_assign(&mut self, other: &'a super::SparsePolynomial<F>) {
        if self.degree() < other.degree() {
            self.coeffs.resize(other.degree() + 1, F::zero());
        }
        for (i, b) in other.coeffs() {
            self.coeffs[*i] += b;
        }
        // If the leading coefficient ends up being zero, pop it off.
        while let Some(true) = self.coeffs.last().map(|c| c.is_zero()) {
            self.coeffs.pop();
        }
    }
}

impl<'a, F: Field> Sub<&'a super::SparsePolynomial<F>> for DensePolynomial<F> {
    type Output = Self;

    #[inline]
    fn sub(mut self, other: &'a super::SparsePolynomial<F>) -> Self::Output {
        if self.degree() < other.degree() {
            self.coeffs.resize(other.degree() + 1, F::zero());
        }
        for (i, b) in other.coeffs() {
            self.coeffs[*i] -= b;
        }
        // If the leading coefficient ends up being zero, pop it off.
        while let Some(true) = self.coeffs.last().map(|c| c.is_zero()) {
            self.coeffs.pop();
        }
        self
    }
}

impl<'a, 'b, F: Field> Div<&'a DensePolynomial<F>> for &'b DensePolynomial<F> {
    type Output = DensePolynomial<F>;

    /// This division can panic and ignores remainders
    #[inline]
    fn div(self, divisor: &'a DensePolynomial<F>) -> DensePolynomial<F> {
        let a: Polynomial<_> = self.into();
        let b: Polynomial<_> = divisor.into();
        a.divide_with_q_and_r(&b).expect("division failed").0
    }
}

impl<F: Field> Div<DensePolynomial<F>> for DensePolynomial<F> {
    type Output = DensePolynomial<F>;

    /// This division can panic and ignores remainders
    #[inline]
    fn div(self, divisor: DensePolynomial<F>) -> DensePolynomial<F> {
        let a: Polynomial<_> = self.into();
        let b: Polynomial<_> = divisor.into();
        a.divide_with_q_and_r(&b).expect("division failed").0
    }
}

impl<F: Field> CheckedDiv for DensePolynomial<F> {
    #[inline]
    fn checked_div(&self, divisor: &DensePolynomial<F>) -> Option<DensePolynomial<F>> {
        let a: Polynomial<_> = self.into();
        let b: Polynomial<_> = divisor.into();
        match a.divide_with_q_and_r(&b) {
            Ok((divisor, remainder)) => {
                if remainder.is_zero() {
                    Some(divisor)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }
}

/// Performs O(nlogn) multiplication of polynomials if F is smooth.
impl<'a, 'b, F: PrimeField> Mul<&'a DensePolynomial<F>> for &'b DensePolynomial<F> {
    type Output = DensePolynomial<F>;

    #[inline]
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn mul(self, other: &'a DensePolynomial<F>) -> DensePolynomial<F> {
        if self.is_zero() || other.is_zero() {
            DensePolynomial::zero()
        } else {
            let mut m = PolyMultiplier::new();
            m.add_polynomial_ref(self, "");
            m.add_polynomial_ref(other, "");
            m.multiply().unwrap()
        }
    }
}

/// Multiplies `self` by `other: F`.
impl<F: Field> Mul<F> for DensePolynomial<F> {
    type Output = Self;

    #[inline]
    fn mul(mut self, other: F) -> Self {
        self.iter_mut().for_each(|c| *c *= other);
        self
    }
}

/// Multiplies `self` by `other: F`.
impl<'a, F: Field> Mul<F> for &'a DensePolynomial<F> {
    type Output = DensePolynomial<F>;

    #[inline]
    fn mul(self, other: F) -> Self::Output {
        let result = self.clone();
        result * other
    }
}

/// Multiplies `self` by `other: F`.
impl<F: Field> MulAssign<F> for DensePolynomial<F> {
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn mul_assign(&mut self, other: F) {
        cfg_iter_mut!(self).for_each(|c| *c *= other);
    }
}

/// Multiplies `self` by `other: F`.
impl<F: Field> std::iter::Sum for DensePolynomial<F> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(DensePolynomial::zero(), |a, b| &a + &b)
    }
}

impl<F: Field> Deref for DensePolynomial<F> {
    type Target = [F];

    fn deref(&self) -> &[F] {
        &self.coeffs
    }
}

impl<F: Field> DerefMut for DensePolynomial<F> {
    fn deref_mut(&mut self) -> &mut [F] {
        &mut self.coeffs
    }
}

#[cfg(test)]
mod tests {
    use crate::fft::polynomial::*;
    use num_traits::CheckedDiv;
    use snarkvm_curves::bls12_377::Fr;
    use snarkvm_fields::{Field, One, Zero};
    use snarkvm_utilities::rand::{TestRng, Uniform};

    use rand::RngCore;

    #[test]
    fn double_polynomials_random() {
        let rng = &mut TestRng::default();
        for degree in 0..70 {
            let p = DensePolynomial::<Fr>::rand(degree, rng);
            let p_double = &p + &p;
            let p_quad = &p_double + &p_double;
            assert_eq!(&(&(&p + &p) + &p) + &p, p_quad);
        }
    }

    #[test]
    fn add_polynomials() {
        let rng = &mut TestRng::default();
        for a_degree in 0..70 {
            for b_degree in 0..70 {
                let p1 = DensePolynomial::<Fr>::rand(a_degree, rng);
                let p2 = DensePolynomial::<Fr>::rand(b_degree, rng);
                let res1 = &p1 + &p2;
                let res2 = &p2 + &p1;
                assert_eq!(res1, res2);
            }
        }
    }

    #[test]
    fn add_polynomials_with_mul() {
        let rng = &mut TestRng::default();
        for a_degree in 0..70 {
            for b_degree in 0..70 {
                let mut p1 = DensePolynomial::rand(a_degree, rng);
                let p2 = DensePolynomial::rand(b_degree, rng);
                let f = Fr::rand(rng);
                let f_p2 = DensePolynomial::from_coefficients_vec(p2.coeffs.iter().map(|c| f * c).collect());
                let res2 = &f_p2 + &p1;
                p1 += (f, &p2);
                let res1 = p1;
                assert_eq!(res1, res2);
            }
        }
    }

    #[test]
    fn sub_polynomials() {
        let rng = &mut TestRng::default();
        let p1 = DensePolynomial::<Fr>::rand(5, rng);
        let p2 = DensePolynomial::<Fr>::rand(3, rng);
        let res1 = &p1 - &p2;
        let res2 = &p2 - &p1;
        assert_eq!(&res1 + &p2, p1, "Subtraction should be inverse of addition!");
        assert_eq!(res1, -res2, "p2 - p1 = -(p1 - p2)");
    }

    #[test]
    fn divide_polynomials_fixed() {
        let dividend = DensePolynomial::from_coefficients_slice(&[
            "4".parse().unwrap(),
            "8".parse().unwrap(),
            "5".parse().unwrap(),
            "1".parse().unwrap(),
        ]);
        let divisor = DensePolynomial::from_coefficients_slice(&[Fr::one(), Fr::one()]); // Construct a monic linear polynomial.
        let result = dividend.checked_div(&divisor).unwrap();
        let expected_result = DensePolynomial::from_coefficients_slice(&[
            "4".parse().unwrap(),
            "4".parse().unwrap(),
            "1".parse().unwrap(),
        ]);
        assert_eq!(expected_result, result);
    }

    #[test]
    #[allow(clippy::needless_borrow)]
    fn divide_polynomials_random() {
        let rng = &mut TestRng::default();

        for a_degree in 0..70 {
            for b_degree in 0..70 {
                let dividend = DensePolynomial::<Fr>::rand(a_degree, rng);
                let divisor = DensePolynomial::<Fr>::rand(b_degree, rng);
                let (quotient, remainder) =
                    Polynomial::divide_with_q_and_r(&(&dividend).into(), &(&divisor).into()).unwrap();
                assert_eq!(dividend, &(&divisor * &quotient) + &remainder)
            }
        }
    }

    #[test]
    fn evaluate_polynomials() {
        let rng = &mut TestRng::default();
        for a_degree in 0..70 {
            let p = DensePolynomial::rand(a_degree, rng);
            let point: Fr = Fr::from(10u64);
            let mut total = Fr::zero();
            for (i, coeff) in p.coeffs.iter().enumerate() {
                total += point.pow([i as u64]) * coeff;
            }
            assert_eq!(p.evaluate(point), total);
        }
    }

    #[test]
    fn divide_poly_by_zero() {
        let a = Polynomial::<Fr>::zero();
        let b = Polynomial::<Fr>::zero();
        assert!(a.divide_with_q_and_r(&b).is_err());
    }

    #[test]
    fn mul_polynomials_random() {
        let rng = &mut TestRng::default();
        for a_degree in 0..70 {
            for b_degree in 0..70 {
                dbg!(a_degree);
                dbg!(b_degree);
                let a = DensePolynomial::<Fr>::rand(a_degree, rng);
                let b = DensePolynomial::<Fr>::rand(b_degree, rng);
                assert_eq!(&a * &b, a.naive_mul(&b))
            }
        }
    }

    #[test]
    fn mul_polynomials_n_random() {
        let rng = &mut TestRng::default();

        let max_degree = 1 << 8;

        for _ in 0..10 {
            let mut multiplier = PolyMultiplier::new();
            let a = DensePolynomial::<Fr>::rand(max_degree / 2, rng);
            let mut mul_degree = a.degree() + 1;
            multiplier.add_polynomial(a.clone(), "a");
            let mut naive = a.clone();

            // Include polynomials and evaluations
            let num_polys = (rng.next_u32() as usize) % 8;
            let num_evals = (rng.next_u32() as usize) % 4;
            println!("\nnum_polys {num_polys}, num_evals {num_evals}");

            for _ in 1..num_polys {
                let degree = (rng.next_u32() as usize) % max_degree;
                mul_degree += degree + 1;
                println!("poly degree {degree}");
                let a = DensePolynomial::<Fr>::rand(degree, rng);
                naive = naive.naive_mul(&a);
                multiplier.add_polynomial(a.clone(), "a");
            }

            // Add evaluations but don't overflow the domain
            let mut eval_degree = mul_degree;
            let domain = EvaluationDomain::new(mul_degree).unwrap();
            println!("mul_degree {}, domain {}", mul_degree, domain.size());
            for _ in 0..num_evals {
                let a = DensePolynomial::<Fr>::rand(mul_degree / 8, rng);
                eval_degree += a.len() + 1;
                if eval_degree < domain.size() {
                    println!("eval degree {eval_degree}");
                    let mut a_evals = a.clone().coeffs;
                    domain.fft_in_place(&mut a_evals);
                    let a_evals = Evaluations::from_vec_and_domain(a_evals, domain);

                    naive = naive.naive_mul(&a);
                    multiplier.add_evaluation(a_evals, "a");
                }
            }

            assert_eq!(multiplier.multiply().unwrap(), naive);
        }
    }

    #[test]
    fn mul_polynomials_corner_cases() {
        let rng = &mut TestRng::default();

        let a_degree = 70;

        // Single polynomial
        println!("Test single polynomial");
        let a = DensePolynomial::<Fr>::rand(a_degree, rng);
        let mut multiplier = PolyMultiplier::new();
        multiplier.add_polynomial(a.clone(), "a");
        assert_eq!(multiplier.multiply().unwrap(), a);

        // Note PolyMultiplier doesn't support evaluations with no polynomials
    }

    #[test]
    fn mul_by_vanishing_poly() {
        let rng = &mut TestRng::default();
        for size in 1..10 {
            let domain = EvaluationDomain::new(1 << size).unwrap();
            for degree in 0..70 {
                let p = DensePolynomial::<Fr>::rand(degree, rng);
                let ans1 = p.mul_by_vanishing_poly(domain);
                let ans2 = &p * &domain.vanishing_polynomial().into();
                assert_eq!(ans1, ans2);
            }
        }
    }
}
