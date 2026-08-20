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

// Originally derived from WHIR (https://github.com/WizardOfMenlo/whir),
// licensed under Apache-2.0 OR MIT.

//! Serializable type information so data contains type information and
//! deserialization checks it.
//!
//! This makes sure `Config` objects can e.g. only be deserialized to instances
//! for the same field.

use std::{
    fmt::{self, Debug, Formatter},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use derive_where::derive_where;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

/// Types that can provide serializable type information for identification.
pub trait TypeInfo {
    type Info: Debug + PartialEq + Eq + Serialize + for<'de> Deserialize<'de>;

    fn type_info() -> Self::Info;
}

/// Zero-sized type that serializes into [`TypeInfo::type_info`].
#[derive_where(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Type<T: TypeInfo>(PhantomData<T>);

/// Wrapper that adds typeinfo when serializing.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Typed<T: TypeInfo>(pub T);

impl<T: TypeInfo> Type<T> {
    /// Creates a new type instance.
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: TypeInfo> Debug for Type<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        T::type_info().fmt(f)
    }
}

impl<T: TypeInfo> Serialize for Type<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        T::type_info().serialize(serializer)
    }
}

impl<'de, T: TypeInfo> Deserialize<'de> for Type<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let expected = T::type_info();
        let got = T::Info::deserialize(deserializer)?;
        if expected == got {
            Ok(Self(PhantomData))
        } else {
            Err(D::Error::custom(format!("Type mismatch, expected: {expected:?}, got: {got:?}")))
        }
    }
}

impl<T: TypeInfo> Typed<T> {
    /// Creates a new type instance.
    pub const fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T: TypeInfo + Debug> Debug for Typed<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: TypeInfo> Deref for Typed<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: TypeInfo> DerefMut for Typed<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: TypeInfo + Serialize> Serialize for Typed<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct TypedValue<'s, T: TypeInfo> {
            #[serde(rename = "type")]
            type_: Type<T>,
            value: &'s T,
        }
        TypedValue { type_: Type::new(), value: &self.0 }.serialize(serializer)
    }
}

impl<'de, T: TypeInfo + Deserialize<'de>> Deserialize<'de> for Typed<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct TypedValue<T: TypeInfo> {
            #[serde(rename = "type")]
            #[allow(unused)]
            type_: Type<T>,
            value: T,
        }
        let read = TypedValue::deserialize(deserializer)?;
        Ok(Self(read.value))
    }
}

#[cfg(any())]
mod tests {
    use static_assertions::const_assert_eq;

    use super::*;
    use crate::snark::provekit::whir::{
        algebra::fields::{Field64_2, Field64_3, Field128, Field256},
        utils::test_serde,
    };

    const_assert_eq!(size_of::<Type<Field256>>(), 0);

    #[test]
    fn test_roundtrip() {
        test_serde(&Type::<Field256>::new());
        test_serde(&Type::<Field64_3>::new());
    }

    #[test]
    fn test_type_mismatch() {
        let value = Type::<Field128>::new();
        assert_eq!(size_of_val(&value), 0);
        let json = serde_json::to_string_pretty(&value).expect("json serialization failed");

        let result: Result<Type<Field64_2>, _> = serde_json::from_str(&json);
        assert!(result.is_err());
    }
}
