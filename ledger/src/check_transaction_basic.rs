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

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Checks the given transaction is well-formed and unique.
    pub fn check_transaction_basic<R: CryptoRng + Rng>(
        &self,
        transaction: &Transaction<N>,
        rejected_id: Option<Field<N>>,
        rng: &mut R,
    ) -> Result<()> {
        self.vm().check_transaction(transaction, rejected_id, rng)
    }

    /// Checks that the given list of transactions are well-formed and unique.
    pub fn check_transactions_basic<R: CryptoRng + Rng>(
        &self,
        transactions: &[(&Transaction<N>, Option<Field<N>>)],
        rng: &mut R,
    ) -> Result<()> {
        self.vm().check_transactions(transactions, rng)
    }
}
