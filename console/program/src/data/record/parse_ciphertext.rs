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

static RECORD_CIPHERTEXT_PREFIX: &str = "record";

impl<N: Network> Parser for Record<N, Ciphertext<N>> {
    /// Parses a string into an ciphertext.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // Prepare a parser for the record ciphertext.
        let parse_record_ciphertext = recognize(pair(
            pair(tag(RECORD_CIPHERTEXT_PREFIX), tag("1")),
            many1(terminated(one_of("qpzry9x8gf2tvdw0s3jn54khce6mua7l"), many0(char('_')))),
        ));

        // Parse the record ciphertext from the string.
        map_res(parse_record_ciphertext, |record_ciphertext: &str| -> Result<_, Error> {
            Self::from_str(&record_ciphertext.replace('_', ""))
        })(string)
    }
}

impl<N: Network> FromStr for Record<N, Ciphertext<N>> {
    type Err = Error;

    /// Reads in the ciphertext string.
    fn from_str(ciphertext: &str) -> Result<Self, Self::Err> {
        // Decode the ciphertext string from bech32m.
        let (hrp, data, variant) = bech32::decode(ciphertext)?;
        if hrp != RECORD_CIPHERTEXT_PREFIX {
            bail!("Failed to decode record ciphertext: '{hrp}' is an invalid prefix")
        } else if data.is_empty() {
            bail!("Failed to decode record ciphertext: data field is empty")
        } else if variant != bech32::Variant::Bech32m {
            bail!("Found a record ciphertext that is not bech32m encoded: {ciphertext}");
        }
        // Decode the record ciphertext data from u5 to u8, and into the record ciphertext.
        Ok(Self::read_le(&Vec::from_base32(&data)?[..])?)
    }
}

impl<N: Network> Debug for Record<N, Ciphertext<N>> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for Record<N, Ciphertext<N>> {
    /// Writes the record ciphertext as a bech32m string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // Convert the ciphertext to bytes.
        let bytes = self.to_bytes_le().map_err(|_| fmt::Error)?;
        // Encode the bytes into bech32m.
        let string = bech32::encode(RECORD_CIPHERTEXT_PREFIX, bytes.to_base32(), bech32::Variant::Bech32m)
            .map_err(|_| fmt::Error)?;
        // Output the string.
        Display::fmt(&string, f)
    }
}
