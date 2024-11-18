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

/// Ensures the given iterator has no duplicate elements, and that the ledger
/// does not already contain a given item.
macro_rules! ensure_is_unique {
    ($name:expr, $self:expr, $method:ident, $iter:expr) => {
        // Ensure there are no duplicate items in the transaction.
        if has_duplicates($iter) {
            bail!("Found a duplicate {} in the transaction", $name);
        }
        // Ensure the ledger does not already contain a given item.
        for item in $iter {
            if $self.transition_store().$method(item)? {
                bail!("The {} '{}' already exists in the ledger", $name, item)
            }
        }
    };
}

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// The maximum number of deployments to verify in parallel.
    pub(crate) const MAX_PARALLEL_DEPLOY_VERIFICATIONS: usize = 5;
    /// The maximum number of executions to verify in parallel.
    pub(crate) const MAX_PARALLEL_EXECUTE_VERIFICATIONS: usize = 1000;

    /// Verifies the list of transactions in the VM. On failure, returns an error.
    pub fn check_transactions<R: CryptoRng + Rng>(
        &self,
        transactions: &[(&Transaction<N>, Option<Field<N>>)],
        rng: &mut R,
    ) -> Result<()> {
        // Separate the transactions into deploys and executions.
        let (deployments, executions): (Vec<_>, Vec<_>) = transactions.iter().partition(|(tx, _)| tx.is_deploy());
        // Chunk the deploys and executions into groups for parallel verification.
        let deployments_for_verification = deployments.chunks(Self::MAX_PARALLEL_DEPLOY_VERIFICATIONS);
        let executions_for_verification = executions.chunks(Self::MAX_PARALLEL_EXECUTE_VERIFICATIONS);

        // Verify the transactions in batches.
        for transactions in deployments_for_verification.chain(executions_for_verification) {
            // Ensure each transaction is well-formed and unique.
            let rngs = (0..transactions.len()).map(|_| StdRng::from_seed(rng.gen())).collect::<Vec<_>>();
            cfg_iter!(transactions).zip(rngs).try_for_each(|((transaction, rejected_id), mut rng)| {
                self.check_transaction(transaction, *rejected_id, &mut rng)
                    .map_err(|e| anyhow!("Invalid transaction found in the transactions list: {e}"))
            })?;
        }

        Ok(())
    }
}

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Verifies the transaction in the VM. On failure, returns an error.
    #[inline]
    pub fn check_transaction<R: CryptoRng + Rng>(
        &self,
        transaction: &Transaction<N>,
        rejected_id: Option<Field<N>>,
        rng: &mut R,
    ) -> Result<()> {
        let timer = timer!("VM::check_transaction");

        /* Transaction */

        // Allocate a buffer to write the transaction.
        let mut buffer = Vec::with_capacity(N::MAX_TRANSACTION_SIZE);
        // Ensure that the transaction is well formed and does not exceed the maximum size.
        if let Err(error) = transaction.write_le(LimitedWriter::new(&mut buffer, N::MAX_TRANSACTION_SIZE)) {
            bail!("Transaction '{}' is not well-formed: {error}", transaction.id())
        }

        // Ensure the transaction ID is unique.
        if self.block_store().contains_transaction_id(&transaction.id())? {
            bail!("Transaction '{}' already exists in the ledger", transaction.id())
        }

        // Compute the Merkle root of the transaction.
        match transaction.to_root() {
            // Ensure the transaction ID is correct.
            Ok(root) if *transaction.id() != root => bail!("Incorrect transaction ID ({})", transaction.id()),
            Ok(_) => (),
            Err(error) => {
                bail!("Failed to compute the Merkle root of the transaction: {error}\n{transaction}");
            }
        };
        lap!(timer, "Verify the transaction ID");

        /* Transition */

        // Ensure the transition IDs are unique.
        ensure_is_unique!("transition ID", self, contains_transition_id, transaction.transition_ids());

        /* Input */

        // Ensure the input IDs are unique.
        ensure_is_unique!("input ID", self, contains_input_id, transaction.input_ids());
        // Ensure the serial numbers are unique.
        ensure_is_unique!("serial number", self, contains_serial_number, transaction.serial_numbers());
        // Ensure the tags are unique.
        ensure_is_unique!("tag", self, contains_tag, transaction.tags());

        /* Output */

        // Ensure the output IDs are unique.
        ensure_is_unique!("output ID", self, contains_output_id, transaction.output_ids());
        // Ensure the commitments are unique.
        ensure_is_unique!("commitment", self, contains_commitment, transaction.commitments());
        // Ensure the nonces are unique.
        ensure_is_unique!("nonce", self, contains_nonce, transaction.nonces());

        /* Metadata */

        // Ensure the transition public keys are unique.
        ensure_is_unique!("transition public key", self, contains_tpk, transaction.transition_public_keys());
        // Ensure the transition commitments are unique.
        ensure_is_unique!("transition commitment", self, contains_tcm, transaction.transition_commitments());

        lap!(timer, "Check for duplicate elements");

        // First, verify the fee.
        self.check_fee(transaction, rejected_id)?;

        // Construct the transaction checksum.
        let checksum = Data::<Transaction<N>>::Buffer(transaction.to_bytes_le()?.into()).to_checksum::<N>()?;

        // Check if the transaction exists in the partially-verified cache.
        let is_partially_verified =
            self.partially_verified_transactions.read().peek(&(transaction.id())) == Some(&checksum);

        // Next, verify the deployment or execution.
        match transaction {
            Transaction::Deploy(id, owner, deployment, _) => {
                // Compute the deployment ID.
                let Ok(deployment_id) = deployment.to_deployment_id() else {
                    bail!("Failed to compute the Merkle root for a deployment transaction '{id}'")
                };
                // Verify the signature corresponds to the transaction ID.
                ensure!(owner.verify(deployment_id), "Invalid owner signature for deployment transaction '{id}'");
                // Ensure the edition is correct.
                if deployment.edition() != N::EDITION {
                    bail!("Invalid deployment transaction '{id}' - expected edition {}", N::EDITION)
                }
                // Ensure the program ID does not already exist in the store.
                if self.transaction_store().contains_program_id(deployment.program_id())? {
                    bail!("Program ID '{}' is already deployed", deployment.program_id())
                }
                // Ensure the program does not already exist in the process.
                if self.contains_program(deployment.program_id()) {
                    bail!("Program ID '{}' already exists", deployment.program_id());
                }
                // Verify the deployment if it has not been verified before.
                if !is_partially_verified {
                    // Verify the deployment.
                    match try_vm_runtime!(|| self.check_deployment_internal(deployment, rng)) {
                        Ok(result) => result?,
                        Err(_) => bail!("VM safely halted transaction '{id}' during verification"),
                    }
                }
            }
            Transaction::Execute(id, execution, _) => {
                // Compute the execution ID.
                let Ok(execution_id) = execution.to_execution_id() else {
                    bail!("Failed to compute the Merkle root for an execution transaction '{id}'")
                };
                // Ensure the execution was not previously rejected (replay attack prevention).
                if self.block_store().contains_rejected_deployment_or_execution_id(&execution_id)? {
                    bail!("Transaction '{id}' contains a previously rejected execution")
                }
                // Verify the execution.
                match try_vm_runtime!(|| self.check_execution_internal(execution, is_partially_verified)) {
                    Ok(result) => result?,
                    Err(_) => bail!("VM safely halted transaction '{id}' during verification"),
                }
            }
            Transaction::Fee(..) => { /* no-op */ }
        }

        // If the above checks have passed and this is not a fee transaction,
        // then add the transaction ID to the partially-verified transactions cache.
        if !matches!(transaction, Transaction::Fee(..)) && !is_partially_verified {
            self.partially_verified_transactions.write().push(transaction.id(), checksum);
        }

        finish!(timer, "Verify the transaction");
        Ok(())
    }

    /// Verifies the `fee` in the given transaction. On failure, returns an error.
    #[inline]
    pub fn check_fee(&self, transaction: &Transaction<N>, rejected_id: Option<Field<N>>) -> Result<()> {
        match transaction {
            Transaction::Deploy(id, _, deployment, fee) => {
                // Ensure the rejected ID is not present.
                ensure!(rejected_id.is_none(), "Transaction '{id}' should not have a rejected ID (deployment)");
                // Compute the deployment ID.
                let Ok(deployment_id) = deployment.to_deployment_id() else {
                    bail!("Failed to compute the Merkle root for deployment transaction '{id}'")
                };
                // Compute the minimum deployment cost.
                let (cost, _) = deployment_cost(deployment)?;
                // Ensure the fee is sufficient to cover the cost.
                if *fee.base_amount()? < cost {
                    bail!("Transaction '{id}' has an insufficient base fee (deployment) - requires {cost} microcredits")
                }
                // Verify the fee.
                self.check_fee_internal(fee, deployment_id)?;
            }
            Transaction::Execute(id, execution, fee) => {
                // Ensure the rejected ID is not present.
                ensure!(rejected_id.is_none(), "Transaction '{id}' should not have a rejected ID (execution)");
                // Compute the execution ID.
                let Ok(execution_id) = execution.to_execution_id() else {
                    bail!("Failed to compute the Merkle root for execution transaction '{id}'")
                };
                // If the transaction contains only 1 transition, and the transition is a split, then the fee can be skipped.
                let is_fee_required = !(execution.len() == 1 && transaction.contains_split());
                // Verify the fee.
                if let Some(fee) = fee {
                    // If the fee is required, then check that the base fee amount is satisfied.
                    if is_fee_required {
                        // Compute the execution cost based on the block height
                        let block_height = self
                            .block_store()
                            .find_block_height_from_state_root(execution.global_state_root())?
                            .unwrap_or_default();
                        let (cost, (_, _)) = match block_height < N::CONSENSUS_V2_HEIGHT {
                            true => execution_cost_v1(&self.process().read(), execution)?,
                            false => execution_cost_v2(&self.process().read(), execution)?,
                        };
                        // Ensure the fee is sufficient to cover the cost.
                        if *fee.base_amount()? < cost {
                            bail!(
                                "Transaction '{id}' has an insufficient base fee (execution) - requires {cost} microcredits"
                            )
                        }
                    } else {
                        // Ensure the base fee amount is zero.
                        ensure!(*fee.base_amount()? == 0, "Transaction '{id}' has a non-zero base fee (execution)");
                    }
                    // Verify the fee.
                    self.check_fee_internal(fee, execution_id)?;
                } else {
                    // Ensure the fee can be safely skipped.
                    ensure!(!is_fee_required, "Transaction '{id}' is missing a fee (execution)");
                }
            }
            // Note: This transaction type does not need to check the fee amount, because:
            //  1. The fee is guaranteed to be non-zero by the constructor of `Transaction::Fee`.
            //  2. The fee may be less that the deployment or execution cost, as this is a valid reason it was rejected.
            Transaction::Fee(id, fee) => {
                // Verify the fee.
                match rejected_id {
                    Some(rejected_id) => self.check_fee_internal(fee, rejected_id)?,
                    None => bail!("Transaction '{id}' is missing a rejected ID (fee)"),
                }
            }
        }
        Ok(())
    }
}

