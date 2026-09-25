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

use super::*;

use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
};

/// The number of decoded points a thread keeps before its cache is cleared.
const DECODED_POINTS_CAPACITY: usize = 4096;

/// Decoded points keyed by environment and x-coordinate bytes. Each value is a `Group<E>` for the
/// environment in its key.
type DecodedPoints = HashMap<(TypeId, Vec<u8>), Box<dyn Any>>;

thread_local! {
    /// Points this thread decoded with [`Group::read_le`]. Only points that passed
    /// [`Group::from_x_coordinate`] are kept.
    static DECODED_POINTS: RefCell<DecodedPoints> = RefCell::new(HashMap::new());
}

impl<E: Environment> FromBytes for Group<E> {
    /// Reads the group from a buffer.
    ///
    /// Decoded points are cached per thread, so bytes seen again, such as a repeated address,
    /// skip the curve and subgroup checks.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let x_coordinate: Field<E> = FromBytes::read_le(&mut reader)?;
        let key = (TypeId::of::<E>(), x_coordinate.to_bytes_le().map_err(into_io_error)?);
        let cached = DECODED_POINTS
            .with_borrow(|points| points.get(&key).and_then(|point| point.downcast_ref::<Self>().copied()));
        if let Some(group) = cached {
            return Ok(group);
        }

        let group = Self::from_x_coordinate(x_coordinate).map_err(into_io_error)?;
        DECODED_POINTS.with_borrow_mut(|points| {
            if points.len() >= DECODED_POINTS_CAPACITY {
                points.clear();
            }
            points.insert(key, Box::new(group));
        });
        Ok(group)
    }
}

impl<E: Environment> ToBytes for Group<E> {
    /// Writes the group to a buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.to_x_coordinate().write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network_environment::Console;

    type CurrentEnvironment = Console;

    const ITERATIONS: u64 = 10_000;

    #[test]
    fn test_bytes() -> Result<()> {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Sample a new group.
            let expected = Group::<CurrentEnvironment>::new(Uniform::rand(&mut rng));

            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, Group::read_le(&expected_bytes[..])?);
            assert_eq!(expected, Group::read_le_unchecked(&expected_bytes[..])?);
            assert!(Group::<CurrentEnvironment>::read_le(&expected_bytes[1..]).is_err());
            assert!(Group::<CurrentEnvironment>::read_le_unchecked(&expected_bytes[1..]).is_err());
        }
        Ok(())
    }

    #[test]
    fn test_read_le_repeats_and_refills_the_cache() -> Result<()> {
        let mut rng = TestRng::default();

        // More points than the cache holds, read twice, so the cache is cleared and refilled.
        let points = (0..DECODED_POINTS_CAPACITY + 16)
            .map(|_| Group::<CurrentEnvironment>::new(Uniform::rand(&mut rng)))
            .collect::<Vec<_>>();
        for _ in 0..2 {
            for point in &points {
                assert_eq!(Group::read_le(&point.to_bytes_le()?[..])?, *point);
            }
        }
        Ok(())
    }

    #[test]
    fn test_read_le_rejects_invalid_bytes_twice() {
        let mut rng = TestRng::default();

        // An x-coordinate with no point in the subgroup is rejected every time; failures are not cached.
        let x_coordinate = std::iter::repeat_with(|| Field::<CurrentEnvironment>::rand(&mut rng))
            .find(|x_coordinate| Group::<CurrentEnvironment>::from_x_coordinate(*x_coordinate).is_err())
            .unwrap();
        let bytes = x_coordinate.to_bytes_le().unwrap();
        assert!(Group::<CurrentEnvironment>::read_le(&bytes[..]).is_err());
        assert!(Group::<CurrentEnvironment>::read_le(&bytes[..]).is_err());
    }
}
