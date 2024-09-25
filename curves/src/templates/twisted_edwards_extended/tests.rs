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

use super::{Affine, Projective};

use snarkvm_utilities::{
    io::Cursor,
    rand::Uniform,
    serialize::{CanonicalDeserialize, CanonicalSerialize},
    to_bytes_le,
    Compress,
    TestRng,
    ToBytes,
    Validate,
};

use crate::traits::{AffineCurve, MontgomeryParameters, ProjectiveCurve, TwistedEdwardsParameters};
use snarkvm_fields::{Field, One, PrimeField, Zero};

pub const ITERATIONS: usize = 10;

pub fn montgomery_conversion_test<P>()
where
    P: TwistedEdwardsParameters,
{
    // A = 2 * (a + d) / (a - d)
    let a =
        P::BaseField::one().double() * (P::EDWARDS_A + P::EDWARDS_D) * (P::EDWARDS_A - P::EDWARDS_D).inverse().unwrap();
    // B = 4 / (a - d)
    let b = P::BaseField::one().double().double() * (P::EDWARDS_A - P::EDWARDS_D).inverse().unwrap();

    assert_eq!(a, P::MontgomeryParameters::MONTGOMERY_A);
    assert_eq!(b, P::MontgomeryParameters::MONTGOMERY_B);
}

pub fn edwards_test<P: TwistedEdwardsParameters>(rng: &mut TestRng)
where
    P::BaseField: PrimeField,
{
    edwards_curve_serialization_test::<P>(rng);
    edwards_from_random_bytes::<P>(rng);
    edwards_from_x_and_y_coordinates::<P>(rng);
}

pub fn edwards_curve_serialization_test<P: TwistedEdwardsParameters>(rng: &mut TestRng) {
    let modes = [
        (Compress::Yes, Validate::Yes),
        (Compress::No, Validate::No),
        (Compress::Yes, Validate::No),
        (Compress::No, Validate::Yes),
    ];
    for (compress, validate) in modes {
        let buf_size = Affine::<P>::zero().serialized_size(compress);

        for _ in 0..10 {
            let a = Projective::<P>::rand(rng);
            let a = a.to_affine();
            {
                let mut serialized = vec![0; buf_size];
                let mut cursor = Cursor::new(&mut serialized[..]);
                a.serialize_with_mode(&mut cursor, compress).unwrap();

                let mut cursor = Cursor::new(&serialized[..]);
                let b = Affine::<P>::deserialize_with_mode(&mut cursor, compress, validate).unwrap();
                assert_eq!(a, b);
            }

            {
                let mut a_copy = a;
                // If we negate the y-coordinate, the point is no longer in the prime-order subgroup,
                // and so this should error when `validate == Validate::Yes`.
                a_copy.y = -a.y;
                a_copy.t = a_copy.x * a_copy.y;
                let mut serialized = vec![0; buf_size];
                let mut cursor = Cursor::new(&mut serialized[..]);
                a_copy.serialize_with_mode(&mut cursor, compress).unwrap();
                let mut cursor = Cursor::new(&serialized[..]);

                let b = Affine::<P>::deserialize_with_mode(&mut cursor, compress, validate);
                if validate == Validate::Yes {
                    b.unwrap_err();
                } else {
                    assert_eq!(a_copy, b.unwrap());
                }
            }

            {
                let a = Affine::<P>::zero();
                let mut serialized = vec![0; buf_size];
                let mut cursor = Cursor::new(&mut serialized[..]);
                a.serialize_with_mode(&mut cursor, compress).unwrap();
                let mut cursor = Cursor::new(&serialized[..]);
                let b = Affine::<P>::deserialize_with_mode(&mut cursor, compress, validate).unwrap();
                assert_eq!(a, b);
            }

            {
                let a = Affine::<P>::zero();
                let mut serialized = vec![0; buf_size - 1];
                let mut cursor = Cursor::new(&mut serialized[..]);
                a.serialize_with_mode(&mut cursor, compress).unwrap_err();
            }

            {
                let serialized = vec![0; buf_size - 1];
                let mut cursor = Cursor::new(&serialized[..]);
                Affine::<P>::deserialize_with_mode(&mut cursor, compress, validate).unwrap_err();
            }

            {
                let mut serialized = vec![0; a.uncompressed_size()];
                let mut cursor = Cursor::new(&mut serialized[..]);
                a.serialize_uncompressed(&mut cursor).unwrap();

                let mut cursor = Cursor::new(&serialized[..]);
                let b = Affine::<P>::deserialize_uncompressed(&mut cursor).unwrap();
                assert_eq!(a, b);
            }

            {
                let mut a_copy = a;
                a_copy.y = -a.y;
                a_copy.t = a_copy.x * a_copy.y;
                let mut serialized = vec![0; a.uncompressed_size()];
                let mut cursor = Cursor::new(&mut serialized[..]);
                a_copy.serialize_uncompressed(&mut cursor).unwrap();
                let mut cursor = Cursor::new(&serialized[..]);
                let _ = Affine::<P>::deserialize_uncompressed(&mut cursor).unwrap_err();
                let b = Affine::<P>::deserialize_uncompressed_unchecked(&*serialized).unwrap();
                assert_eq!(a_copy, b);
            }

            {
                let a = Affine::<P>::zero();
                let mut serialized = vec![0; a.uncompressed_size()];
                let mut cursor = Cursor::new(&mut serialized[..]);
                a.serialize_uncompressed(&mut cursor).unwrap();
                let mut cursor = Cursor::new(&serialized[..]);
                let b = Affine::<P>::deserialize_uncompressed(&mut cursor).unwrap();
                assert_eq!(a, b);
            }
        }
    }
}

