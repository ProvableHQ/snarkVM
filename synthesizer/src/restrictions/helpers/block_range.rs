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

use console::{
    network::Network,
    prelude::{de, Deserialize, Deserializer, EnumAccess, One, Result, Serialize, Serializer, VariantAccess, Visitor},
    types::Field,
};

use core::{
    fmt,
    ops::{Range, RangeFrom, RangeInclusive, RangeTo},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockRange {
    Range(Range<u32>),
    RangeFrom(RangeFrom<u32>),
    RangeTo(RangeTo<u32>),
    RangeInclusive(RangeInclusive<u32>),
    FullRange,
}

impl BlockRange {
    /// Returns `true` if the block range contains the specified block height.
    pub fn contains(&self, height: u32) -> bool {
        match self {
            BlockRange::Range(range) => range.contains(&height),
            BlockRange::RangeFrom(range) => range.contains(&height),
            BlockRange::RangeTo(range) => range.contains(&height),
            BlockRange::RangeInclusive(range) => range.contains(&height),
            BlockRange::FullRange => true,
        }
    }
}

impl BlockRange {
    /// Returns a unique field element encoding of the block range.
    pub fn to_fields<N: Network>(&self) -> Result<Vec<Field<N>>> {
        match self {
            BlockRange::Range(range) => {
                Ok(vec![Field::from_u8(0), Field::from_u32(range.start), Field::from_u32(range.end)])
            }
            BlockRange::RangeFrom(range) => Ok(vec![Field::from_u8(1), Field::from_u32(range.start), -Field::one()]),
            BlockRange::RangeTo(range) => Ok(vec![Field::from_u8(2), -Field::one(), Field::from_u32(range.end)]),
            BlockRange::RangeInclusive(range) => {
                Ok(vec![Field::from_u8(3), Field::from_u32(*range.start()), Field::from_u32(*range.end())])
            }
            BlockRange::FullRange => Ok(vec![Field::from_u8(4), -Field::one(), -Field::one()]),
        }
    }
}

impl Serialize for BlockRange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            BlockRange::Range(range) => {
                serializer.serialize_newtype_variant("BlockRange", 0, "Range", &(range.start, range.end))
            }
            BlockRange::RangeFrom(range) => {
                serializer.serialize_newtype_variant("BlockRange", 1, "RangeFrom", &range.start)
            }
            BlockRange::RangeTo(range) => serializer.serialize_newtype_variant("BlockRange", 2, "RangeTo", &range.end),
            BlockRange::RangeInclusive(range) => {
                serializer.serialize_newtype_variant("BlockRange", 3, "RangeInclusive", &(range.start(), range.end()))
            }
            BlockRange::FullRange => serializer.serialize_newtype_variant("BlockRange", 4, "FullRange", &()),
        }
    }
}

