// Copyright (c) 2019-2025 Provable Inc.
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

#![forbid(unsafe_code)]
#![warn(clippy::cast_possible_truncation)]

extern crate snarkvm_console as console;

mod bytes;
mod serialize;
mod string;

use console::{account::Address, prelude::*, program::SUBDAG_CERTIFICATES_DEPTH, types::Field};
use snarkvm_ledger_committee::Committee;
use snarkvm_ledger_narwhal_batch_certificate::BatchCertificate;
use snarkvm_ledger_narwhal_batch_header::BatchHeader;
use snarkvm_ledger_narwhal_compact_certificate::CompactCertificate;
use snarkvm_ledger_narwhal_traits::NarwhalCertificate;
use snarkvm_ledger_puzzle::{PuzzleSolutions, SolutionID};

use bit_set::BitSet;
use indexmap::IndexSet;
use std::collections::BTreeMap;

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

/// Helper enum to wrap the leader certificate.
#[derive(Clone, PartialEq, Eq)]
pub enum LeaderCertificate<'a, N: Network> {
    Full(&'a BatchCertificate<N>),
    Compact(&'a CompactCertificate<N>),
}

impl<'a, N: Network> From<&'a BatchCertificate<N>> for LeaderCertificate<'a, N> {
    fn from(cert: &'a BatchCertificate<N>) -> Self {
        Self::Full(cert)
    }
}

impl<'a, N: Network> From<&'a CompactCertificate<N>> for LeaderCertificate<'a, N> {
    fn from(cert: &'a CompactCertificate<N>) -> Self {
        Self::Compact(cert)
    }
}

impl<'a, N: Network> LeaderCertificate<'a, N> {
    pub fn id(&self) -> Field<N> {
        match self {
            Self::Full(cert) => cert.id(),
            Self::Compact(cert) => cert.id(),
        }
    }

    pub fn author(&self) -> Address<N> {
        match self {
            Self::Full(cert) => cert.author(),
            Self::Compact(cert) => cert.author(),
        }
    }

    pub fn committee_id(&self) -> Field<N> {
        match self {
            Self::Full(cert) => cert.committee_id(),
            Self::Compact(cert) => cert.committee_id(),
        }
    }

    pub fn previous_certificate_ids(&self) -> &IndexSet<Field<N>> {
        match self {
            Self::Full(cert) => cert.previous_certificate_ids(),
            Self::Compact(cert) => cert.previous_certificate_ids(),
        }
    }

    pub fn round(&self) -> u64 {
        match self {
            Self::Full(cert) => cert.round(),
            Self::Compact(cert) => cert.round(),
        }
    }
}

/// Returns `true` if the rounds are sequential.
fn is_sequential<T>(map: &BTreeMap<u64, T>) -> bool {
    let mut previous_round = None;
    for &round in map.keys() {
        match previous_round {
            Some(previous) if previous + 1 != round => return false,
            _ => previous_round = Some(round),
        }
    }
    true
}

/// Checks that the given subDAG is not partitioned and the batches are ordered as expected, by traversing it starting from the leader.
///
/// Returns `true` if the DFS traversal using the given subdag structure matches the commit.
/// Note, this does not guarantee that the subDAG contains all batches it should, because this function has no knowledge of other blocks/subDAGs.
macro_rules! sanity_check_subdag_with_dfs {
    ($subdag:expr) => {{
        use std::collections::HashSet;

        // Initialize a map for the certificates to commit.
        let mut commit = BTreeMap::<u64, IndexSet<_>>::new();
        // Initialize a set for the already ordered certificates.
        let mut already_ordered = HashSet::new();
        // Initialize a buffer for the certificates to order, starting with the leader certificate.
        let mut buffer = $subdag.iter().next_back().map_or(Default::default(), |(_, leader)| leader.clone());
        // Iterate over the certificates to order.
        while let Some(certificate) = buffer.pop() {
            // Insert the certificate into the map.
            commit.entry(certificate.round()).or_default().insert(certificate.clone());
            // Iterate over the previous certificate IDs.
            for previous_certificate_id in certificate.previous_certificate_ids() {
                let Some(previous_certificate) = $subdag
                    .get(&(certificate.round() - 1))
                    .and_then(|map| map.iter().find(|certificate| certificate.id() == *previous_certificate_id))
                else {
                    // It is either ordered or below the GC round.
                    continue;
                };
                // Insert the previous certificate into the set of already ordered certificates.
                if !already_ordered.insert(previous_certificate.id()) {
                    // If the previous certificate is already ordered, continue.
                    continue;
                }
                // Insert the previous certificate into the buffer.
                buffer.insert(previous_certificate.clone());
            }
        }
        // Return `true` if the subdag matches the commit.
        &commit == $subdag
    }};
}