impl<N: Network, C: ConsensusStorage<N>> VM<N, C> {
    /// Verifies the given deployment. On failure, returns an error.
    ///
    /// Note: This is an internal check only. To ensure all components of the deployment are checked,
    /// use `VM::check_transaction` instead.
    #[inline]
    fn check_deployment_internal<R: CryptoRng + Rng>(&self, deployment: &Deployment<N>, rng: &mut R) -> Result<()> {
        macro_rules! logic {
            ($process:expr, $network:path, $aleo:path) => {{
                // Prepare the deployment.
                let deployment = cast_ref!(&deployment as Deployment<$network>);
                // Verify the deployment.
                $process.verify_deployment::<$aleo, _>(&deployment, rng)
            }};
        }

        // Process the logic.
        let timer = timer!("VM::check_deployment");
        let result = process!(self, logic).map_err(|error| anyhow!("Deployment verification failed - {error}"));
        finish!(timer);
        result
    }

    /// Verifies the given execution. On failure, returns an error.
    ///
    /// Note: This is an internal check only. To ensure all components of the execution are checked,
    /// use `VM::check_transaction` instead.
    #[inline]
    fn check_execution_internal(&self, execution: &Execution<N>, is_partially_verified: bool) -> Result<()> {
        let timer = timer!("VM::check_execution");

        // Retrieve the block height.
        let block_height = self.block_store().current_block_height();

        // Ensure the execution does not contain any restricted transitions.
        if self.restrictions.contains_restricted_transitions(execution, block_height) {
            bail!("Execution verification failed - restricted transition found");
        }

        // Verify the execution proof, if it has not been partially-verified before.
        let verification = match is_partially_verified {
            true => Ok(()),
            false => self.process.read().verify_execution(execution),
        };
        lap!(timer, "Verify the execution");

        // Ensure the global state root exists in the block store.
        let result = match verification {
            // Ensure the global state root exists in the block store.
            Ok(()) => match self.block_store().contains_state_root(&execution.global_state_root()) {
                Ok(true) => Ok(()),
                Ok(false) => bail!("Execution verification failed - global state root does not exist (yet)"),
                Err(error) => bail!("Execution verification failed - {error}"),
            },
            Err(error) => bail!("Execution verification failed - {error}"),
        };
        finish!(timer, "Check the global state root");
        result
    }

