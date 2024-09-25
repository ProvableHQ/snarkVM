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

#![forbid(unsafe_code)]
#![warn(clippy::cast_possible_truncation)]

mod bytes;
mod serialize;
mod string;

use console::{
    account::{Address, Signature},
    prelude::*,
    types::Field,
};
use narwhal_batch_header::BatchHeader;
use narwhal_transmission_id::TransmissionID;

use core::hash::{Hash, Hasher};
use indexmap::IndexSet;
use std::collections::HashSet;

#[cfg(not(feature = "serial"))]
use rayon::prelude::*;

#[derive(Clone)]
pub struct BatchCertificate<N: Network> {
    /// The batch header.
    batch_header: BatchHeader<N>,
    /// The signatures for the batch ID from the committee.
    signatures: IndexSet<Signature<N>>,
}

impl<N: Network> BatchCertificate<N> {
    /// The maximum number of signatures in a batch certificate.
    pub const MAX_SIGNATURES: u16 = BatchHeader::<N>::MAX_CERTIFICATES;
}

impl<N: Network> BatchCertificate<N> {
    /// Initializes a new batch certificate.
    pub fn from(batch_header: BatchHeader<N>, signatures: IndexSet<Signature<N>>) -> Result<Self> {
        // Ensure that the number of signatures is within bounds.
        ensure!(signatures.len() <= Self::MAX_SIGNATURES as usize, "Invalid number of signatures");

        // Ensure that the signature is from a unique signer and not from the author.
        let signature_authors = signatures.iter().map(|signature| signature.to_address()).collect::<HashSet<_>>();
        ensure!(
            !signature_authors.contains(&batch_header.author()),
            "The author's signature was included in the signers"
        );
        ensure!(signature_authors.len() == signatures.len(), "A duplicate author was found in the set of signatures");

        // Verify the signatures are valid.
        cfg_iter!(signatures).try_for_each(|signature| {
            if !signature.verify(&signature.to_address(), &[batch_header.batch_id()]) {
                bail!("Invalid batch certificate signature")
            }
            Ok(())
        })?;
        // Return the batch certificate.
        Self::from_unchecked(batch_header, signatures)
    }

    /// Initializes a new batch certificate.
    pub fn from_unchecked(batch_header: BatchHeader<N>, signatures: IndexSet<Signature<N>>) -> Result<Self> {
        // Ensure the signatures are not empty.
        ensure!(!signatures.is_empty(), "Batch certificate must contain signatures");
        // Return the batch certificate.
        Ok(Self { batch_header, signatures })
    }
}

impl<N: Network> PartialEq for BatchCertificate<N> {
    fn eq(&self, other: &Self) -> bool {
        self.batch_id() == other.batch_id()
    }
}

impl<N: Network> Eq for BatchCertificate<N> {}

impl<N: Network> Hash for BatchCertificate<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.batch_header.batch_id().hash(state);
    }
}

impl<N: Network> BatchCertificate<N> {
    /// Returns the certificate ID.
    pub const fn id(&self) -> Field<N> {
        self.batch_header.batch_id()
    }

    /// Returns the batch header.
    pub const fn batch_header(&self) -> &BatchHeader<N> {
        &self.batch_header
    }

    /// Returns the batch ID.
    pub const fn batch_id(&self) -> Field<N> {
        self.batch_header().batch_id()
    }

    /// Returns the author.
    pub const fn author(&self) -> Address<N> {
        self.batch_header().author()
    }

    /// Returns the round.
    pub const fn round(&self) -> u64 {
        self.batch_header().round()
    }

    /// Returns the timestamp of the batch header.
    pub fn timestamp(&self) -> i64 {
        self.batch_header().timestamp()
    }

    /// Returns the committee ID.
    pub const fn committee_id(&self) -> Field<N> {
        self.batch_header().committee_id()
    }

    /// Returns the transmission IDs.
    pub const fn transmission_ids(&self) -> &IndexSet<TransmissionID<N>> {
        self.batch_header().transmission_ids()
    }

    /// Returns the batch certificate IDs for the previous round.
    pub const fn previous_certificate_ids(&self) -> &IndexSet<Field<N>> {
        self.batch_header().previous_certificate_ids()
    }