/// Performs sanity checks on the subdag.
macro_rules! sanity_check_subdag {
    ($subdag:expr) => {{
        // Ensure the subdag is not empty.
        ensure!(!$subdag.is_empty(), "Subdag cannot be empty");
        // Ensure the subdag does not exceed the maximum number of rounds.
        ensure!(
            $subdag.len() <= usize::try_from(Self::MAX_ROUNDS)?,
            "Subdag cannot exceed the maximum number of rounds"
        );
        // Ensure the anchor round is even.
        ensure!($subdag.iter().next_back().map_or(0, |(r, _)| *r) % 2 == 0, "Anchor round must be even");
        // Ensure there is only one leader certificate.
        ensure!($subdag.iter().next_back().map_or(0, |(_, c)| c.len()) == 1, "Subdag cannot have multiple leaders");
        // Ensure the rounds are sequential.
        ensure!(is_sequential($subdag), "Subdag rounds must be sequential");
        // Ensure the subdag structure matches the commit.
        ensure!(sanity_check_subdag_with_dfs!($subdag), "Subdag structure does not match commit");
        // Ensure the leader certificate is an even round.
        Ok::<_, Error>(())
    }};
}

/// Returns the weighted median timestamp of the given timestamps and stakes.
fn weighted_median(timestamps_and_stake: Vec<(i64, u64)>) -> i64 {
    let mut timestamps_and_stake = timestamps_and_stake;

    // Sort the timestamps.
    #[cfg(not(feature = "serial"))]
    timestamps_and_stake.par_sort_unstable_by_key(|(timestamp, _)| *timestamp);
    #[cfg(feature = "serial")]
    timestamps_and_stake.sort_unstable_by_key(|(timestamp, _)| *timestamp);

    // Calculate the total stake of the authors.
    let total_stake = timestamps_and_stake.iter().map(|(_, stake)| *stake).sum::<u64>();

    // Initialize the current timestamp and accumulated stake.
    let mut current_timestamp: i64 = 0;
    let mut accumulated_stake: u64 = 0;

    // Find the weighted median timestamp.
    for (timestamp, stake) in timestamps_and_stake.iter() {
        accumulated_stake = accumulated_stake.saturating_add(*stake);
        current_timestamp = *timestamp;
        if accumulated_stake.saturating_mul(2) >= total_stake {
            break;
        }
    }

    // Return the weighted median timestamp
    current_timestamp
}

#[derive(Clone)]
pub enum Subdag<N: Network> {
    Full {
        /// The subdag of round certificates.
        subdag: BTreeMap<u64, IndexSet<BatchCertificate<N>>>,
    },
    Compact {
        /// The subdag of round certificates.
        subdag: BTreeMap<u64, IndexSet<CompactCertificate<N>>>,
    },
}

impl<N: Network> PartialEq for Subdag<N> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Full { subdag }, Self::Full { subdag: other_subdag }) => subdag == other_subdag,
            (Self::Compact { subdag }, Self::Compact { subdag: other_subdag }) => subdag == other_subdag,
            _ => {
                tracing::warn!("Subdag equality check failed - trying to compare a Full to a Compact subdag");
                false
            }
        }
    }
}

