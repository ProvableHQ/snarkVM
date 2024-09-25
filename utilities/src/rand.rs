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

use rand::{
    distributions::{Distribution, Standard},
    rngs::StdRng,
    Rng,
    SeedableRng,
};
use rand_xorshift::XorShiftRng;

/// A trait for a uniform random number generator.
pub trait Uniform: Sized {
    /// Samples a random value from a uniform distribution.
    fn rand<R: Rng + ?Sized>(rng: &mut R) -> Self;
}

impl<T> Uniform for T
where
    Standard: Distribution<T>,
{
    #[inline]
    fn rand<R: Rng + ?Sized>(rng: &mut R) -> Self {
        rng.sample(Standard)
    }
}

/// A fast RNG used **solely** for testing and benchmarking, **not** for any real world purposes.
pub struct TestRng(XorShiftRng);

impl Default for TestRng {
    fn default() -> Self {
        // Obtain the initial seed using entropy provided by the OS.
        let seed = StdRng::from_entropy().gen();

        // Use it as the basis for the underlying Rng.
        Self::fixed(seed)
    }
}

impl TestRng {
    pub fn fixed(seed: u64) -> Self {
        // Print the seed, so it's displayed if any of the tests using `test_rng` fails.
        println!("\nInitializing 'TestRng' with seed '{seed}'\n");

        // Use the seed to initialize a fast, non-cryptographic Rng.
        Self::from_seed(seed)
    }

    // This is the preferred method to use once the main instance of TestRng had already
    // been initialized in a test or benchmark and an auxiliary one is desired without
    // spamming the stdout.
    pub fn from_seed(seed: u64) -> Self {
        Self(XorShiftRng::seed_from_u64(seed))
    }

    /// Returns a randomly-sampled `String`, given the maximum size in bytes and an RNG.
    ///
    /// Some of the snarkVM internal tests involve the random generation of strings,
    /// which are parsed and tested against the original ones. However, since the string parser
    /// rejects certain characters, if those characters are randomly generated, the tests fail.
    ///
    /// To prevent these failures, as we randomly generate the characters,
    /// we ensure that they are not among the ones rejected by the parser;
    /// if they are, we adjust them to be allowed characters.
    ///
    /// Note that the randomness of the characters is strictly for **testing** purposes;
    /// also note that the disallowed characters are a small fraction of the total set of characters,
    /// and thus the adjustments rarely occur.
    pub fn next_string(&mut self, max_bytes: u32, is_fixed_size: bool) -> String {
        /// Adjust an unsafe character.
        ///
        /// As our parser rejects certain potentially unsafe characters (see `Sanitizer::parse_safe_char`),
        /// we need to avoid generating them randomly. This function acts as an adjusting filter:
        /// it changes an unsafe character to `'0'` (other choices are possible), and leaves other
        /// characters unchanged.
        fn adjust_unsafe_char(ch: char) -> char {
            let code = ch as u32;
            if code < 9
                || code == 11
                || code == 12
                || (14..=31).contains(&code)
                || code == 127
                || (0x202a..=0x202e).contains(&code)
                || (0x2066..=0x2069).contains(&code)
            {
                '0'
            } else {
                ch
            }
        }

        /// Adjust a backslash and a double quote.
        ///
        /// Aside from the characters rejected through the function [adjust_unsafe_char],
        /// the syntax of strings allows backslash and double quotes only in certain circumstances:
        /// backslash is used to introduce an escape, and there are constraints on what can occur
        /// after a backslash; double quotes is only used in escaped form just after a backslash.
        ///
        /// If we randomly sample characters, we may end up generating backslashes with
        /// malformed escape syntax, or double quotes not preceded by backslash. Thus,
        /// we also adjust backslashes and double quotes as we randomly sample characters.
        ///
        /// Note that, this way, we do not test the parsing of any escape sequences;
        /// to do that, we would need to reify the possible elements of strings,
        /// namely characters and escapes, and randomly generate such elements.
        fn adjust_backslash_and_doublequote(ch: char) -> char {
            if ch == '\\' || ch == '\"' { '0' } else { ch }
        }

        let range = match is_fixed_size {
            true => 0..max_bytes,
            false => 0..self.gen_range(0..max_bytes),
        };

        range.map(|_| self.gen::<char>()).map(adjust_unsafe_char).map(adjust_backslash_and_doublequote).collect()
    }
}

impl rand::RngCore for TestRng {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.0.try_fill_bytes(dest)
    }
}

impl rand::CryptoRng for TestRng {}
