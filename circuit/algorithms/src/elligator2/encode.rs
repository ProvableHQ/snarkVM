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

impl<E: Environment> Elligator2<E> {
    /// Returns the encoded affine group element, given a field element.
    /// Note: Unlike the console implementation, this function does not return the sign bit.
    pub fn encode(input: &Field<E>) -> Group<E> {
        // Ensure D on the twisted Edwards curve is a quadratic nonresidue.
        debug_assert!(console::Group::<E::Network>::EDWARDS_D.legendre().is_qnr());

        // Ensure the input is nonzero.
        E::assert_neq(input, Field::<E>::zero());

        // Define `1` as a constant.
        let one = Field::one();

        // Define the Montgomery curve coefficients A and B.
        let montgomery_a = Field::constant(console::Group::<E::Network>::MONTGOMERY_A);
        let montgomery_b = Field::constant(console::Group::<E::Network>::MONTGOMERY_B);
        let montgomery_b_inverse = montgomery_b.inverse();
        let montgomery_b2 = montgomery_b.square();
        let montgomery_b3 = &montgomery_b2 * &montgomery_b;

        // Define the twisted Edwards curve coefficient D.
        let edwards_d = Field::constant(console::Group::<E::Network>::EDWARDS_D);

        // Define the coefficients for the Weierstrass form: y^2 == x^3 + A * x^2 + B * x.
        let a = &montgomery_a * &montgomery_b_inverse;
        let a_half = &a * Field::constant(console::Field::half());
        let b = montgomery_b_inverse.square();

        // Define the MODULUS_MINUS_ONE_DIV_TWO as a constant.
        let modulus_minus_one_div_two = match E::BaseField::from_bigint(E::BaseField::modulus_minus_one_div_two()) {
            Some(modulus_minus_one_div_two) => Field::constant(console::Field::new(modulus_minus_one_div_two)),
            None => E::halt("Failed to initialize MODULUS_MINUS_ONE_DIV_TWO as a constant"),
        };

        // Compute the mapping from Fq to E(Fq) as a Montgomery element (u, v).
        let (u, v) = {
            // Let ur2 = D * input^2;
            let ur2 = edwards_d * input.square();
            let one_plus_ur2 = &one + &ur2;
            // Verify A^2 * ur^2 != B(1 + ur^2)^2.
            E::assert_neq(a.square() * &ur2, &b * one_plus_ur2.square());

            // Let v = -A / (1 + ur^2).
            let v = -&a / one_plus_ur2;

            // Let e = legendre(v^3 + Av^2 + Bv).
            let v2 = v.square();
            let e = ((&v2 * &v) + (&a * &v2) + (&b * &v)).pow(modulus_minus_one_div_two);

            // Let x = ev - ((1 - e) * A/2).
            let x = (&e * &v) - ((&one - &e) * a_half);

            // Let y = -e * sqrt(x^3 + Ax^2 + Bx).
            let x2 = x.square();
            let x3 = &x2 * &x;
            let rhs = &x3 + (&a * &x2) + (&b * &x);

            // Witness the even square root of `rhs`.
            let rhs_square_root: Field<E> = witness!(|rhs| {
                match rhs.square_root() {
                    Ok(sqrt) => {
                        // Get the least significant bit of the square root.
                        // Note that the unwrap is safe since the number of bits is always greater than zero.
                        match *sqrt.to_bits_be().last().unwrap() {
                            // If the lsb is set, return the negated square root.
                            true => -sqrt,
                            // Otherwise, return the square root.
                            false => sqrt,
                        }
                    }
                    Err(_) => console::Field::zero(),
                }
            });
            // Verify that the square root is even.
            // Note that the unwrap is safe since the number of bits is always greater than zero,
            E::assert(!rhs_square_root.to_bits_be().last().unwrap());

            let y = -&e * rhs_square_root;

            // Ensure v * e * x * y != 0.
            E::assert_neq(&v * &e * &x * &y, Field::<E>::zero());

            // Ensure (x, y) is a valid Weierstrass element on: y^2 == x^3 + A * x^2 + B * x.
            let y2 = y.square();
            E::assert_eq(&y2, rhs);

            // Convert the Weierstrass element (x, y) to Montgomery element (u, v).
            let u = x * &montgomery_b;
            let v = y * &montgomery_b;

            // Ensure (u, v) is a valid Montgomery element on: B * v^2 == u^3 + A * u^2 + u
            let u2 = &x2 * &montgomery_b2;
            let u3 = &x3 * &montgomery_b3;
            let v2 = &y2 * &montgomery_b3;
            E::assert_eq(v2, u3 + (montgomery_a * u2) + &u);

            (u, v)
        };

        // Convert the Montgomery element (u, v) to the twisted Edwards element (x, y).
        let x = &u / v;
        let y = (&u - &one) / (u + &one);

        // Recover the point and check that it is 1) on the curve, and 2) in the correct subgroup.
        let encoding = Group::from_xy_coordinates_unchecked(x, y);
        // Ensure the encoding is on the curve.
        encoding.enforce_on_curve();
        // Cofactor clear the twisted Edwards element (x, y).
        encoding.mul_by_cofactor()
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use snarkvm_circuit_types::environment::Circuit;
    use snarkvm_utilities::{TestRng, Uniform};

    const ITERATIONS: u64 = 1_000;

    fn check_encode(mode: Mode, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a random element.
            let given = Uniform::rand(&mut rng);

            // Compute the expected native result.
            let (expected, _sign) = console::Elligator2::<<Circuit as Environment>::Network>::encode(&given).unwrap();

            // Initialize the input field element.
            let input = Field::<Circuit>::new(mode, given);

            // Encode the input.
            Circuit::scope("Elligator2::encode", || {
                let candidate = Elligator2::encode(&input);
                assert_eq!(expected, candidate.eject_value());
                assert_scope!(num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
    }

    #[test]
    fn test_encode_constant() {
        check_encode(Mode::Constant, 527, 0, 0, 0);
    }

    #[test]
    fn test_encode_public() {
        check_encode(Mode::Public, 263, 0, 875, 880);
    }

    #[test]
    fn test_encode_private() {
        check_encode(Mode::Private, 263, 0, 875, 880);
    }
}