impl<N: Network> Eq for Subdag<N> {}

impl<N: Network> Subdag<N> {
    /// Initializes a new subdag.
    pub fn from_full(subdag: BTreeMap<u64, IndexSet<BatchCertificate<N>>>) -> Result<Self> {
        // Perform sanity checks.
        sanity_check_subdag!(&subdag)?;
        // Construct the subdag.
        Ok(Self::Full { subdag })
    }

    /// Initializes a new subdag.
    pub fn from_compact(subdag: BTreeMap<u64, IndexSet<CompactCertificate<N>>>) -> Result<Self> {
        // Perform sanity checks.
        sanity_check_subdag!(&subdag)?;
        // Construct the subdag.
        Ok(Self::Compact { subdag })
    }

    // pub fn get_and_iter(&self, round: &u64) -> Option<Vec<LeaderCertificate<N>>> {
    //     match self {
    //         Self::Full { subdag } => subdag.get(round).map(|certs| certs.iter().map(LeaderCertificate::<N>::from).collect()),
    //         Self::Compact { subdag } => subdag.get(round).map(|certs| certs.iter().map(LeaderCertificate::<N>::from).collect()),
    //     }
    // }

    pub fn check_committee_id_coherence(&self) -> Result<()> {
        macro_rules! checker {
            ($round:expr, $certificates:expr) => {{
                // Check that every certificate for a given round shares the same committee ID.
                let expected_committee_id = $certificates
                    .first()
                    .map(|certificate| certificate.committee_id())
                    .ok_or(anyhow!("No certificates found for subdag round {}", $round))?;
                ensure!(
                    $certificates.iter().skip(1).all(|certificate| certificate.committee_id() == expected_committee_id),
                    "Certificates on round {} do not all have the same committee ID",
                    $round,
                );
                Ok(())
            }};
        }

        match self {
            Self::Full { subdag } => cfg_iter!(subdag).try_for_each(|(round, certs)| checker!(round, certs))?,
            Self::Compact { subdag } => cfg_iter!(subdag).try_for_each(|(round, certs)| checker!(round, certs))?,
        }

        Ok(())
    }
}

impl<N: Network> Subdag<N> {
    /// The maximum number of rounds in a subdag (bounded up to GC depth).
    pub const MAX_ROUNDS: u64 = BatchHeader::<N>::MAX_GC_ROUNDS as u64;
}

impl<N: Network> Subdag<N> {
    /// Returns the anchor round.
    pub fn anchor_round(&self) -> u64 {
        match self {
            Self::Full { subdag } => subdag.iter().next_back().map_or(0, |(round, _)| *round),
            Self::Compact { subdag } => subdag.iter().next_back().map_or(0, |(round, _)| *round),
        }
    }

    /// Returns the certificate IDs of the subdag (from earliest round to latest round).
    pub fn certificate_ids(&self) -> Vec<Field<N>> {
        match self {
            Self::Full { subdag } => subdag.values().flatten().map(BatchCertificate::id).collect_vec(),
            Self::Compact { subdag } => subdag.values().flatten().map(CompactCertificate::id).collect_vec(),
        }
    }

    /// Returns the certificates of the subdag (from earliest round to latest round).
    pub fn into_iter_batch_certificates(self) -> Result<impl Iterator<Item = BatchCertificate<N>>> {
        let Self::Full { subdag } = self else {
            bail!("Can only iter over certificates of Full Subdag.");
        };
        Ok(subdag.into_values().flatten())
    }

    /// Returns the certificate IDs and rounds of the subdag (from earliest round to latest round).
    pub fn certificate_ids_rounds(&self) -> Vec<(Field<N>, u64)> {
        match self {
            Self::Full { subdag } => subdag
                .iter()
                .flat_map(|(round, certificates)| certificates.iter().map(|c| (c.id(), *round)))
                .collect_vec(),
            Self::Compact { subdag } => subdag
                .iter()
                .flat_map(|(round, certificates)| certificates.iter().map(|c| (c.id(), *round)))
                .collect_vec(),
        }
    }