    /// Verifies the given fee. On failure, returns an error.
    ///
    /// Note: This is an internal check only. To ensure all components of the fee are checked,
    /// use `VM::check_fee` instead.
    #[inline]
    fn check_fee_internal(&self, fee: &Fee<N>, deployment_or_execution_id: Field<N>) -> Result<()> {
        let timer = timer!("VM::check_fee");

        // Ensure the fee does not exceed the limit.
        let fee_amount = fee.amount()?;
        ensure!(*fee_amount <= N::MAX_FEE, "Fee verification failed: fee exceeds the maximum limit");

        // Verify the fee.
        let verification = self.process.read().verify_fee(fee, deployment_or_execution_id);
        lap!(timer, "Verify the fee");

        // TODO (howardwu): This check is technically insufficient. Consider moving this upstream
        //  to the speculation layer.
        // If the fee is public, speculatively check the account balance.
        if fee.is_fee_public() {
            // Retrieve the payer.
            let Some(payer) = fee.payer() else {
                bail!("Fee verification failed: fee is public, but the payer is missing");
            };
            // Retrieve the account balance of the payer.
            let Some(Value::Plaintext(Plaintext::Literal(Literal::U64(balance), _))) =
                self.finalize_store().get_value_speculative(
                    ProgramID::from_str("credits.aleo")?,
                    Identifier::from_str("account")?,
                    &Plaintext::from(Literal::Address(payer)),
                )?
            else {
                bail!("Fee verification failed: fee is public, but the payer account balance is missing");
            };
            // Ensure the balance is sufficient.
            ensure!(balance >= fee_amount, "Fee verification failed: insufficient balance");
        }

        // Ensure the global state root exists in the block store.
        let result = match verification {
            Ok(()) => match self.block_store().contains_state_root(&fee.global_state_root()) {
                Ok(true) => Ok(()),
                Ok(false) => bail!("Fee verification failed: global state root not found"),
                Err(error) => bail!("Fee verification failed: {error}"),
            },
            Err(error) => bail!("Fee verification failed: {error}"),
        };
        finish!(timer, "Check the global state root");
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::vm::test_helpers::sample_finalize_state;
    use console::{
        account::{Address, ViewKey},
        types::Field,
    };
    use ledger_block::{Block, Header, Metadata, Transaction, Transition};

    type CurrentNetwork = test_helpers::CurrentNetwork;

    #[test]
    fn test_verify() {
        let rng = &mut TestRng::default();
        let vm = crate::vm::test_helpers::sample_vm_with_genesis_block(rng);

        // Fetch a deployment transaction.
        let deployment_transaction = crate::vm::test_helpers::sample_deployment_transaction(rng);
        // Ensure the transaction verifies.
        vm.check_transaction(&deployment_transaction, None, rng).unwrap();

        // Fetch an execution transaction.
        let execution_transaction = crate::vm::test_helpers::sample_execution_transaction_with_private_fee(rng);
        // Ensure the transaction verifies.
        vm.check_transaction(&execution_transaction, None, rng).unwrap();

        // Fetch an execution transaction.
        let execution_transaction = crate::vm::test_helpers::sample_execution_transaction_with_public_fee(rng);
        // Ensure the transaction verifies.
        vm.check_transaction(&execution_transaction, None, rng).unwrap();
    }

    #[test]
    fn test_verify_deployment() {
        let rng = &mut TestRng::default();
        let vm = crate::vm::test_helpers::sample_vm();

        // Fetch the program from the deployment.
        let program = crate::vm::test_helpers::sample_program();

        // Deploy the program.
        let deployment = vm.deploy_raw(&program, rng).unwrap();

        // Ensure the deployment is valid.
        vm.check_deployment_internal(&deployment, rng).unwrap();

        // Ensure that deserialization doesn't break the transaction verification.
        let serialized_deployment = deployment.to_string();
        let deployment_transaction: Deployment<CurrentNetwork> = serde_json::from_str(&serialized_deployment).unwrap();
        vm.check_deployment_internal(&deployment_transaction, rng).unwrap();
    }

    #[test]
    fn test_verify_execution() {
        let rng = &mut TestRng::default();
        let vm = crate::vm::test_helpers::sample_vm_with_genesis_block(rng);

        // Fetch execution transactions.
        let transactions = [
            crate::vm::test_helpers::sample_execution_transaction_with_private_fee(rng),
            crate::vm::test_helpers::sample_execution_transaction_with_public_fee(rng),
        ];

        for transaction in transactions {
            match transaction {
                Transaction::Execute(_, execution, _) => {
                    // Ensure the proof exists.
                    assert!(execution.proof().is_some());
                    // Verify the execution.
                    vm.check_execution_internal(&execution, false).unwrap();

                    // Ensure that deserialization doesn't break the transaction verification.
                    let serialized_execution = execution.to_string();
                    let recovered_execution: Execution<CurrentNetwork> =
                        serde_json::from_str(&serialized_execution).unwrap();
                    vm.check_execution_internal(&recovered_execution, false).unwrap();
                }
                _ => panic!("Expected an execution transaction"),
            }
        }
    }

    #[test]
    fn test_verify_fee() {
        let rng = &mut TestRng::default();
        let vm = crate::vm::test_helpers::sample_vm_with_genesis_block(rng);

        // Fetch execution transactions.
        let transactions = [
            crate::vm::test_helpers::sample_execution_transaction_with_private_fee(rng),
            crate::vm::test_helpers::sample_execution_transaction_with_public_fee(rng),
        ];

        for transaction in transactions {
            match transaction {
                Transaction::Execute(_, execution, Some(fee)) => {
                    let execution_id = execution.to_execution_id().unwrap();

                    // Ensure the proof exists.
                    assert!(fee.proof().is_some());
                    // Verify the fee.
                    vm.check_fee_internal(&fee, execution_id).unwrap();

                    // Ensure that deserialization doesn't break the transaction verification.
                    let serialized_fee = fee.to_string();
                    let recovered_fee: Fee<CurrentNetwork> = serde_json::from_str(&serialized_fee).unwrap();
                    vm.check_fee_internal(&recovered_fee, execution_id).unwrap();
                }
                _ => panic!("Expected an execution with a fee"),
            }
        }
    }

    #[test]
    fn test_check_transaction_execution() {
        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch a valid execution transaction with a private fee.
        let valid_transaction = crate::vm::test_helpers::sample_execution_transaction_with_private_fee(rng);
        vm.check_transaction(&valid_transaction, None, rng).unwrap();

        // Fetch a valid execution transaction with a public fee.
        let valid_transaction = crate::vm::test_helpers::sample_execution_transaction_with_public_fee(rng);
        vm.check_transaction(&valid_transaction, None, rng).unwrap();

        // Fetch an valid execution transaction with no fee.
        let valid_transaction = crate::vm::test_helpers::sample_execution_transaction_without_fee(rng);
        vm.check_transaction(&valid_transaction, None, rng).unwrap();
    }

    #[test]
    fn test_verify_deploy_and_execute() {
        // Initialize the RNG.
        let rng = &mut TestRng::default();

        // Initialize a new caller.
        let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        let caller_view_key = ViewKey::try_from(&caller_private_key).unwrap();
        let address = Address::try_from(&caller_private_key).unwrap();

        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);

        // Fetch the unspent records.
        let records = genesis.records().collect::<indexmap::IndexMap<_, _>>();

        // Prepare the fee.
        let credits = records.values().next().unwrap().decrypt(&caller_view_key).unwrap();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Deploy.
        let program = crate::vm::test_helpers::sample_program();
        let deployment_transaction = vm.deploy(&caller_private_key, &program, Some(credits), 10, None, rng).unwrap();

        // Construct the new block header.
        let time_since_last_block = CurrentNetwork::BLOCK_TIME as i64;
        let (ratifications, transactions, aborted_transaction_ids, ratified_finalize_operations) = vm
            .speculate(
                sample_finalize_state(1),
                time_since_last_block,
                Some(0u64),
                vec![],
                &None.into(),
                [deployment_transaction].iter(),
                rng,
            )
            .unwrap();
        assert!(aborted_transaction_ids.is_empty());

        // Construct the metadata associated with the block.
        let deployment_metadata = Metadata::new(
            CurrentNetwork::ID,
            1,
            1,
            0,
            0,
            CurrentNetwork::GENESIS_COINBASE_TARGET,
            CurrentNetwork::GENESIS_PROOF_TARGET,
            genesis.last_coinbase_target(),
            genesis.last_coinbase_timestamp(),
            genesis.timestamp().saturating_add(time_since_last_block),
        )
        .unwrap();

        let deployment_header = Header::from(
            vm.block_store().current_state_root(),
            transactions.to_transactions_root().unwrap(),
            transactions.to_finalize_root(ratified_finalize_operations).unwrap(),
            ratifications.to_ratifications_root().unwrap(),
            Field::zero(),
            Field::zero(),
            deployment_metadata,
        )
        .unwrap();

        // Construct a new block for the deploy transaction.
        let deployment_block = Block::new_beacon(
            &caller_private_key,
            genesis.hash(),
            deployment_header,
            ratifications,
            None.into(),
            vec![],
            transactions,
            aborted_transaction_ids,
            rng,
        )
        .unwrap();

        // Add the deployment block.
        vm.add_next_block(&deployment_block).unwrap();

        // Fetch the unspent records.
        let records = deployment_block.records().collect::<indexmap::IndexMap<_, _>>();

        // Prepare the inputs.
        let inputs = [
            Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
            Value::<CurrentNetwork>::from_str("10u64").unwrap(),
        ]
        .into_iter();

        // Prepare the fee.
        let credits = Some(records.values().next().unwrap().decrypt(&caller_view_key).unwrap());

        // Execute.
        let transaction =
            vm.execute(&caller_private_key, ("testing.aleo", "initialize"), inputs, credits, 10, None, rng).unwrap();

        // Verify.
        vm.check_transaction(&transaction, None, rng).unwrap();
    }

