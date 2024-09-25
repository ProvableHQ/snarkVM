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

impl<N: Network, Private: Visibility> Record<N, Private> {
    /// A helper method to derive the serial number from the private key and commitment.
    pub fn serial_number(private_key: PrivateKey<N>, commitment: Field<N>) -> Result<Field<N>> {
        // Compute the generator `H` as `HashToGroup(commitment)`.
        let h = N::hash_to_group_psd2(&[N::serial_number_domain(), commitment])?;
        // Compute `gamma` as `sk_sig * H`.
        let gamma = h * private_key.sk_sig();
        // Compute the serial number from `gamma`.
        Self::serial_number_from_gamma(&gamma, commitment)
    }

    /// A helper method to derive the serial number from the gamma and commitment.
    pub fn serial_number_from_gamma(gamma: &Group<N>, commitment: Field<N>) -> Result<Field<N>> {
        // Compute `sn_nonce` as `Hash(COFACTOR * gamma)`.
        let sn_nonce = N::hash_to_scalar_psd2(&[N::serial_number_domain(), gamma.mul_by_cofactor().to_x_coordinate()])?;
        // Compute `serial_number` as `Commit(commitment, sn_nonce)`.
        N::commit_bhp512(&(N::serial_number_domain(), commitment).to_bits_le(), &sn_nonce)
    }
}