    /// Returns the number of certificates and rounds of the subdag (from earliest round to latest round).
    pub fn num_certificates_rounds(&self) -> Vec<(usize, u64)> {
        match self {
            Self::Full { subdag } => {
                subdag.iter().map(|(round, certificates)| (certificates.len(), *round)).collect_vec()
            }
            Self::Compact { subdag } => {
                subdag.iter().map(|(round, certificates)| (certificates.len(), *round)).collect_vec()
            }
        }
    }

    // /// Returns certificates in this subdag (from earliest round to latest round).
    // pub fn certificates(&self) -> impl Iterator<Item = &BatchCertificate<N>> {
    //     self.values().flatten()
    // }

    /// Returns the leader certificate.
    pub fn leader_certificate(&self) -> LeaderCertificate<N> {
        match self {
            Self::Full { subdag } => {
                // Retrieve entry for the anchor round.
                let entry = subdag.iter().next_back();
                debug_assert!(entry.is_some(), "There must be at least one round of certificates");
                // Retrieve the certificates from the anchor round.
                let certificates = entry.expect("There must be one round in the subdag").1;
                debug_assert!(certificates.len() == 1, "There must be only one leader certificate, by definition");
                // Note: There is guaranteed to be only one leader certificate.
                let cert = certificates.iter().next().expect("There must be a leader certificate");
                LeaderCertificate::Full(cert)
            }
            Self::Compact { subdag } => {
                // Retrieve entry for the anchor round.
                let entry = subdag.iter().next_back();
                debug_assert!(entry.is_some(), "There must be at least one round of certificates");
                // Retrieve the certificates from the anchor round.
                let certificates = entry.expect("There must be one round in the subdag").1;
                debug_assert!(certificates.len() == 1, "There must be only one leader certificate, by definition");
                // Note: There is guaranteed to be only one leader certificate.
                let cert = certificates.iter().next().expect("There must be a leader certificate");
                LeaderCertificate::Compact(cert)
            }
        }
    }

    /// Returns the leader certificate.
    pub fn leader_batch_certificate(&self) -> Result<&BatchCertificate<N>> {
        match self.leader_certificate() {
            LeaderCertificate::Full(cert) => Ok(cert),
            LeaderCertificate::Compact(_) => {
                bail!("We can only return the leader batch certificate of a full Subdag")
            }
        }
    }

    /// Returns the timestamp of the leader certificate.
    pub fn leader_timestamp(&self) -> Result<i64> {
        match self.leader_certificate() {
            LeaderCertificate::Full(cert) => Ok(cert.timestamp()),
            LeaderCertificate::Compact(cert) => Ok(cert.timestamp()),
        }
    }

    /// Return the number of certificates
    pub fn num_certificates(&self) -> usize {
        match self {
            Self::Full { subdag } => subdag.values().flatten().count(),
            Self::Compact { subdag } => subdag.values().flatten().count(),
        }
    }

    /// Return CompactCertificate for a given round
    pub fn get_certificate_for_round(&self, round: u64, certificate_id: &Field<N>) -> Option<&CompactCertificate<N>> {
        let Self::Compact { subdag } = self else {
            return None;
        };
        match subdag.get(&round) {
            Some(certificates) => {
                // Retrieve the certificate for the given certificate ID.
                match certificates.iter().find(|certificate| certificate.id() == *certificate_id) {
                    Some(certificate) => Some(certificate),
                    None => None,
                }
            }
            None => None,
        }
    }

    /// Returns the address of the leader.
    pub fn leader_address(&self) -> Address<N> {
        // Retrieve the leader address from the leader certificate.
        match self.leader_certificate() {
            LeaderCertificate::Full(cert) => cert.author(),
            LeaderCertificate::Compact(cert) => cert.author(),
        }
    }