    #[test]
    fn test_failed_credits_deployment() {
        let rng = &mut TestRng::default();
        let vm = crate::vm::test_helpers::sample_vm();

        // Fetch the credits program
        let program = Program::credits().unwrap();

        // Ensure that the program can't be deployed.
        assert!(vm.deploy_raw(&program, rng).is_err());

        // Create a new `credits.aleo` program.
        let program = Program::from_str(
            r"
program credits.aleo;

record token:
    owner as address.private;
    amount as u64.private;

function compute:
    input r0 as u32.private;
    add r0 r0 into r1;
    output r1 as u32.public;",
        )
        .unwrap();

        // Ensure that the program can't be deployed.
        assert!(vm.deploy_raw(&program, rng).is_err());
    }

    #[test]
    fn test_check_mutated_execution() {
        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Fetch the caller's private key.
        let caller_private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch a valid execution transaction with a public fee.
        let valid_transaction = crate::vm::test_helpers::sample_execution_transaction_with_public_fee(rng);
        vm.check_transaction(&valid_transaction, None, rng).unwrap();

        // Mutate the execution transaction by inserting a Field::Zero as an output.
        let execution = valid_transaction.execution().unwrap();

        // Extract the first transition from the execution.
        let transitions: Vec<_> = execution.transitions().collect();
        assert_eq!(transitions.len(), 1);
        let transition = transitions[0].clone();

        // Mutate the transition by adding an additional `Field::zero` output. This is significant because the Varuna
        // verifier pads the inputs with `Field::zero`s, which means that the same proof is valid for both the
        // original and the mutated executions.
        let added_output = Output::ExternalRecord(Field::zero());
        let mutated_outputs = [transition.outputs(), &[added_output]].concat();
        let mutated_transition = Transition::new(
            *transition.program_id(),
            *transition.function_name(),
            transition.inputs().to_vec(),
            mutated_outputs,
            *transition.tpk(),
            *transition.tcm(),
            *transition.scm(),
        )
        .unwrap();

        // Construct the mutated execution.
        let mutated_execution = Execution::from(
            [mutated_transition].into_iter(),
            execution.global_state_root(),
            execution.proof().cloned(),
        )
        .unwrap();

        // Authorize the fee.
        let authorization = vm
            .authorize_fee_public(
                &caller_private_key,
                10_000_000,
                100,
                mutated_execution.to_execution_id().unwrap(),
                rng,
            )
            .unwrap();
        // Compute the fee.
        let fee = vm.execute_fee_authorization(authorization, None, rng).unwrap();

        // Construct the transaction.
        let mutated_transaction = Transaction::from_execution(mutated_execution, Some(fee)).unwrap();

        // Ensure that the mutated transaction fails verification due to an extra output.
        assert!(vm.check_transaction(&mutated_transaction, None, rng).is_err());
    }

