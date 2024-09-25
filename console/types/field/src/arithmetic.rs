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

impl<E: Environment> Neg for Field<E> {
    type Output = Field<E>;

    /// Returns the `negation` of `self`.
    #[inline]
    fn neg(self) -> Self::Output {
        Field::new(-self.field)
    }
}

impl<E: Environment> Add<Field<E>> for Field<E> {
    type Output = Field<E>;

    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn add(self, other: Field<E>) -> Self::Output {
        Field::new(self.field + other.field)
    }
}

impl<E: Environment> Add<&Field<E>> for Field<E> {
    type Output = Field<E>;

    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn add(self, other: &Field<E>) -> Self::Output {
        Field::new(self.field + other.field)
    }
}

impl<E: Environment> AddAssign<Field<E>> for Field<E> {
    /// Adds `other` to `self`.
    #[inline]
    fn add_assign(&mut self, other: Field<E>) {
        self.field += other.field;
    }
}

impl<E: Environment> AddAssign<&Field<E>> for Field<E> {
    /// Adds `other` to `self`.
    #[inline]
    fn add_assign(&mut self, other: &Field<E>) {
        self.field += other.field;
    }
}

impl<E: Environment> Sub<Field<E>> for Field<E> {
    type Output = Field<E>;

    /// Returns the `difference` of `self` and `other`.
    #[inline]
    fn sub(self, other: Field<E>) -> Self::Output {
        Field::new(self.field - other.field)
    }
}

impl<E: Environment> Sub<&Field<E>> for Field<E> {
    type Output = Field<E>;

    /// Returns the `difference` of `self` and `other`.
    #[inline]
    fn sub(self, other: &Field<E>) -> Self::Output {
        Field::new(self.field - other.field)
    }
}

impl<E: Environment> SubAssign<Field<E>> for Field<E> {
    /// Subtracts `other` from `self`.
    #[inline]
    fn sub_assign(&mut self, other: Field<E>) {
        self.field -= other.field;
    }
}

impl<E: Environment> SubAssign<&Field<E>> for Field<E> {
    /// Subtracts `other` from `self`.
    #[inline]
    fn sub_assign(&mut self, other: &Field<E>) {
        self.field -= other.field;
    }
}

impl<E: Environment> Mul<Field<E>> for Field<E> {
    type Output = Field<E>;

    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn mul(self, other: Field<E>) -> Self::Output {
        Field::new(self.field * other.field)
    }
}

impl<E: Environment> Mul<&Field<E>> for Field<E> {
    type Output = Field<E>;

    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn mul(self, other: &Field<E>) -> Self::Output {
        Field::new(self.field * other.field)
    }
}

impl<E: Environment> MulAssign<Field<E>> for Field<E> {
    /// Multiplies `self` by `other`.
    #[inline]
    fn mul_assign(&mut self, other: Field<E>) {
        self.field *= other.field;
    }
}

impl<E: Environment> MulAssign<&Field<E>> for Field<E> {
    /// Multiplies `self` by `other`.
    #[inline]
    fn mul_assign(&mut self, other: &Field<E>) {
        self.field *= other.field;
    }
}

impl<E: Environment> Div<Field<E>> for Field<E> {
    type Output = Field<E>;

    /// Returns the `quotient` of `self` and `other`.
    #[inline]
    fn div(self, other: Field<E>) -> Self::Output {
        match other.is_zero() {
            true => E::halt(format!("Field division by zero: {self} / {other}")),
            false => Field::new(self.field / other.field),
        }
    }
}

impl<E: Environment> Div<&Field<E>> for Field<E> {
    type Output = Field<E>;

    /// Returns the `quotient` of `self` and `other`.
    #[inline]
    fn div(self, other: &Field<E>) -> Self::Output {
        match other.is_zero() {
            true => E::halt(format!("Field division by zero: {self} / {other}")),
            false => Field::new(self.field / other.field),
        }
    }
}

impl<E: Environment> DivAssign<Field<E>> for Field<E> {
    /// Divides `self` by `other`.
    #[inline]
    fn div_assign(&mut self, other: Field<E>) {
        match other.is_zero() {
            true => E::halt(format!("Field division by zero: {self} / {other}")),
            false => self.field /= other.field,
        }
    }
}

impl<E: Environment> DivAssign<&Field<E>> for Field<E> {
    /// Divides `self` by `other`.
    #[inline]
    fn div_assign(&mut self, other: &Field<E>) {
        match other.is_zero() {
            true => E::halt(format!("Field division by zero: {self} / {other}")),
            false => self.field /= other.field,
        }
    }
}