    /// Returns unique transaction indices of the subdag (from earliest round to latest round).
    pub fn transaction_indices(&self, expected_len: usize) -> Result<BitSet> {
        let Self::Compact { subdag } = self else {
            bail!("A Full Subdag does not have transaction indices.");
        };
        Ok(subdag.values().flatten().map(CompactCertificate::transaction_indices).fold(
            BitSet::with_capacity(expected_len),
            |mut acc, x| {
                acc.union_with(x);
                acc
            },
        ))
    }

    /// Returns unique solution indices of the subdag (from earliest round to latest round).
    pub fn solution_indices(&self, expected_len: usize) -> Result<BitSet> {
        let Self::Compact { subdag } = self else {
            bail!("A Full Subdag does not have solution indices.");
        };
        Ok(subdag.values().flatten().map(CompactCertificate::solution_indices).fold(
            BitSet::with_capacity(expected_len),
            |mut acc, x| {
                acc.union_with(x);
                acc
            },
        ))
    }

    /// Returns the timestamp of the anchor round, defined as the weighted median timestamp of the subdag.
    pub fn timestamp(&self, committee: &Committee<N>) -> i64 {
        // Retrieve the anchor round.
        let anchor_round = self.anchor_round();
        // Retrieve the timestamps and stakes of the certificates for `anchor_round` - 1.
        let timestamps_and_stakes = match self {
            Self::Compact { subdag } => subdag
                .values()
                .flatten()
                .filter(|certificate| certificate.round() == anchor_round.saturating_sub(1))
                .map(|cert| (cert.timestamp(), committee.get_stake(cert.author())))
                .collect::<Vec<_>>(),
            Self::Full { subdag } => subdag
                .values()
                .flatten()
                .filter(|certificate| certificate.round() == anchor_round.saturating_sub(1))
                .map(|cert| (cert.timestamp(), committee.get_stake(cert.author())))
                .collect::<Vec<_>>(),
        };

        // Return the weighted median timestamp.
        weighted_median(timestamps_and_stakes)
    }