    #[cfg(feature = "test")]
    #[test]
    fn test_fee_migration() {
        // This test will fail if the consensus v2 height is 0
        assert_ne!(0, CurrentNetwork::CONSENSUS_V2_HEIGHT);

        let minimum_credits_transfer_public_fee = 34_060;
        let old_minimum_credits_transfer_public_fee = 51_060;

        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Create the base transaction
        let transaction = crate::vm::test_helpers::sample_execution_transaction_with_public_fee(rng);

        // Try to submit a tx with the new fee before the migration block height
        let fee_too_low_transaction = crate::vm::test_helpers::create_new_transaction_with_different_fee(
            rng,
            transaction.clone(),
            minimum_credits_transfer_public_fee,
        );
        assert!(vm.check_transaction(&fee_too_low_transaction, None, rng).is_err());

        // Try to submit a tx with the old fee before the migration block height
        let old_valid_transaction = crate::vm::test_helpers::create_new_transaction_with_different_fee(
            rng,
            transaction.clone(),
            old_minimum_credits_transfer_public_fee,
        );
        assert!(vm.check_transaction(&old_valid_transaction, None, rng).is_ok());

        // Update the VM to the migration block height
        let private_key = test_helpers::sample_genesis_private_key(rng);
        let transactions: [Transaction<CurrentNetwork>; 0] = [];
        for _ in 0..CurrentNetwork::CONSENSUS_V2_HEIGHT {
            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // Create a new transaction with the new stateroot post migration block height
        let transaction = {
            let address = Address::try_from(&private_key).unwrap();
            let inputs = [
                Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
            ]
            .into_iter();

            // Execute.
            let transaction_without_fee =
                vm.execute(&private_key, ("credits.aleo", "transfer_public"), inputs, None, 0, None, rng).unwrap();
            let execution = transaction_without_fee.execution().unwrap().clone();

            // Authorize the fee.
            let authorization = vm
                .authorize_fee_public(&private_key, 10_000_000, 100, execution.to_execution_id().unwrap(), rng)
                .unwrap();
            // Compute the fee.
            let fee = vm.execute_fee_authorization(authorization, None, rng).unwrap();

            // Construct the transaction.
            Transaction::from_execution(execution, Some(fee)).unwrap()
        };

        // Try to submit a tx with the old fee after the migration block height
        // Should work as now the fee is just too high
        let fee_too_high_transaction = crate::vm::test_helpers::create_new_transaction_with_different_fee(
            rng,
            transaction.clone(),
            old_minimum_credits_transfer_public_fee,
        );
        assert!(vm.check_transaction(&fee_too_high_transaction, None, rng).is_ok());

        // Try to submit a tx with the new fee after the migration block height
        let valid_transaction = crate::vm::test_helpers::create_new_transaction_with_different_fee(
            rng,
            transaction.clone(),
            minimum_credits_transfer_public_fee,
        );
        assert!(vm.check_transaction(&valid_transaction, None, rng).is_ok());
    }
}
