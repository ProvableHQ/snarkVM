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
use crate::{AffineCurve, ProjectiveCurve, ShortWeierstrassParameters};
use snarkvm_fields::Zero;
use snarkvm_utilities::{
    io::Cursor,
    rand::Uniform,
    serialize::{CanonicalDeserialize, CanonicalSerialize},
    Compress,
    TestRng,
    Validate,
};

pub const ITERATIONS: usize = 10;

pub fn sw_tests<P: ShortWeierstrassParameters>(rng: &mut TestRng) {
    sw_curve_serialization_test::<P>(rng);
    sw_from_random_bytes::<P>(rng);
    sw_from_x_coordinate::<P>(rng);
}

pub fn sw_curve_serialization_test<P: ShortWeierstrassParameters>(rng: &mut TestRng) {
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
            let mut a = a.to_affine();
            {
                let mut serialized = vec![0; buf_size];
                let mut cursor = Cursor::new(&mut serialized[..]);
                a.serialize_with_mode(&mut cursor, compress).unwrap();

                let mut cursor = Cursor::new(&serialized[..]);
                let b = Affine::<P>::deserialize_with_mode(&mut cursor, compress, validate).unwrap();
                assert_eq!(a, b);
            }

            {
                a.y = -a.y;
                let mut serialized = vec![0; buf_size];
                let mut cursor = Cursor::new(&mut serialized[..]);
                a.serialize_with_mode(&mut cursor, compress).unwrap();
                let mut cursor = Cursor::new(&serialized[..]);
                let b = Affine::<P>::deserialize_with_mode(&mut cursor, compress, validate).unwrap();
                assert_eq!(a, b);
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
                a.y = -a.y;
                let mut serialized = vec![0; a.uncompressed_size()];
                let mut cursor = Cursor::new(&mut serialized[..]);
                a.serialize_uncompressed(&mut cursor).unwrap();
                let mut cursor = Cursor::new(&serialized[..]);
                let b = Affine::<P>::deserialize_uncompressed(&mut cursor).unwrap();
                assert_eq!(a, b);
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

pub fn sw_from_random_bytes<P: ShortWeierstrassParameters>(rng: &mut TestRng) {
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
}

pub fn sw_from_x_coordinate<P: ShortWeierstrassParameters>(rng: &mut TestRng) {
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
    }
}
