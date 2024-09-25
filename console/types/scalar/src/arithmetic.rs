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

use super::*;

impl<E: Environment> Neg for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `negation` of `self`.
    #[inline]
    fn neg(self) -> Self::Output {
        Scalar::new(-self.scalar)
    }
}

impl<E: Environment> Add<Scalar<E>> for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn add(self, other: Scalar<E>) -> Self::Output {
        Scalar::new(self.scalar + other.scalar)
    }
}

impl<E: Environment> Add<&Scalar<E>> for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn add(self, other: &Scalar<E>) -> Self::Output {
        Scalar::new(self.scalar + other.scalar)
    }
}

impl<E: Environment> AddAssign<Scalar<E>> for Scalar<E> {
    /// Adds `other` to `self`.
    #[inline]
    fn add_assign(&mut self, other: Scalar<E>) {
        self.scalar += other.scalar;
    }
}

impl<E: Environment> AddAssign<&Scalar<E>> for Scalar<E> {
    /// Adds `other` to `self`.
    #[inline]
    fn add_assign(&mut self, other: &Scalar<E>) {
        self.scalar += other.scalar;
    }
}

impl<E: Environment> Sub<Scalar<E>> for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `difference` of `self` and `other`.
    #[inline]
    fn sub(self, other: Scalar<E>) -> Self::Output {
        Scalar::new(self.scalar - other.scalar)
    }
}

impl<E: Environment> Sub<&Scalar<E>> for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `difference` of `self` and `other`.
    #[inline]
    fn sub(self, other: &Scalar<E>) -> Self::Output {
        Scalar::new(self.scalar - other.scalar)
    }
}

impl<E: Environment> SubAssign<Scalar<E>> for Scalar<E> {
    /// Subtracts `other` from `self`.
    #[inline]
    fn sub_assign(&mut self, other: Scalar<E>) {
        self.scalar -= other.scalar;
    }
}

impl<E: Environment> SubAssign<&Scalar<E>> for Scalar<E> {
    /// Subtracts `other` from `self`.
    #[inline]
    fn sub_assign(&mut self, other: &Scalar<E>) {
        self.scalar -= other.scalar;
    }
}

impl<E: Environment> Mul<Scalar<E>> for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn mul(self, other: Scalar<E>) -> Self::Output {
        Scalar::new(self.scalar * other.scalar)
    }
}

impl<E: Environment> Mul<&Scalar<E>> for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn mul(self, other: &Scalar<E>) -> Self::Output {
        Scalar::new(self.scalar * other.scalar)
    }
}

impl<E: Environment> MulAssign<Scalar<E>> for Scalar<E> {
    /// Multiplies `self` by `other`.
    #[inline]
    fn mul_assign(&mut self, other: Scalar<E>) {
        self.scalar *= other.scalar;
    }
}

impl<E: Environment> MulAssign<&Scalar<E>> for Scalar<E> {
    /// Multiplies `self` by `other`.
    #[inline]
    fn mul_assign(&mut self, other: &Scalar<E>) {
        self.scalar *= other.scalar;
    }
}

impl<E: Environment> Div<Scalar<E>> for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `quotient` of `self` and `other`.
    #[inline]
    fn div(self, other: Scalar<E>) -> Self::Output {
        match other.is_zero() {
            true => E::halt(format!("Scalar division by zero: {self} / {other}")),
            false => Scalar::new(self.scalar / other.scalar),
        }
    }
}

impl<E: Environment> Div<&Scalar<E>> for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `quotient` of `self` and `other`.
    #[inline]
    fn div(self, other: &Scalar<E>) -> Self::Output {
        match other.is_zero() {
            true => E::halt(format!("Scalar division by zero: {self} / {other}")),
            false => Scalar::new(self.scalar / other.scalar),
        }
    }
}

impl<E: Environment> DivAssign<Scalar<E>> for Scalar<E> {
    /// Divides `self` by `other`.
    #[inline]
    fn div_assign(&mut self, other: Scalar<E>) {
        match other.is_zero() {
            true => E::halt(format!("Scalar division by zero: {self} / {other}")),
            false => self.scalar /= other.scalar,
        }
    }
}

impl<E: Environment> DivAssign<&Scalar<E>> for Scalar<E> {
    /// Divides `self` by `other`.
    #[inline]
    fn div_assign(&mut self, other: &Scalar<E>) {
        match other.is_zero() {
            true => E::halt(format!("Scalar division by zero: {self} / {other}")),
            false => self.scalar /= other.scalar,
        }
    }
}

impl<E: Environment> Pow<Scalar<E>> for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `power` of `self` to the power of `other`.
    #[inline]
    fn pow(self, other: Scalar<E>) -> Self::Output {
        Scalar::new(self.scalar.pow(other.scalar.to_bigint()))
    }
}

impl<E: Environment> Pow<&Scalar<E>> for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `power` of `self` to the power of `other`.
    #[inline]
    fn pow(self, other: &Scalar<E>) -> Self::Output {
        Scalar::new(self.scalar.pow(other.scalar.to_bigint()))
    }
}

impl<E: Environment> Double for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `double` of `self`.
    #[inline]
    fn double(&self) -> Self::Output {
        Scalar::new(self.scalar.double())
    }
}

impl<E: Environment> Inverse for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `inverse` of `self`.
    #[inline]
    fn inverse(&self) -> Result<Self::Output> {
        match self.scalar.inverse() {
            Some(inverse) => Ok(Scalar::new(inverse)),
            None => bail!("Failed to invert a scalar element: {self}"),
        }
    }
}

impl<E: Environment> Square for Scalar<E> {
    type Output = Scalar<E>;

    /// Returns the `square` of `self`.
    #[inline]
    fn square(&self) -> Self::Output {
        Scalar::new(self.scalar.square())
    }
}

impl<E: Environment> Sum<Scalar<E>> for Scalar<E> {
    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn sum<I: Iterator<Item = Scalar<E>>>(iter: I) -> Self {
        iter.fold(Scalar::zero(), |a, b| a + b)
    }
}

impl<'a, E: Environment> Sum<&'a Scalar<E>> for Scalar<E> {
    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn sum<I: Iterator<Item = &'a Scalar<E>>>(iter: I) -> Self {
        iter.fold(Scalar::zero(), |a, b| a + b)
    }
}

impl<E: Environment> Product<Scalar<E>> for Scalar<E> {
    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn product<I: Iterator<Item = Scalar<E>>>(iter: I) -> Self {
        iter.fold(Scalar::one(), |a, b| a * b)
    }
}

impl<'a, E: Environment> Product<&'a Scalar<E>> for Scalar<E> {
    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn product<I: Iterator<Item = &'a Scalar<E>>>(iter: I) -> Self {
        iter.fold(Scalar::one(), |a, b| a * b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    #[test]
    fn test_div_by_zero_fails() {
        let one = Scalar::<CurrentEnvironment>::one();
        let zero = Scalar::<CurrentEnvironment>::zero();

        let result = std::panic::catch_unwind(|| one / zero);
        assert!(result.is_err()); // Probe further for specific error type here, if desired
    }
}
