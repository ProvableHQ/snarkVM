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

use std::iter::ExactSizeIterator;

/// Iterates over a slice of `u64` in *big-endian* order.
#[derive(Debug)]
pub struct BitIteratorBE<Slice> {
    s: Slice,
    n: usize,
}

#[allow(clippy::len_without_is_empty)]
impl<Slice: AsRef<[u64]>> BitIteratorBE<Slice> {
    pub fn new(s: Slice) -> Self {
        let n = s.as_ref().len() * 64;
        BitIteratorBE { s, n }
    }

    /// Construct an iterator that automatically skips any leading zeros.
    /// That is, it skips all zeros before the most-significant one.
    pub fn new_without_leading_zeros(s: Slice) -> impl Iterator<Item = bool> {
        Self::new(s).skip_while(|b| !b)
    }
}

impl<Slice: AsRef<[u64]>> Iterator for BitIteratorBE<Slice> {
    type Item = bool;

    fn next(&mut self) -> Option<bool> {
        if self.n == 0 {
            None
        } else {
            self.n -= 1;
            let part = self.n / 64;
            let bit = self.n - (64 * part);

            Some(self.s.as_ref()[part] & (1 << bit) > 0)
        }
    }
}

impl<Slice: AsRef<[u64]>> ExactSizeIterator for BitIteratorBE<Slice> {
    fn len(&self) -> usize {
        self.n
    }
}

/// Iterates over a slice of `u64` in *little-endian* order.
#[derive(Debug)]
pub struct BitIteratorLE<Slice: AsRef<[u64]>> {
    s: Slice,
    n: usize,
    max_len: usize,
}

impl<Slice: AsRef<[u64]>> BitIteratorLE<Slice> {
    pub fn new(s: Slice) -> Self {
        let n = 0;
        let max_len = s.as_ref().len() * 64;
        BitIteratorLE { s, n, max_len }
    }
}

impl<Slice: AsRef<[u64]>> Iterator for BitIteratorLE<Slice> {
    type Item = bool;

    fn next(&mut self) -> Option<bool> {
        if self.n == self.max_len {
            None
        } else {
            let part = self.n / 64;
            let bit = self.n - (64 * part);
            self.n += 1;

            Some(self.s.as_ref()[part] & (1 << bit) > 0)
        }
    }
}

impl<Slice: AsRef<[u64]>> ExactSizeIterator for BitIteratorLE<Slice> {
    fn len(&self) -> usize {
        self.max_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bititerator_be() {
        let mut five = BitIteratorBE::new(&[5]);

        for _ in 0..61 {
            assert_eq!(Some(false), five.next());
        }
        assert_eq!(Some(true), five.next());
        assert_eq!(Some(false), five.next());
        assert_eq!(Some(true), five.next());
        assert_eq!(None, five.next());
    }

    #[test]
    fn test_bititerator_be_without_leading_zeros() {
        let mut five = BitIteratorBE::new_without_leading_zeros(&[5]);

        assert_eq!(Some(true), five.next());
        assert_eq!(Some(false), five.next());
        assert_eq!(Some(true), five.next());
        assert_eq!(None, five.next());
    }

    #[test]
    fn test_bititerator_le() {
        let mut five = BitIteratorLE::new(&[5]);

        assert_eq!(Some(true), five.next());
        assert_eq!(Some(false), five.next());
        assert_eq!(Some(true), five.next());
        for _ in 0..61 {
            assert_eq!(Some(false), five.next());
        }
        assert_eq!(None, five.next());
    }
}