pub fn edwards_from_random_bytes<P: TwistedEdwardsParameters>(rng: &mut TestRng)
where
    P::BaseField: PrimeField,
{
    let buf_size = Affine::<P>::zero().compressed_size();

    for _ in 0..ITERATIONS {
        let a = Projective::<P>::rand(rng);
        let a = a.to_affine();
        {
            let mut serialized = vec![0; buf_size];
            let mut cursor = Cursor::new(&mut serialized[..]);
            a.serialize_compressed(&mut cursor).unwrap();

            let mut cursor = Cursor::new(&serialized[..]);
            let p1 = Affine::<P>::deserialize_compressed(&mut cursor).unwrap();
            let p2 = Affine::<P>::from_random_bytes(&serialized).unwrap();
            assert_eq!(p1, p2);
        }
    }

    for _ in 0..ITERATIONS {
        let biginteger = <<Affine<P> as AffineCurve>::BaseField as PrimeField>::BigInteger::rand(rng);
        let mut bytes = to_bytes_le![biginteger].unwrap();
        let mut g = Affine::<P>::from_random_bytes(&bytes);
        while g.is_none() {
            bytes.iter_mut().for_each(|i| *i = i.wrapping_sub(1));
            g = Affine::<P>::from_random_bytes(&bytes);
        }
        let _g = g.unwrap();
    }
}

pub fn edwards_from_x_and_y_coordinates<P: TwistedEdwardsParameters>(rng: &mut TestRng)
where
    P::BaseField: PrimeField,
{
    for _ in 0..ITERATIONS {
        let a = Projective::<P>::rand(rng);
        let a = a.to_affine();
        {
            let x = a.x;

            let a1 = Affine::<P>::from_x_coordinate(x, true).unwrap();
            let a2 = Affine::<P>::from_x_coordinate(x, false).unwrap();

            assert!(a == a1 || a == a2);

            let (b2, b1) = Affine::<P>::pair_from_x_coordinate(x).unwrap();

            assert_eq!(a1, b1);
            assert_eq!(a2, b2);
        }
        {
            let y = a.y;

            let a1 = Affine::<P>::from_y_coordinate(y, true).unwrap();
            let a2 = Affine::<P>::from_y_coordinate(y, false).unwrap();

            assert!(a == a1 || a == a2);
        }
    }
}
