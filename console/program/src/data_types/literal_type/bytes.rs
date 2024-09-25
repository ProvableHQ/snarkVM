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

impl FromBytes for LiteralType {
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let index = u8::read_le(&mut reader)?;
        FromPrimitive::from_u8(index).ok_or_else(|| error("Failed to deserialize literal type variant {index}"))
    }
}

impl ToBytes for LiteralType {
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.type_id().write_le(&mut writer)
    }
}