    /// Returns the subdag root of the certificates.
    pub fn to_subdag_root(&self) -> Result<Field<N>> {
        // Prepare the leaves.
        let leaves = match self {
            Self::Full { subdag } => cfg_iter!(subdag)
                .map(|(_, certificates)| {
                    certificates.iter().flat_map(|certificate| certificate.id().to_bits_le()).collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
            Self::Compact { subdag } => cfg_iter!(subdag)
                .map(|(_, certificates)| {
                    certificates.iter().flat_map(|certificate| certificate.id().to_bits_le()).collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        };
        // Compute the subdag root.
        Ok(*N::merkle_tree_bhp::<SUBDAG_CERTIFICATES_DEPTH>(&leaves)?.root())
    }

    /// Returns the subdag with compact certificates
    pub fn into_compact(
        self,
        ratifications: Vec<N::RatificationID>,
        solutions: Option<&PuzzleSolutions<N>>,
        prior_solutions: Vec<SolutionID<N>>,
        transaction_ids: Vec<N::TransactionID>,
        prior_transaction_ids: Vec<N::TransactionID>,
        aborted_transaction_ids: Vec<N::TransactionID>,
    ) -> Result<Subdag<N>> {
        let Self::Full { subdag } = self else { bail!("Can only turn a Full subdag into a Compact one.") };

        // Initialize the new subdag.
        let mut compact_subdag = BTreeMap::new();

        // Iterate over the subdag.
        for (round, batch_certificates) in subdag.into_iter() {
            let certificates = cfg_into_iter!(batch_certificates)
                .map(|batch_certificate| {
                    let ratifications_iter = ratifications.iter();
                    let solutions_iter = solutions.map(|s| s.solution_ids());
                    let prior_solutions_iter = prior_solutions.iter();
                    let transactions_iter = transaction_ids.iter();
                    let prior_transactions_iter = prior_transaction_ids.iter();
                    let aborted_tx_iter = aborted_transaction_ids.iter();
                    CompactCertificate::from_batch_certificate(
                        batch_certificate,
                        ratifications_iter,
                        solutions_iter,
                        prior_solutions_iter,
                        transactions_iter,
                        prior_transactions_iter,
                        aborted_tx_iter,
                    )
                })
                .collect::<Result<_>>()?;
            compact_subdag.insert(round, certificates);
        }

        // Return the compact certificates.
        Ok(Self::Compact { subdag: compact_subdag })
    }

    /// Returns the subdag with full batch certificates.
    pub fn into_full(
        self,
        ratifications: Vec<N::RatificationID>,
        solutions: Option<PuzzleSolutions<N>>,
        prior_solutions: Vec<SolutionID<N>>,
        transaction_ids: Vec<N::TransactionID>,
        prior_transactions: Vec<N::TransactionID>,
        aborted_transaction_ids: Vec<N::TransactionID>,
    ) -> Result<Subdag<N>> {
        let Self::Compact { subdag } = self else { bail!("Can only turn a Compact subdag into a Full one.") };
        // Initialize the new subdag.
        let mut batch_subdag = BTreeMap::new();

        // Iterate over the subdag.
        for (round, compact_certificates) in subdag.into_iter() {
            let mut batch_certificates = IndexSet::with_capacity(compact_certificates.len());
            for certificate in compact_certificates.into_iter() {
                // Create iters.
                let ratifications_iter = ratifications.iter();
                let solutions_iter = solutions.as_ref().map(|s| s.solution_ids());
                let prior_solutions_iter = prior_solutions.iter();
                let transactions_iter = transaction_ids.iter();
                let prior_transactions_iter = prior_transactions.iter();
                let aborted_tx_iter = aborted_transaction_ids.iter();

                // Convert compact certificate to batch certificate.
                let batch_certificate = certificate.into_batch_certificate(
                    ratifications_iter,
                    solutions_iter,
                    prior_solutions_iter,
                    transactions_iter,
                    prior_transactions_iter,
                    aborted_tx_iter,
                )?;
                batch_certificates.insert(batch_certificate);
            }
            batch_subdag.insert(round, batch_certificates);
        }

        // Return the batch certificates.
        Ok(Self::Full { subdag: batch_subdag })
    }
}

#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers {
    use super::*;
    use console::{network::MainnetV0, prelude::TestRng};

    use indexmap::{IndexSet, indexset};

    type CurrentNetwork = MainnetV0;

    /// Returns a sample subdag, sampled at random.
    pub fn sample_subdag(rng: &mut TestRng) -> Subdag<CurrentNetwork> {
        const F: usize = 1;
        const AVAILABILITY_THRESHOLD: usize = F + 1;
        const QUORUM_THRESHOLD: usize = 2 * F + 1;

        // Initialize the map for the subdag.
        let mut subdag = BTreeMap::<u64, IndexSet<_>>::new();

        // Initialize the starting round.
        let starting_round = {
            loop {
                let round = rng.gen_range(2..u64::MAX);
                if round % 2 == 0 {
                    break round;
                }
            }
        };

        // Process the earliest round.
        let mut previous_certificate_ids = IndexSet::new();
        for _ in 0..AVAILABILITY_THRESHOLD {
            let certificate =
                snarkvm_ledger_narwhal_batch_certificate::test_helpers::sample_batch_certificate_for_round(
                    starting_round,
                    rng,
                );
            previous_certificate_ids.insert(certificate.id());
            subdag.entry(starting_round).or_default().insert(certificate);
        }

        // Process the middle round.
        let mut previous_certificate_ids_2 = IndexSet::new();
        for _ in 0..QUORUM_THRESHOLD {
            let certificate =
                snarkvm_ledger_narwhal_batch_certificate::test_helpers::sample_batch_certificate_for_round_with_previous_certificate_ids(starting_round + 1, previous_certificate_ids.clone(), rng);
            previous_certificate_ids_2.insert(certificate.id());
            subdag.entry(starting_round + 1).or_default().insert(certificate);
        }

        // Process the latest round.
        let certificate =
            snarkvm_ledger_narwhal_batch_certificate::test_helpers::sample_batch_certificate_for_round_with_previous_certificate_ids(
                starting_round + 2,
                previous_certificate_ids_2,
                rng,
            );
        subdag.insert(starting_round + 2, indexset![certificate]);

        // Return the subdag.
        Subdag::from_full(subdag).unwrap()
    }

    /// Returns a list of sample subdags, sampled at random.
    pub fn sample_subdags(rng: &mut TestRng) -> Vec<Subdag<CurrentNetwork>> {
        // Initialize a sample vector.
        let mut sample = Vec::with_capacity(10);
        // Append sample subdags.
        for _ in 0..10 {
            sample.push(sample_subdag(rng));
        }
        // Return the sample vector.
        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_ledger_narwhal_batch_header::BatchHeader;

    type CurrentNetwork = console::network::MainnetV0;

    const ITERATIONS: u64 = 100;

    #[test]
    fn test_max_certificates() {
        // Determine the maximum number of certificates in a block.
        let max_certificates_per_block =
            BatchHeader::<CurrentNetwork>::MAX_GC_ROUNDS * CurrentNetwork::LATEST_MAX_CERTIFICATES().unwrap() as usize;

        // Note: The maximum number of certificates in a block must be able to be Merklized.
        assert!(
            max_certificates_per_block <= 2u32.checked_pow(SUBDAG_CERTIFICATES_DEPTH as u32).unwrap() as usize,
            "The maximum number of certificates in a block is too large"
        );
    }

    #[test]
    fn test_weighted_median_simple() {
        // Test a simple case with equal weights.
        let data = vec![(1, 10), (2, 10), (3, 10)];
        assert_eq!(weighted_median(data), 2);

        // Test a case with a single element.
        let data = vec![(5, 10)];
        assert_eq!(weighted_median(data), 5);

        // Test a case with an even number of elements
        let data = vec![(1, 10), (2, 30), (3, 20), (4, 40)];
        assert_eq!(weighted_median(data), 3);

        // Test a case with a skewed weight.
        let data = vec![(100, 100), (200, 10000), (300, 500)];
        assert_eq!(weighted_median(data), 200);

        // Test a case with a empty set.
        assert_eq!(weighted_median(vec![]), 0);

        // Test a case where there is possible truncation.
        let data = vec![(1, 1), (2, 1), (3, 1), (4, 1), (5, 1)];
        assert_eq!(weighted_median(data), 3);

        // Test a case where weights of 0 do not affect the median.
        let data = vec![(1, 10), (2, 0), (3, 0), (4, 0), (5, 20), (6, 0), (7, 10)];
        assert_eq!(weighted_median(data), 5);
    }

    #[test]
    fn test_weighted_median_range() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let data: Vec<(i64, u64)> = (0..10).map(|_| (rng.gen_range(1..100), rng.gen_range(10..100))).collect();
            let min = data.iter().min_by_key(|x| x.0).unwrap().0;
            let max = data.iter().max_by_key(|x| x.0).unwrap().0;
            let median = weighted_median(data);
            assert!(median >= min && median <= max);
        }
    }

    #[test]
    fn test_weighted_median_scaled_weights() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let data: Vec<(i64, u64)> = (0..10).map(|_| (rng.gen_range(1..100), rng.gen_range(10..100) * 2)).collect();
            let scaled_data: Vec<(i64, u64)> = data.iter().map(|&(t, s)| (t, s * 10)).collect();

            if weighted_median(data.clone()) != weighted_median(scaled_data.clone()) {
                println!("data: {data:?}");
                println!("scaled_data: {scaled_data:?}");
            }
            assert_eq!(weighted_median(data), weighted_median(scaled_data));
        }
    }
}
