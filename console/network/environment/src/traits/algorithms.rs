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

use anyhow::Result;

/// A trait for a commitment scheme.
pub trait Commit {
    type Input;
    type Output;
    type Randomizer;

    /// Returns the commitment to the given input and randomizer.
    fn commit(&self, input: &[Self::Input], randomizer: &Self::Randomizer) -> Result<Self::Output>;
}

/// A trait for a commitment scheme.
pub trait CommitUncompressed {
    type Input;
    type Output;
    type Randomizer;

    /// Returns the commitment to the given input and randomizer.
    fn commit_uncompressed(&self, input: &[Self::Input], randomizer: &Self::Randomizer) -> Result<Self::Output>;
}

/// A trait for a hash function.
pub trait Hash {
    type Input;
    type Output;

    /// Returns the hash of the given input.
    fn hash(&self, input: &[Self::Input]) -> Result<Self::Output>;
}

/// A trait for a hash function that produces multiple outputs.
pub trait HashMany {
    type Input;
    type Output;

    /// Returns the hash of the given input.
    fn hash_many(&self, input: &[Self::Input], num_outputs: u16) -> Vec<Self::Output>;
}

/// A trait for a hash function that projects the value to an affine group element.
pub trait HashToGroup {
    type Input;
    type Output;

    /// Returns the hash of the given input.
    fn hash_to_group(&self, input: &[Self::Input]) -> Result<Self::Output>;
}

/// A trait for a hash function that projects the value to a scalar.
pub trait HashToScalar {
    type Input;
    type Output;

    /// Returns the hash of the given input.
    fn hash_to_scalar(&self, input: &[Self::Input]) -> Result<Self::Output>;
}

/// A trait for a hash function of an uncompressed variant.
pub trait HashUncompressed {
    type Input;
    type Output;

    /// Returns the hash of the given input.
    fn hash_uncompressed(&self, input: &[Self::Input]) -> Result<Self::Output>;
}

/// A trait for a pseudorandom function.
pub trait PRF {
    type Seed;
    type Input;
    type Output;

    /// Returns the output for the given seed and input.
    fn prf(&self, seed: &Self::Seed, input: &[Self::Input]) -> Result<Self::Output>;
}
