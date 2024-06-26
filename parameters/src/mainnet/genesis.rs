// Copyright (C) 2019-2023 Aleo Systems Inc.
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

pub struct GenesisBytes;

impl GenesisBytes {
    pub const fn load_bytes() -> &'static [u8] {
        include_bytes!("./resources/block.genesis")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_block() {
        let bytes = GenesisBytes::load_bytes();
        assert_eq!(20083, bytes.len() as u64, "Update me if serialization has changed");
    }
}
