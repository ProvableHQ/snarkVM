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

impl<N: Network> Record<N, Plaintext<N>> {
    /// Returns the record commitment.
    pub fn to_commitment(
        &self,
        program_id: &ProgramID<N>,
        record_name: &Identifier<N>,
        record_view_key: &Field<N>,
    ) -> Result<Field<N>> {
        // Ensure the bit encoding of the record fits its size prefixes, so hashing it cannot halt.
        self.check_bit_lengths()?;

        // Construct the input as `(program_id || record_name || record)`.
        let input = to_bits_le![program_id, record_name, self];

        // Version 0 - Construct the input as the *record bits* without the version bits.
        let input_v0 = input[..input.len() - 8].to_vec();
        // Version 0 - Compute the BHP hash of the program record.
        let digest = N::hash_bhp1024(&input_v0)?;

        // Version 1 - Construct the input as the *digest* with the version bits.
        let mut input_v1 = digest.to_bits_le();
        // Append the version bits.
        input_v1.extend_from_slice(&input[input.len() - 8..]);

        // If the record is non-hiding, then return the digest. Otherwise, return the commitment.
        match !self.is_hiding() {
            // Version 0 - Compute the BHP hash of the program record.
            true => Ok(digest),
            // Version 1 - Compute the BHP commitment of the program record.
            false => {
                // Construct the commitment nonce.
                let cm_nonce = N::hash_to_scalar_psd2(&[N::commitment_domain(), *record_view_key])?;
                // Compute the BHP commitment of the program record using the commitment nonce.
                N::commit_bhp512(&input_v1, &cm_nonce)
            }
        }
    }
}

impl<N: Network> Record<N, Ciphertext<N>> {
    /// Returns the record commitment.
    pub fn to_commitment(
        &self,
        _program_id: &ProgramID<N>,
        _record_name: &Identifier<N>,
        _record_view_key: &Field<N>,
    ) -> Result<Field<N>> {
        bail!("Illegal operation: Record::to_commitment() cannot be invoked on the `Ciphertext` variant.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_console_network::MainnetV0;
    use snarkvm_console_types::U64;
    use snarkvm_utilities::{TestRng, Uniform};

    use core::str::FromStr;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_to_commitment_rejects_oversized_entry() {
        let rng = &mut TestRng::default();

        // A record whose entry holds an outer array whose sole element is an array of 1024
        // u64s: the entry's bit encoding exceeds the u16 size prefix that bit-encoding the
        // record writes before each array element.
        let inner = Plaintext::Array(
            core::iter::repeat_n(Plaintext::from(Literal::U64(U64::rand(rng))), 1024).collect(),
            Default::default(),
        );
        let data = indexmap::indexmap! {
            Identifier::from_str("amount").unwrap() => Entry::Private(Plaintext::Array(vec![inner], Default::default())),
        };
        let record = Record::<CurrentNetwork, Plaintext<CurrentNetwork>>::from_plaintext(
            Owner::Public(Address::rand(rng)),
            data,
            Group::rand(rng),
            U8::new(0),
        )
        .unwrap();

        // Hashing the record would halt the process, so the commitment must fail instead.
        let program_id = ProgramID::from_str("token.aleo").unwrap();
        let record_name = Identifier::from_str("token").unwrap();
        let error = record.to_commitment(&program_id, &record_name, &Field::zero()).unwrap_err();
        assert!(error.to_string().contains("exceeds u16::MAX bits"));
    }
}
