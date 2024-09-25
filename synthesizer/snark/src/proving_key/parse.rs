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

static PROVING_KEY: &str = "prover";

impl<N: Network> Parser for ProvingKey<N> {
    /// Parses a string into the proving key.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // Prepare a parser for the Aleo proving key.
        let parse_key = recognize(pair(
            pair(tag(PROVING_KEY), tag("1")),
            many1(terminated(one_of("qpzry9x8gf2tvdw0s3jn54khce6mua7l"), many0(char('_')))),
        ));

        // Parse the proving key from the string.
        map_res(parse_key, |key: &str| -> Result<_, Error> { Self::from_str(&key.replace('_', "")) })(string)
    }
}

impl<N: Network> FromStr for ProvingKey<N> {
    type Err = Error;

    /// Reads in the proving key string.
    fn from_str(key: &str) -> Result<Self, Self::Err> {
        // Decode the proving key string from bech32m.
        let (hrp, data, variant) = bech32::decode(key)?;
        if hrp != PROVING_KEY {
            bail!("Failed to decode proving key: '{hrp}' is an invalid prefix")
        } else if data.is_empty() {
            bail!("Failed to decode proving key: data field is empty")
        } else if variant != bech32::Variant::Bech32m {
            bail!("Found a proving key that is not bech32m encoded: {key}");
        }
        // Decode the proving key data from u5 to u8, and into the proving key.
        Ok(Self::read_le(&Vec::from_base32(&data)?[..])?)
    }
}

impl<N: Network> Debug for ProvingKey<N> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for ProvingKey<N> {
    /// Writes the proving key as a bech32m string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // Convert the proving key to bytes.
        let bytes = self.to_bytes_le().map_err(|_| fmt::Error)?;
        // Encode the bytes into bech32m.
        let string =
            bech32::encode(PROVING_KEY, bytes.to_base32(), bech32::Variant::Bech32m).map_err(|_| fmt::Error)?;
        // Output the string.
        Display::fmt(&string, f)
    }
}