    /// Returns the signatures of the batch ID from the committee.
    pub fn signatures(&self) -> Box<dyn '_ + ExactSizeIterator<Item = &Signature<N>>> {
        Box::new(self.signatures.iter())
    }
}

#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers {
    use super::*;
    use console::{account::PrivateKey, network::MainnetV0, prelude::TestRng, types::Field};

    use indexmap::IndexSet;

    type CurrentNetwork = MainnetV0;

    /// Returns a sample batch certificate, sampled at random.
    pub fn sample_batch_certificate(rng: &mut TestRng) -> BatchCertificate<CurrentNetwork> {
        sample_batch_certificate_for_round(rng.gen(), rng)
    }

    /// Returns a sample batch certificate with a given round; the rest is sampled at random.
    pub fn sample_batch_certificate_for_round(round: u64, rng: &mut TestRng) -> BatchCertificate<CurrentNetwork> {
        // Sample certificate IDs.
        let certificate_ids = (0..10).map(|_| Field::<CurrentNetwork>::rand(rng)).collect::<IndexSet<_>>();
        // Return the batch certificate.
        sample_batch_certificate_for_round_with_previous_certificate_ids(round, certificate_ids, rng)
    }

    /// Returns a sample batch certificate with a given round; the rest is sampled at random.
    pub fn sample_batch_certificate_for_round_with_previous_certificate_ids(
        round: u64,
        previous_certificate_ids: IndexSet<Field<CurrentNetwork>>,
        rng: &mut TestRng,
    ) -> BatchCertificate<CurrentNetwork> {
        // Sample a batch header.
        let batch_header =
            narwhal_batch_header::test_helpers::sample_batch_header_for_round_with_previous_certificate_ids(
                round,
                previous_certificate_ids,
                rng,
            );
        // Sample a list of signatures.
        let mut signatures = IndexSet::with_capacity(5);
        for _ in 0..5 {
            let private_key = PrivateKey::new(rng).unwrap();
            signatures.insert(private_key.sign(&[batch_header.batch_id()], rng).unwrap());
        }
        // Return the batch certificate.
        BatchCertificate::from(batch_header, signatures).unwrap()
    }

    /// Returns a list of sample batch certificates, sampled at random.
    pub fn sample_batch_certificates(rng: &mut TestRng) -> IndexSet<BatchCertificate<CurrentNetwork>> {
        // Initialize a sample vector.
        let mut sample = IndexSet::with_capacity(10);
        // Append sample batch certificates.
        for _ in 0..10 {
            sample.insert(sample_batch_certificate(rng));
        }
        // Return the sample vector.
        sample
    }

    /// Returns a sample batch certificate with previous certificates, sampled at random.
    pub fn sample_batch_certificate_with_previous_certificates(
        round: u64,
        rng: &mut TestRng,
    ) -> (BatchCertificate<CurrentNetwork>, Vec<BatchCertificate<CurrentNetwork>>) {
        assert!(round > 1, "Round must be greater than 1");

        // Initialize the round parameters.
        let previous_round = round - 1; // <- This must be an even number, for `BFT::update_dag` to behave correctly below.
        let current_round = round;

        assert_eq!(previous_round % 2, 0, "Previous round must be even");

        // Sample the previous certificates.
        let previous_certificates = vec![
            sample_batch_certificate_for_round(previous_round, rng),
            sample_batch_certificate_for_round(previous_round, rng),
            sample_batch_certificate_for_round(previous_round, rng),
            sample_batch_certificate_for_round(previous_round, rng),
        ];
        // Construct the previous certificate IDs.
        let previous_certificate_ids: IndexSet<_> = previous_certificates.iter().map(|c| c.id()).collect();
        // Sample the leader certificate.
        let certificate = sample_batch_certificate_for_round_with_previous_certificate_ids(
            current_round,
            previous_certificate_ids,
            rng,
        );

        (certificate, previous_certificates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type CurrentNetwork = console::network::MainnetV0;

    #[test]
    fn test_maximum_signatures() {
        assert_eq!(BatchHeader::<CurrentNetwork>::MAX_CERTIFICATES, BatchCertificate::<CurrentNetwork>::MAX_SIGNATURES);
    }
}
