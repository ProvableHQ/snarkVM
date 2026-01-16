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

use super::{Affine, Projective};
use crate::traits::{ShortWeierstrassParameters as Parameters, group::AffineCurve};
// use crate::AffineCurve as AffineRepr;
use snarkvm_fields::{Field, One, Zero};
use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    hash::{Hash, Hasher},
    ops::{Add, AddAssign, Neg, SubAssign},
    // One, Zero,
};

/// Extended Jacobian coordinates for a point on an elliptic curve in short Weierstrass
/// form, over the base field `P::BaseField`.
/// This struct implements arithmetic via the extended Jacobian arithmetic outlined here:
/// <https://www.hyperelliptic.org/EFD/g1p/auto-shortw-xyzz.html>
#[derive(Copy, Clone)]
#[must_use]
pub struct Bucket<P: Parameters> {
    /// `X / ZZ` projection of the affine `X`
    pub x: P::BaseField,
    /// `Y / ZZZ` projection of the affine `Y`
    pub y: P::BaseField,
    /// Bucket multiplicative inverse. Will be `0` only at infinity.
    pub zz: P::BaseField,
    /// Bucket multiplicative inverse. Will be `0` only at infinity.
    pub zzz: P::BaseField,
}

impl<P: Parameters> Debug for Bucket<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.is_zero() {
            true => write!(f, "infinity"),
            false => write!(f, "({}, {}, {}, {})", self.x, self.y, self.zz, self.zzz),
        }
    }
}

impl<P: Parameters> Eq for Bucket<P> {}

impl<P: Parameters> PartialEq for Bucket<P> {
    fn eq(&self, other: &Self) -> bool {
        if self.is_zero() {
            return other.is_zero();
        }

        if other.is_zero() {
            return false;
        }

        // The points (X, Y, Z) and (X', Y', Z')
        // are equal when (X * Z^2) = (X' * Z'^2)
        // and (Y * Z^3) = (Y' * Z'^3).
        if self.x * other.zz == other.x * self.zz { self.y * other.zzz == other.y * self.zzz } else { false }
    }
}

impl<P: Parameters> Hash for Bucket<P> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Affine::from(*self).hash(state)
    }
}

impl<P: Parameters> Default for Bucket<P> {
    #[inline]
    fn default() -> Self {
        Self::zero()
    }
}

impl<P: Parameters> Bucket<P> {
    /// Constructs a new group element without checking whether the coordinates
    /// specify a point in the subgroup.
    pub const fn new_unchecked(x: P::BaseField, y: P::BaseField, zz: P::BaseField, zzz: P::BaseField) -> Self {
        Self { x, y, zz, zzz }
    }

    /// Returns the point at infinity, which always has Z = 0.
    #[inline]
    pub fn zero() -> Self {
        Self::new_unchecked(P::BaseField::one(), P::BaseField::one(), P::BaseField::zero(), P::BaseField::zero())
    }

    /// Checks whether `self.z.is_zero()`.
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.zz == P::BaseField::zero() && self.zzz == P::BaseField::zero()
    }

    pub fn double_in_place(&mut self) {
        // From https://www.hyperelliptic.org/EFD/g1p/auto-shortw-xyzz.html#doubling-dbl-2008-s-1
        // U = 2*Y1
        let mut u = self.y;
        u.double_in_place();

        // V = U^2
        let mut v = u;
        v.square_in_place();

        // W = U*V
        let mut w = u;
        w *= &v;

        // S = X1*V
        let mut s = self.x;
        s *= &v;

        // M = 3*X1^2+a*ZZ1^2
        let mut m = self.x.square();
        m += m.double();
        if P::WEIERSTRASS_A != P::BaseField::zero() {
            m += P::mul_by_a(&self.zz.square());
        }
        // X3 = M^2-2*S
        self.x = m;
        self.x.square_in_place();
        self.x -= &s.double();
        // Y3 = M*(S-X3)-W*Y1
        self.y = P::BaseField::sum_of_products(&[m, -w], &[(s - self.x), self.y]);
        // ZZ3 = V*ZZ1
        self.zz *= v;
        // ZZZ3 = W*ZZZ1
        self.zzz *= &w;
    }
}

impl<P: Parameters> Neg for Bucket<P> {
    type Output = Self;

    #[inline]
    fn neg(mut self) -> Self {
        self.y = -self.y;
        self
    }
}

