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

use std::io::Cursor;

use super::{Affine, Projective};
use crate::{AffineCurve, ProjectiveCurve, ShortWeierstrassParameters};
use snarkvm_fields::{One, Zero};
use snarkvm_utilities::{
    Compress,
    TestRng,
    Validate,
    rand::Uniform,
    serialize::{CanonicalDeserialize, CanonicalSerialize},
};

pub const ITERATIONS: usize = 10;

pub fn sw_tests<P: ShortWeierstrassParameters>(rng: &mut TestRng) {
    sw_curve_serialization_test::<P>(rng);
    sw_from_random_bytes::<P>(rng);
    sw_from_x_coordinate::<P>(rng);
    sw_non_canonical_infinity_is_rejected::<P>();
}

/// The point at infinity must have exactly one compressed encoding.
///
/// When the infinity flag is set, the x-coordinate carries no information and
/// must be zero. A deserializer that ignores it instead of rejecting it admits
/// roughly 2^(8*(N-1)) encodings of the identity, all of which re-serialize to
/// the single canonical form -- so an object containing one has many byte
/// representations, and anything that hashes those bytes disagrees with anything
/// that hashes a re-serialization.
///
/// This is the same requirement the BLS12-381 serialization spec places on
/// decoders, and the same predicate `Affine::from_random_bytes` already applies
/// a few lines away: `if x.is_zero() && flags.is_infinity()`.
pub fn sw_non_canonical_infinity_is_rejected<P: ShortWeierstrassParameters>() {
    // First, the other direction: the serializer must never *emit* a
    // non-canonical infinity, or the rejection below would refuse our own
    // output. This is reachable rather than hypothetical -- the uncompressed
    // deserializer builds `Affine::new(x, y, flags.is_infinity())`, keeping
    // whatever coordinates were sent, so a point can carry the infinity flag
    // alongside a non-zero x.
    {
        let dirty = Affine::<P>::new(P::BaseField::one(), P::BaseField::one(), true);
        let mut written = Vec::new();
        dirty.serialize_with_mode(&mut written, Compress::Yes).unwrap();

        let mut canonical = Vec::new();
        Affine::<P>::zero().serialize_with_mode(&mut canonical, Compress::Yes).unwrap();

        assert_eq!(
            written, canonical,
            "compressing an infinity point with dirty coordinates must still emit the canonical form"
        );
    }

    for validate in [Validate::Yes, Validate::No] {
        // Validate does not implement Debug, and the mode matters to the report:
        // this must be rejected on the structure of the encoding, not by the
        // subgroup check, so it has to fail under Validate::No as well.
        let mode = match validate {
            Validate::Yes => "Validate::Yes",
            Validate::No => "Validate::No",
        };
        // The canonical encoding of infinity: zero x, with the flag set.
        let mut canonical = Vec::new();
        Affine::<P>::zero().serialize_with_mode(&mut canonical, Compress::Yes).unwrap();

        // It must keep working. A fix for the case below that also rejects this
        // one would break every proof in existence.
        let mut cursor = Cursor::new(&canonical[..]);
        let point = Affine::<P>::deserialize_with_mode(&mut cursor, Compress::Yes, validate).unwrap();
        assert!(point.is_zero(), "the canonical infinity encoding must still deserialize");

        // The same encoding with a non-zero x-coordinate. The flag lives in the
        // high bits of the last byte, so touching the first byte changes x and
        // leaves the flag set.
        let mut non_canonical = canonical.clone();
        non_canonical[0] = 1;
        assert_ne!(non_canonical, canonical);

        let mut cursor = Cursor::new(&non_canonical[..]);
        let result = Affine::<P>::deserialize_with_mode(&mut cursor, Compress::Yes, validate);

        // Before the fix this succeeds and yields the identity, which is the
        // bug: two byte strings, one value. The assertion below is written
        // against the desired behaviour, so it fails on an unfixed tree.
        if let Ok(point) = result {
            let mut round_tripped = Vec::new();
            point.serialize_with_mode(&mut round_tripped, Compress::Yes).unwrap();
            panic!(
                "a non-canonical infinity encoding was accepted ({mode}).\n\
                 It deserialized to infinity = {}, and re-serialized to the canonical form, so\n\
                 {:?}... and {:?}... are two encodings of one value.",
                point.is_zero(),
                &non_canonical[..4],
                &round_tripped[..4],
            );
        }
    }
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