impl<E: Environment> Pow<Field<E>> for Field<E> {
    type Output = Field<E>;

    /// Returns the `power` of `self` to the power of `other`.
    #[inline]
    fn pow(self, other: Field<E>) -> Self::Output {
        Field::new(self.field.pow(other.field.to_bigint()))
    }
}

impl<E: Environment> Pow<&Field<E>> for Field<E> {
    type Output = Field<E>;

    /// Returns the `power` of `self` to the power of `other`.
    #[inline]
    fn pow(self, other: &Field<E>) -> Self::Output {
        Field::new(self.field.pow(other.field.to_bigint()))
    }
}

impl<E: Environment> Double for Field<E> {
    type Output = Field<E>;

    /// Returns the `double` of `self`.
    #[inline]
    fn double(&self) -> Self::Output {
        Field::new(self.field.double())
    }
}

impl<E: Environment> Inverse for Field<E> {
    type Output = Field<E>;

    /// Returns the `inverse` of `self`.
    #[inline]
    fn inverse(&self) -> Result<Self::Output> {
        match self.field.inverse() {
            Some(inverse) => Ok(Field::new(inverse)),
            None => bail!("Failed to invert a field element: {self}"),
        }
    }
}

impl<E: Environment> Square for Field<E> {
    type Output = Field<E>;

    /// Returns the `square` of `self`.
    #[inline]
    fn square(&self) -> Self::Output {
        Field::new(self.field.square())
    }
}

impl<E: Environment> SquareRoot for Field<E> {
    type Output = Field<E>;

    /// Returns the `square_root` of `self`.
    /// If there are two square roots, the bitwise lesser one is returned.
    #[inline]
    fn square_root(&self) -> Result<Self::Output> {
        match self.field.sqrt() {
            Some(sqrt) => {
                // Return the smaller square root.
                let sqrt = Field::new(sqrt);
                let negative_sqrt: Field<E> = -sqrt;
                match *(sqrt.is_less_than_or_equal(&negative_sqrt)) {
                    true => Ok(sqrt),
                    false => Ok(negative_sqrt),
                }
            }
            None => bail!("Failed to square root a field element: {self}"),
        }
    }
}

impl<E: Environment> Field<E> {
    /// Returns the `square_root` of `self`, where the least significant bit of the square root is zero.
    #[inline]
    pub fn even_square_root(&self) -> Result<Self> {
        match self.field.sqrt() {
            Some(sqrt) => {
                let sqrt: Field<E> = Field::new(sqrt);
                // Check the least significant bit of the square root.
                // Note that the unwrap is safe since the number of bits is always greater than zero.
                match *sqrt.to_bits_be().last().unwrap() {
                    // If the lsb is set, return the negated square root.
                    true => Ok(-sqrt),
                    // Otherwise, return the square root.
                    false => Ok(sqrt),
                }
            }
            None => bail!("Failed to square root a field element: {self}"),
        }
    }
}

impl<E: Environment> Sum<Field<E>> for Field<E> {
    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn sum<I: Iterator<Item = Field<E>>>(iter: I) -> Self {
        iter.fold(Field::zero(), |a, b| a + b)
    }
}

impl<'a, E: Environment> Sum<&'a Field<E>> for Field<E> {
    /// Returns the `sum` of `self` and `other`.
    #[inline]
    fn sum<I: Iterator<Item = &'a Field<E>>>(iter: I) -> Self {
        iter.fold(Field::zero(), |a, b| a + b)
    }
}

impl<E: Environment> Product<Field<E>> for Field<E> {
    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn product<I: Iterator<Item = Field<E>>>(iter: I) -> Self {
        iter.fold(Field::one(), |a, b| a * b)
    }
}

impl<'a, E: Environment> Product<&'a Field<E>> for Field<E> {
    /// Returns the `product` of `self` and `other`.
    #[inline]
    fn product<I: Iterator<Item = &'a Field<E>>>(iter: I) -> Self {
        iter.fold(Field::one(), |a, b| a * b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    #[test]
    fn test_div_by_zero_fails() {
        let one = Field::<CurrentEnvironment>::one();
        let zero = Field::<CurrentEnvironment>::zero();

        let result = std::panic::catch_unwind(|| one / zero);
        assert!(result.is_err()); // Probe further for specific error type here, if desired
    }
}