impl<P: Parameters> AddAssign<&Projective<P>> for Bucket<P> {
    /// Using <https://www.hyperelliptic.org/EFD/g1p/auto-shortw-xyzz.html#addition-madd-2008-s>
    fn add_assign(&mut self, other: &Projective<P>) {
        // If the other point is not at infinity, set `self` to the other point.
        // If the other point *is* at infinity, `other.xy()` will be `None`, and we
        // will do nothing
        if !other.is_zero() {
            if self.is_zero() {
                self.x = other.x;
                self.y = other.y;
                self.zz = P::BaseField::one();
                self.zzz = P::BaseField::one();
                return;
            }

            let z1z1 = self.zz;

            // U2 = X2*Z1Z1
            let mut u2 = other.x;
            u2 *= &z1z1;

            // S2 = Y2*Z1*Z1Z1
            let s2 = other.y * self.zzz;

            if self.x == u2 {
                if self.y == s2 {
                    // The two points are equal, so we double.
                    *self = other.double_to_bucket();
                } else {
                    // a + (-a) = 0
                    *self = Self::zero()
                }
            } else {
                // P = (U2 - X1);
                let mut p = u2;
                p -= &self.x;

                // R = S2-Y1
                let mut r = s2;
                r -= &self.y;

                // PP = P^2
                let mut pp = p;
                pp.square_in_place();

                // PPP = P*PP
                let mut ppp = pp;
                ppp *= &p;

                // Q = X1 * PP;
                let mut q = self.x;
                q *= &pp;

                // X3 = r^2 -PPP - 2*Q
                self.x = r.square();
                self.x -= &ppp;
                self.x -= &q.double();

                // Y3 = R*(Q-X3)-Y1*PPP
                q -= &self.x;
                self.y = P::BaseField::sum_of_products(&[r, -self.y], &[q, ppp]);

                // ZZ3 = ZZ1*PP
                // ZZZ3 = ZZZ1*PPP
                self.zz *= &pp;
                self.zzz *= &ppp;
            }
        }
    }
}

impl<'a, P: Parameters> Add<&'a Self> for Bucket<P> {
    type Output = Self;

    #[inline]
    fn add(mut self, other: &'a Self) -> Self {
        self += other;
        self
    }
}

impl<'a, P: Parameters> AddAssign<&'a Self> for Bucket<P> {
    fn add_assign(&mut self, other: &'a Self) {
        if self.is_zero() {
            *self = *other;
            return;
        }

        if other.is_zero() {
            return;
        }

        // https://www.hyperelliptic.org/EFD/g1p/auto-shortw-xyzz.html#addition-add-2008-s
        // Works for all curves.

        // Z1Z1 = Z1^2
        let z1z1 = self.zz;

        // Z2Z2 = Z2^2
        let z2z2 = other.zz;

        // U1 = X1*Z2Z2
        let mut u1 = self.x;
        u1 *= &z2z2;

        // U2 = X2*Z1Z1
        let mut u2 = other.x;
        u2 *= &z1z1;

        // S1 = Y1*Z2*Z2Z2
        let s1 = self.y * other.zzz;

        // S2 = Y2*Z1*Z1Z1
        let s2 = other.y * self.zzz;

        if u1 == u2 {
            if s1 == s2 {
                // The two points are equal, so we double.
                self.double_in_place();
            } else {
                // a + (-a) = 0
                *self = Self::zero();
            }
        } else {
            // P = U2-U1
            let mut p = u2;
            p -= &u1;

            // R = S2-S1
            let mut r = s2;
            r -= &s1;

            // PP = P^2
            let mut pp = p;
            pp.square_in_place();

            // PPP = P*PP
            let mut ppp = pp;
            ppp *= &p;

            // Q = U1*PP
            let mut q = u1;
            q *= &pp;

            // X3 = R^2 - PPP - 2*Q
            self.x = r.square();
            self.x -= &ppp;
            self.x -= &q.double();

            // Y3 = R*(Q-X3)-S1*PPP
            q -= &self.x;
            self.y = P::BaseField::sum_of_products(&[r, -s1], &[q, ppp]);

            // ZZ3 = ZZ1*ZZ2*PP
            self.zz *= &pp;
            self.zz *= &other.zz;

            // ZZZ3 = ZZZ1*ZZZ2*PPP
            self.zzz *= &ppp;
            self.zzz *= &other.zzz;
        }
    }
}

impl<'a, P: Parameters> SubAssign<&'a Self> for Bucket<P> {
    fn sub_assign(&mut self, other: &'a Self) {
        *self += &(-(*other));
    }
}

impl<'a, P: Parameters> AddAssign<&'a Bucket<P>> for Projective<P> {
    fn add_assign(&mut self, other: &'a Bucket<P>) {
        if self.is_zero() {
            *self = (*other).into();
            return;
        }

        if other.is_zero() {
            return;
        }

        // TODO: optimize using an explicit formula.
        *self += Self::from(*other);
    }
}

// The affine point X, Y is represented in the Jacobian
// coordinates with Z = 1.
impl<P: Parameters> From<Affine<P>> for Bucket<P> {
    #[inline]
    fn from(p: Affine<P>) -> Self {
        if p.infinity {
            Self::zero()
        } else {
            Self { x: p.to_x_coordinate(), y: p.to_y_coordinate(), zz: P::BaseField::one(), zzz: P::BaseField::one() }
        }
    }
}

// The affine point X, Y is represented in the Jacobian
// coordinates with Z = 1.
impl<P: Parameters> From<Bucket<P>> for Affine<P> {
    #[inline]
    fn from(p: Bucket<P>) -> Self {
        p.zzz.inverse().map_or(Affine::zero(), |zzz_inv| {
            let b = p.zz.square();
            let x = p.x * b;
            let y = p.y * zzz_inv;
            Affine::from_coordinates_unchecked((x, y, p == Bucket::<P>::zero()))
        })
    }
}

impl<P: Parameters> From<Bucket<P>> for Projective<P> {
    #[inline]
    fn from(p: Bucket<P>) -> Self {
        if p.is_zero() { Projective::zero() } else { Projective::new(p.x * p.zz, p.y * p.zzz, p.zz) }
    }
}
