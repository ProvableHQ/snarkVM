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

#![allow(clippy::type_complexity)]

use console::network::Network;
use ledger_block::{Ratify, Transaction};
use ledger_narwhal::{Transmission, TransmissionID};
use ledger_puzzle::Solution;

use anyhow::{bail, ensure, Result};
use std::collections::HashSet;

/// Takes in an iterator of transmissions and returns a tuple of ratifications, solutions, and transactions.
///
/// This method ensures each transmission ID corresponds to its given transmission.
/// This method guarantees that the output is 1) order-preserving, and 2) unique.
pub fn decouple_transmissions<N: Network>(
    transmissions: impl Iterator<Item = (TransmissionID<N>, Transmission<N>)>,
) -> Result<(Vec<Ratify<N>>, Vec<Solution<N>>, Vec<Transaction<N>>)> {
    // Initialize a list for the ratifications.
    let ratifications = Vec::new();
    // Initialize a list for the solutions.
    let mut solutions = Vec::new();
    // Initialize a list for the transactions.
    let mut transactions = Vec::new();

    // Initialize a set to ensure the transmissions are unique.
    let mut unique = HashSet::new();

    // Iterate over the transmissions.
    for (transmission_id, transmission) in transmissions {
        // Ensure the transmission ID is unique.
        ensure!(unique.insert(transmission_id), "Found a duplicate transmission ID - {transmission_id}");
        // Deserialize and store the transmission.
        match (transmission_id, transmission) {
            (TransmissionID::Ratification, Transmission::Ratification) => (),
            (TransmissionID::Solution(commitment, checksum), Transmission::Solution(solution)) => {
                // Ensure the transmission checksum corresponds to the solution.
                ensure!(checksum == solution.to_checksum::<N>()?, "Mismatching transmission checksum (solution)");
                // Deserialize the solution.
                let solution = solution.deserialize_blocking()?;
                // Ensure the transmission ID corresponds to the solution.
                ensure!(commitment == solution.id(), "Mismatching transmission ID (solution)");
                // Insert the solution into the list.
                solutions.push(solution);
            }
            (TransmissionID::Transaction(transaction_id, checksum), Transmission::Transaction(transaction)) => {
                // Ensure the transmission checksum corresponds to the transaction.
                ensure!(checksum == transaction.to_checksum::<N>()?, "Mismatching transmission checksum (transaction)");
                // Deserialize the transaction.
                let transaction = transaction.deserialize_blocking()?;
                // Ensure the transmission ID corresponds to the transaction.
                ensure!(transaction_id == transaction.id(), "Mismatching transmission ID (transaction)");
                // Insert the transaction into the list.
                transactions.push(transaction);
            }
            _ => bail!("Mismatching (transmission ID, transmission) entry"),
        }
    }
    // Return the ratifications, solutions, and transactions.
    Ok((ratifications, solutions, transactions))
}
