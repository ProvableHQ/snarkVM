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

#![allow(clippy::type_complexity)]

use console::network::Network;
use indexmap::IndexSet;
use snarkvm_ledger_block::{Ratify, Transaction};
use snarkvm_ledger_narwhal::{Transmission, TransmissionID};
use snarkvm_ledger_puzzle::Solution;

use anyhow::{Result, bail, ensure};
use std::collections::HashSet;

/// Takes in an iterator of transmissions and returns a tuple of ratifications, solutions, and transactions.
///
/// This method ensures each transmission ID corresponds to its given transmission.
/// This method guarantees that the output is 1) order-preserving, and 2) unique.
pub fn decouple_transmissions<N: Network>(
    transmissions: impl Iterator<Item = (TransmissionID<N>, Transmission<N>)>,
) -> Result<(
    Vec<Ratify<N>>,
    Vec<Solution<N>>,
    Vec<(TransmissionID<N>, Solution<N>)>,
    Vec<Transaction<N>>,
    Vec<(TransmissionID<N>, Transaction<N>)>,
)> {
    // Initialize a list for the objects to return.
    let ratifications = Vec::new();
    let mut solutions = Vec::new();
    let mut solutions_with_id = Vec::new();
    let mut transactions = Vec::new();
    let mut transactions_with_id = Vec::new();

    // Initialize a set to ensure the transmissions are unique.
    let mut unique = HashSet::new();

    // Iterate over the transmissions.
    for (transmission_id, transmission) in transmissions {
        // Ensure the transmission ID is unique.
        ensure!(unique.insert(transmission_id), "Found a duplicate transmission ID - {transmission_id}");
        // Deserialize and store the transmission.
        match (transmission_id, transmission) {
            (TransmissionID::Ratification, Transmission::Ratification) => (),
            (id @ TransmissionID::Solution(commitment, checksum), Transmission::Solution(solution)) => {
                // Ensure the transmission checksum corresponds to the solution.
                ensure!(checksum == solution.to_checksum::<N>()?, "Mismatching transmission checksum (solution)");
                // Deserialize the solution.
                let solution = solution.deserialize_blocking()?;
                // Ensure the transmission ID corresponds to the solution.
                ensure!(commitment == solution.id(), "Mismatching transmission ID (solution)");
                // Insert the solution into the lists.
                solutions.push(solution);
                solutions_with_id.push((id, solution));
            }
            (id @ TransmissionID::Transaction(transaction_id, checksum), Transmission::Transaction(transaction)) => {
                // Ensure the transmission checksum corresponds to the transaction.
                ensure!(checksum == transaction.to_checksum::<N>()?, "Mismatching transmission checksum (transaction)");
                // Deserialize the transaction.
                let transaction = transaction.deserialize_blocking()?;
                // Ensure the transmission ID corresponds to the transaction.
                ensure!(transaction_id == transaction.id(), "Mismatching transmission ID (transaction)");
                // Insert the transaction into the lists.
                transactions.push(transaction.clone());
                transactions_with_id.push((id, transaction));
            }
            _ => bail!("Mismatching (transmission ID, transmission) entry"),
        }
    }
    // Return the ratifications, solutions, and transactions.
    Ok((ratifications, solutions, solutions_with_id, transactions, transactions_with_id))
}

/// Takes in an iterator of transmission IDs and returns a tuple of ratifications, solutions, and transactions.
///
/// This method guarantees that the output is 1) order-preserving, and 2) unique.
pub fn decouple_transmission_ids<N: Network>(
    transmission_ids: IndexSet<TransmissionID<N>>,
) -> Result<(Vec<TransmissionID<N>>, Vec<TransmissionID<N>>, Vec<TransmissionID<N>>)> {
    // Initialize a list for the ratifications.
    let ratifications = Vec::new();
    // Initialize a list for the solutions.
    let mut solution_ids = Vec::new();
    // Initialize a list for the transactions.
    let mut transaction_ids = Vec::new();

    // Iterate over the transmissions.
    for transmission_id in transmission_ids.into_iter() {
        // Deserialize and store the transmission.
        match transmission_id {
            TransmissionID::Ratification => (),
            TransmissionID::Solution(..) => {
                // Insert the solution into the list.
                solution_ids.push(transmission_id);
            }
            TransmissionID::Transaction(..) => {
                // Insert the transaction into the list.
                transaction_ids.push(transmission_id);
            }
        }
    }
    // Return the ratifications, solution_ids, and transaction_ids.
    Ok((ratifications, solution_ids, transaction_ids))
}