impl<'de> Deserialize<'de> for BlockRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Range,
            RangeFrom,
            RangeTo,
            RangeInclusive,
            FullRange,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`Range`, `RangeFrom`, `RangeTo`, `RangeInclusive`, or `FullRange`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "Range" => Ok(Field::Range),
                            "RangeFrom" => Ok(Field::RangeFrom),
                            "RangeTo" => Ok(Field::RangeTo),
                            "RangeInclusive" => Ok(Field::RangeInclusive),
                            "FullRange" => Ok(Field::FullRange),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct BlockRangeVisitor;

        impl<'de> Visitor<'de> for BlockRangeVisitor {
            type Value = BlockRange;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct BlockRange")
            }

            fn visit_enum<A>(self, data: A) -> Result<BlockRange, A::Error>
            where
                A: EnumAccess<'de>,
            {
                let (field, variant) = data.variant()?;
                match field {
                    Field::Range => {
                        let (start, end): (u32, u32) = variant.newtype_variant()?;
                        Ok(BlockRange::Range(start..end))
                    }
                    Field::RangeFrom => {
                        let start: u32 = variant.newtype_variant()?;
                        Ok(BlockRange::RangeFrom(RangeFrom { start }))
                    }
                    Field::RangeTo => {
                        let end: u32 = variant.newtype_variant()?;
                        Ok(BlockRange::RangeTo(RangeTo { end }))
                    }
                    Field::RangeInclusive => {
                        let (start, end): (u32, u32) = variant.newtype_variant()?;
                        Ok(BlockRange::RangeInclusive(RangeInclusive::new(start, end)))
                    }
                    Field::FullRange => {
                        variant.newtype_variant()?;
                        Ok(BlockRange::FullRange)
                    }
                }
            }
        }

        const FIELDS: &[&str] = &["Range", "RangeFrom", "RangeTo", "RangeInclusive", "FullRange"];
        deserializer.deserialize_enum("BlockRange", FIELDS, BlockRangeVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type CurrentNetwork = console::network::MainnetV0;

    #[test]
    fn test_serialize_range() {
        let range = BlockRange::Range(1..10);
        let serialized = serde_json::to_string(&range).unwrap();
        assert_eq!(serialized, r#"{"Range":[1,10]}"#);
    }

    #[test]
    fn test_deserialize_range() {
        let serialized = r#"{"Range":[1,10]}"#;
        let deserialized: BlockRange = serde_json::from_str(serialized).unwrap();
        assert_eq!(deserialized, BlockRange::Range(1..10));
    }

    #[test]
    fn test_serialize_range_from() {
        let range = BlockRange::RangeFrom(RangeFrom { start: 5 });
        let serialized = serde_json::to_string(&range).unwrap();
        assert_eq!(serialized, r#"{"RangeFrom":5}"#);
    }

    #[test]
    fn test_deserialize_range_from() {
        let serialized = r#"{"RangeFrom":5}"#;
        let deserialized: BlockRange = serde_json::from_str(serialized).unwrap();
        assert_eq!(deserialized, BlockRange::RangeFrom(RangeFrom { start: 5 }));
    }

    #[test]
    fn test_serialize_range_to() {
        let range = BlockRange::RangeTo(RangeTo { end: 8 });
        let serialized = serde_json::to_string(&range).unwrap();
        assert_eq!(serialized, r#"{"RangeTo":8}"#);
    }

    #[test]
    fn test_deserialize_range_to() {
        let serialized = r#"{"RangeTo":8}"#;
        let deserialized: BlockRange = serde_json::from_str(serialized).unwrap();
        assert_eq!(deserialized, BlockRange::RangeTo(RangeTo { end: 8 }));
    }

    #[test]
    fn test_serialize_range_inclusive() {
        let range = BlockRange::RangeInclusive(RangeInclusive::new(2, 9));
        let serialized = serde_json::to_string(&range).unwrap();
        assert_eq!(serialized, r#"{"RangeInclusive":[2,9]}"#);
    }

    #[test]
    fn test_deserialize_range_inclusive() {
        let serialized = r#"{"RangeInclusive":[2,9]}"#;
        let deserialized: BlockRange = serde_json::from_str(serialized).unwrap();
        assert_eq!(deserialized, BlockRange::RangeInclusive(RangeInclusive::new(2, 9)));
    }

    #[test]
    fn test_serialize_full_range() {
        let range = BlockRange::FullRange;
        let serialized = serde_json::to_string(&range).unwrap();
        assert_eq!(serialized, r#"{"FullRange":null}"#);
    }

    #[test]
    fn test_deserialize_full_range() {
        let serialized = r#"{"FullRange":null}"#;
        let deserialized: BlockRange = serde_json::from_str(serialized).unwrap();
        assert_eq!(deserialized, BlockRange::FullRange);
    }

    #[test]
    fn test_deserialize_error() {
        let serialized = r#"{"InvalidRange":[1,2]}"#;
        let result: Result<BlockRange, _> = serde_json::from_str(serialized);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_missing_field() {
        let serialized = r#"{}"#;
        let result: Result<BlockRange, _> = serde_json::from_str(serialized);
        assert!(result.is_err());
    }

    #[test]
    fn test_contains() {
        let range = BlockRange::Range(1..10);
        assert!(range.contains(1));
        assert!(range.contains(5));
        assert!(range.contains(9));
        assert!(!range.contains(10));

        let range = BlockRange::RangeFrom(RangeFrom { start: 5 });
        assert!(!range.contains(4));
        assert!(range.contains(5));
        assert!(range.contains(6));

        let range = BlockRange::RangeTo(RangeTo { end: 8 });
        assert!(range.contains(7));
        assert!(!range.contains(8));
        assert!(!range.contains(9));

        let range = BlockRange::RangeInclusive(RangeInclusive::new(2, 9));
        assert!(!range.contains(1));
        assert!(range.contains(2));
        assert!(range.contains(9));
        assert!(!range.contains(10));

        let range = BlockRange::FullRange;
        assert!(range.contains(0));
        assert!(range.contains(1));
        assert!(range.contains(1000));
    }

    #[test]
    fn test_to_fields() {
        let range = BlockRange::Range(1..10);
        let fields = range.to_fields::<CurrentNetwork>().unwrap();
        assert_eq!(fields, vec![Field::from_u8(0), Field::from_u32(1), Field::from_u32(10)]);

        let range = BlockRange::RangeFrom(RangeFrom { start: 5 });
        let fields = range.to_fields::<CurrentNetwork>().unwrap();
        assert_eq!(fields, vec![Field::from_u8(1), Field::from_u32(5), -Field::one()]);

        let range = BlockRange::RangeTo(RangeTo { end: 8 });
        let fields = range.to_fields::<CurrentNetwork>().unwrap();
        assert_eq!(fields, vec![Field::from_u8(2), -Field::one(), Field::from_u32(8)]);

        let range = BlockRange::RangeInclusive(RangeInclusive::new(2, 9));
        let fields = range.to_fields::<CurrentNetwork>().unwrap();
        assert_eq!(fields, vec![Field::from_u8(3), Field::from_u32(2), Field::from_u32(9)]);

        let range = BlockRange::FullRange;
        let fields = range.to_fields::<CurrentNetwork>().unwrap();
        assert_eq!(fields, vec![Field::from_u8(4), -Field::one(), -Field::one()]);
    }
}
