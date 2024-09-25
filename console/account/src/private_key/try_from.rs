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
use snarkvm_console_types::Field;

static ACCOUNT_SK_SIG_DOMAIN: &str = "AleoAccountSignatureSecretKey0";
static ACCOUNT_R_SIG_DOMAIN: &str = "AleoAccountSignatureRandomizer0";

impl<N: Network> PrivateKey<N> {
    /// Returns the account private key from an account seed.
    #[inline]
    pub fn try_from(seed: Field<N>) -> Result<Self> {
        // Construct the sk_sig domain separator.
        let sk_sig_domain = Field::<N>::new_domain_separator(ACCOUNT_SK_SIG_DOMAIN);

        // Construct the r_sig domain separator.
        let r_sig_input = format!("{}.{}", ACCOUNT_R_SIG_DOMAIN, 0);
        let r_sig_domain = Field::new_domain_separator(&r_sig_input);

        Ok(Self {
            seed,
            sk_sig: N::hash_to_scalar_psd2(&[sk_sig_domain, seed])?,
            r_sig: N::hash_to_scalar_psd2(&[r_sig_domain, seed])?,
        })
    }
}
