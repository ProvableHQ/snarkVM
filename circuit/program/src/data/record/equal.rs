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

impl<A: Aleo, Private: Visibility<A>> Equal<Self> for Record<A, Private> {
    type Output = Boolean<A>;

    /// Returns `true` if `self` and `other` are equal.
    ///
    /// Note: This method does **not** check the `nonce` equality.
    fn is_equal(&self, other: &Self) -> Self::Output {
        // Ensure the `data` are the same length.
        if self.data.len() != other.data.len() {
            return Boolean::constant(false);
        }
        // Recursively check each entry for equality.
        let mut equal = Boolean::constant(true);
        for ((name_a, entry_a), (name_b, entry_b)) in self.data.iter().zip_eq(other.data.iter()) {
            equal = equal & name_a.is_equal(name_b) & entry_a.is_equal(entry_b);
        }

        // Check the `owner`, `data`, and `nonce`.
        self.owner.is_equal(&other.owner) & equal & self.nonce.is_equal(&other.nonce)
    }

    /// Returns `true` if `self` and `other` are *not* equal.
    ///
    /// Note: This method does **not** check the `nonce` equality.
    fn is_not_equal(&self, other: &Self) -> Self::Output {
        // Check the `data` lengths.
        if self.data.len() != other.data.len() {
            return Boolean::constant(true);
        }
        // Recursively check each entry for inequality.
        let mut not_equal = Boolean::constant(false);
        for ((name_a, entry_a), (name_b, entry_b)) in self.data.iter().zip_eq(other.data.iter()) {
            not_equal = not_equal | name_a.is_not_equal(name_b) | entry_a.is_not_equal(entry_b);
        }

        // Check the `owner`, `data`, and `nonce`.
        self.owner.is_not_equal(&other.owner) | not_equal | self.nonce.is_not_equal(&other.nonce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Circuit;

    fn sample_record(mode: Mode) -> Record<Circuit, Plaintext<Circuit>> {
        let record = console::Record::<
            <Circuit as Environment>::Network,
            console::Plaintext<<Circuit as Environment>::Network>,
        >::from_str(
            r"{
    owner: aleo14tlamssdmg3d0p5zmljma573jghe2q9n6wz29qf36re2glcedcpqfg4add.private,
    a: true.private,
    b: 123456789field.public,
    c: 0group.private,
    d: {
        e: true.private,
        f: 123456789field.private,
        g: 0group.private
    },
    _nonce: 0group.public
}",
        )
        .unwrap();
        Record::new(mode, record)
    }

    fn sample_mismatched_record(mode: Mode) -> Record<Circuit, Plaintext<Circuit>> {
        let record = console::Record::<
            <Circuit as Environment>::Network,
            console::Plaintext<<Circuit as Environment>::Network>,
        >::from_str(
            r"{
    owner: aleo14tlamssdmg3d0p5zmljma573jghe2q9n6wz29qf36re2glcedcpqfg4add.private,
    a: true.public,
    b: 123456789field.public,
    c: 0group.private,
    d: {
        e: true.private,
        f: 123456789field.private,
        g: 0group.private
    },
    _nonce: 0group.public
}",
        )
        .unwrap();
        Record::new(mode, record)
    }

    fn check_is_equal(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        // Sample the record.
        let record = sample_record(mode);
        let mismatched_record = sample_mismatched_record(mode);

        Circuit::scope(format!("{mode}"), || {
            let candidate = record.is_equal(&record);
            assert!(candidate.eject_value());
            assert_scope!(num_constants, num_public, num_private, num_constraints);
        });

        Circuit::scope(format!("{mode}"), || {
            let candidate = record.is_equal(&mismatched_record);
            assert!(!candidate.eject_value());
        });

        Circuit::reset();
        Ok(())
    }

    fn check_is_not_equal(
        mode: Mode,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
    ) -> Result<()> {
        // Sample the record.
        let record = sample_record(mode);
        let mismatched_record = sample_mismatched_record(mode);

        Circuit::scope(format!("{mode}"), || {
            let candidate = record.is_not_equal(&mismatched_record);
            assert!(candidate.eject_value());
            assert_scope!(num_constants, num_public, num_private, num_constraints);
        });

        Circuit::scope(format!("{mode}"), || {
            let candidate = record.is_not_equal(&record);
            assert!(!candidate.eject_value());
        });

        Circuit::reset();
        Ok(())
    }

    #[test]
    fn test_is_equal_constant() -> Result<()> {
        // Note: This is correct. At this (high) level of a program, we override the default mode in the `Record` case,
        // based on the user-defined visibility in the record type. Thus, we have nonzero private and constraint values.
        check_is_equal(Mode::Constant, 17, 0, 23, 33)
    }

    #[test]
    fn test_is_equal_public() -> Result<()> {
        check_is_equal(Mode::Public, 17, 0, 23, 33)
    }

    #[test]
    fn test_is_equal_private() -> Result<()> {
        check_is_equal(Mode::Private, 17, 0, 23, 33)
    }

    #[test]
    fn test_is_not_equal_constant() -> Result<()> {
        // Note: This is correct. At this (high) level of a program, we override the default mode in the `Record` case,
        // based on the user-defined visibility in the record type. Thus, we have nonzero private and constraint values.
        check_is_not_equal(Mode::Constant, 7, 0, 27, 27)
    }

    #[test]
    fn test_is_not_equal_public() -> Result<()> {
        check_is_not_equal(Mode::Public, 7, 0, 27, 27)
    }

    #[test]
    fn test_is_not_equal_private() -> Result<()> {
        check_is_not_equal(Mode::Private, 7, 0, 27, 27)
    }
}
