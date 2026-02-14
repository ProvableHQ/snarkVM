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

/// The reason a transaction was rejected.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum RejectedReason {
    Placeholder,
}

impl RejectedReason {
    // TODO (raychu86): Rejected Reason
    pub fn new(_x: String) -> Self {
        Self::Placeholder
    }
}

impl FromBytes for RejectedReason {
    /// Reads the rejected reason from a buffer.
    fn read_le<R: Read>(mut _reader: R) -> IoResult<Self> {
        unimplemented!()
    }
}

impl ToBytes for RejectedReason {
    /// Writes the rejected reason to a buffer.
    fn write_le<W: Write>(&self, mut _writer: W) -> IoResult<()> {
        unimplemented!()
    }
}

impl Serialize for RejectedReason {
    /// Serializes the rejected reason into string or bytes.
    fn serialize<S: Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        unimplemented!()
    }
}

impl<'de> Deserialize<'de> for RejectedReason {
    /// Deserializes the rejected reason from a string or bytes.
    fn deserialize<D: Deserializer<'de>>(_deserializer: D) -> Result<Self, D::Error> {
        unimplemented!()
    }
}

impl FromStr for RejectedReason {
    type Err = Error;

    /// Initializes the rejected reason from a JSON-string.
    fn from_str(_reason: &str) -> Result<Self, Self::Err> {
        unimplemented!()
    }
}

impl Debug for RejectedReason {
    /// Prints the rejected reason as a JSON-string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for RejectedReason {
    /// Displays the rejected reason as a JSON-string.
    fn fmt(&self, _f: &mut Formatter) -> fmt::Result {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    // TODO (raychu86): Rejected Reason

    #[test]
    fn test_bytes() {
        unimplemented!()
    }

    #[test]
    fn test_serde_json() {
        unimplemented!()
    }

    #[test]
    fn test_bincode() {
        unimplemented!()
    }

    #[test]
    fn test_string() {
        unimplemented!()
    }
}
