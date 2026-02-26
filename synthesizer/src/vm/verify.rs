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
    /// The maximum number of deployments that the VM can verify in parallel.
    pub const MAX_PARALLEL_DEPLOY_VERIFICATIONS: usize = 5;
    /// The maximum number of executions to verify in parallel.
    pub const MAX_PARALLEL_EXECUTE_VERIFICATIONS: usize = 1000;

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
            let rngs = (0..transactions.len()).map(|_| StdRng::from_seed(rng.r#gen())).collect::<Vec<_>>();
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

        // Get the current block height for consensus version checks.
        let current_block_height = self.block_store().current_block_height();

        /* Transaction */

        // Get the maximum transaction size for the current consensus version.
        let max_transaction_size = consensus_config_value!(N, MAX_TRANSACTION_SIZE, current_block_height)
            .ok_or(anyhow!("Failed to fetch maximum transaction size"))?;
        // Allocate a buffer to write the transaction.
        let mut buffer = Vec::with_capacity(max_transaction_size);
        // Ensure that the transaction is well formed and does not exceed the maximum size.
        if let Err(error) = transaction.write_le(LimitedWriter::new(&mut buffer, max_transaction_size)) {
            bail!("Transaction '{}' is not well-formed: {error}", transaction.id())
        }

        // Ensure the transaction ID is unique.
        if self.block_store().contains_transaction_id(&transaction.id())? {
            bail!("Transaction '{}' already exists in the ledger", transaction.id())
        }

        // Compute the Merkle root of the transaction.
        // Debug-mode only, as the `Transaction` constructor recomputes the transaction ID at initialization.
        #[cfg(debug_assertions)]
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

        // Get the consensus version.
        let consensus_version = N::CONSENSUS_VERSION(current_block_height)?;

        // Construct the transaction checksum.
        let checksum = Data::<Transaction<N>>::Buffer(transaction.to_bytes_le()?.into()).to_checksum::<N>()?;

        // Get the program editions from the transaction.

        let mut program_editions = Vec::with_capacity(Transaction::<N>::MAX_TRANSITIONS);
        for transition in transaction.transitions() {
            // Get the stack.
            let stack = self.process.read().get_stack(transition.program_id())?;
            // Get the program ID.
            let program_id = *stack.program_id();
            // Get the program edition.
            let edition = stack.program_edition();

            // If the consensus version is V8 or greater and any of the component programs (except for `credits.aleo`)
            //   - have edition 0
            //   - and the program does not have a constructor.
            // then fail.
            if consensus_version >= ConsensusVersion::V8
                && program_id != ProgramID::from_str("credits.aleo")?
                && edition.is_zero()
                && !stack.program().contains_constructor()
            {
                bail!(
                    "Invalid transaction '{}' - the program edition for '{program_id}' cannot be zero for `ConsensusVersion::V8` or greater. Please redeploy the program.",
                    transaction.id()
                );
            }
            // Add the program editions.
            program_editions.push(edition);
        }

        // Prepare the cache key.
        let cache_key = (transaction.id(), program_editions);

        // Check if the transaction exists in the partially-verified cache.
        let is_partially_verified = self.partially_verified_transactions.read().peek(&cache_key) == Some(&checksum);

        // Verify the fee.
        self.check_fee(transaction, rejected_id, is_partially_verified)?;

        // Next, verify the deployment or execution.
        match transaction {
            Transaction::Deploy(id, deployment_id, owner, deployment, _) => {
                // Sanity check that the program is not `credits.aleo`.
                ensure!(
                    deployment.program_id() != &ProgramID::from_str("credits.aleo")?,
                    "Cannot deploy 'credits.aleo'"
                );
                // Verify the signature corresponds to the transaction ID.
                ensure!(owner.verify(*deployment_id), "Invalid owner signature for deployment transaction '{id}'");

                // If the `CONSENSUS_VERSION` is less than `V8`, ensure that
                //   - the deployment edition is zero.
                // If the `CONSENSUS_VERSION` is less than `V9` ensure that
                //   - the deployment edition is zero or one.
                //   - the program checksum is **not** present in the deployment,
                //   - the program owner is **not** present in the deployment
                //   - the program does not use constructors, `Operand::Checksum`, `Operand::Edition`, or `Operand::ProgramOwner`
                // If the `CONSENSUS_VERSION` is greater than or equal to `V9`, then verify that:
                //   - the program checksum is present in the deployment
                //   - the program owner is present in the deployment
                // If the `CONSENSUS_VERSION` is less than `V11`, ensure that
                //   - the program does not include V11 syntax
                // If the `CONSENSUS_VERSION` is less than `V12`, ensure that
                //   - the program does not include V12 syntax
                // If the `CONSENSUS_VERSION` is less than `V13`, ensure that
                //   - the program does not include V13 syntax
                //   - the program does not use the external struct syntax `some_program.aleo/Struct`
                // If the `CONSENSUS_VERSION` is greater than or equal to `V13`, then verify that:
                //   - the program's mappings do not use non-existent structs.
                // If the `CONSENSUS_VERSION` is less than `V14`, ensure that
                //   - the program does not include V14 syntax
                //   - the argument bit size of futures does not exceed the maximum allowed size of u16::MAX.
                if consensus_version < ConsensusVersion::V8 {
                    ensure!(
                        deployment.edition().is_zero(),
                        "Invalid deployment transaction '{id}' - edition should be zero before `ConsensusVersion::V8`",
                    );
                }
                if consensus_version < ConsensusVersion::V9 {
                    ensure!(
                        deployment.edition() <= 1,
                        "Invalid deployment transaction '{id}' - edition should be zero or one for before `ConsensusVersion::V9`"
                    );
                    ensure!(
                        deployment.program_checksum().is_none(),
                        "Invalid deployment transaction '{id}' - should not contain program checksum"
                    );
                    ensure!(
                        deployment.program_owner().is_none(),
                        "Invalid deployment transaction '{id}' - should not contain program owner"
                    );
                    ensure!(
                        !deployment.program().contains_v9_syntax(),
                        "Invalid deployment transaction '{id}' - program uses syntax that is not allowed before `ConsensusVersion::V9`"
                    );
                }
                if consensus_version >= ConsensusVersion::V9 {
                    ensure!(
                        deployment.program_checksum().is_some(),
                        "Invalid deployment transaction '{id}' - missing program checksum"
                    );
                    ensure!(
                        deployment.program_owner().is_some(),
                        "Invalid deployment transaction '{id}' - missing program owner"
                    );
                }
                if consensus_version < ConsensusVersion::V11 {
                    ensure!(
                        !deployment.program().contains_v11_syntax(),
                        "Invalid deployment transaction '{id}' - program uses syntax that is not allowed before `ConsensusVersion::V11`"
                    );
                }
                if consensus_version < ConsensusVersion::V12 {
                    ensure!(
                        !deployment.program().contains_v12_syntax(),
                        "Invalid deployment transaction '{id}' - program uses syntax that is not allowed before `ConsensusVersion::V12`"
                    );
                }
                if consensus_version >= ConsensusVersion::V12 {
                    ensure!(
                        !deployment.program().contains_string_type(),
                        "Invalid deployment transaction '{id}' - program uses string type after `ConsensusVersion::V12`"
                    );
                }
                if consensus_version < ConsensusVersion::V13 {
                    ensure!(
                        !deployment.program().contains_external_struct(),
                        "Invalid deployment transaction '{id}' - external structs may only be used beginning with `ConsensusVersion::V13`"
                    );
                }
                if consensus_version >= ConsensusVersion::V13 {
                    self.process.read().mapping_types_exist(deployment.program())?;
                }
                if consensus_version < ConsensusVersion::V14 {
                    ensure!(
                        !deployment.program().contains_v14_syntax(),
                        "Invalid deployment transaction '{id}' - program uses syntax that is not allowed before `ConsensusVersion::V14`"
                    );
                    // Check that all future argument bit sizes do not exceed the maximum allowed size of u16::MAX.
                    let stack = Stack::new(&self.process().read(), deployment.program())?;
                    check_future_argument_bit_size(deployment.program(), &stack, u16::MAX as usize)?;
                }

                // Determine if any of the array types exceed the maximum array elements.
                // Do not perform this check if the consensus version is beyond the latest version threshold for `MAX_ARRAY_ELEMENTS`.
                if let Some((latest_version_threshold, _)) = N::MAX_ARRAY_ELEMENTS.last()
                    && consensus_version < *latest_version_threshold
                {
                    let max_array_elements = consensus_config_value!(N, MAX_ARRAY_ELEMENTS, current_block_height)
                        .ok_or_else(|| anyhow!("Missing consensus config value: MAX_ARRAY_ELEMENTS"))?;
                    ensure!(
                        !deployment.program().exceeds_max_array_size(u32::try_from(max_array_elements)?),
                        "Invalid deployment transaction '{id}' - program contains an array that exceeds the maximum allowed size of {max_array_elements} elements",
                    );
                }

                // Ensure the program size is bounded properly based on the current block height.
                deployment.program().check_program_size(current_block_height)?;

                // Ensure the program writes do not exceed the maximum allowed based on the current block height.
                deployment.program().check_program_writes(current_block_height)?;

                // If the program owner exists in the deployment, then verify that it matches the owner in the transaction.
                if let Some(given_owner) = deployment.program_owner() {
                    // Ensure the program owner matches the owner in the transaction.
                    ensure!(
                        given_owner == owner.address(),
                        "The program owner in the deployment did not match the owner in the transaction\n('[{}]' != '[{}]')",
                        given_owner,
                        owner.address()
                    );
                }

                // If the edition is zero, then check that:
                //  - The program does not exist in the store or process.
                //  - The program contains a constructor.
                // Otherwise, check that:
                //  - The program exists in the store and process.
                //  - The new edition increments the old edition by 1.
                //  - If the new program does not contain a constructor
                //      - the existing program does not have a constructor
                //      - the new program exactly matches the existing program
                //      - the edition is exactly one.
                //  - Otherwise, if the new program contains a constructor.
                //      - the existing program has a constructor.
                //      - if the consensus version is V10 or greater, then check that each function's **record** output registers match the existing program.
                //      - Note. Constructor validity is checked at a later point.
                //      - Note. The remaining syntactic checks on upgrades are done in `Stack::check_upgrade_is_valid`.
                let is_program_in_storage = self.transaction_store().contains_program_id(deployment.program_id())?;
                let is_program_in_process = self.contains_program(deployment.program_id());
                match deployment.edition() {
                    0 => {
                        // Ensure the program ID does not already exist in the store.
                        ensure!(!is_program_in_storage, "Program ID '{}' is already deployed", deployment.program_id());
                        // Ensure the program does not already exist in the process.
                        ensure!(!is_program_in_process, "Program ID '{}' already exists", deployment.program_id());
                        // Ensure that the program contains a constructor if the program is deployed after `ConsensusVersion::V9`.
                        if consensus_version >= ConsensusVersion::V9 {
                            // Check that the program contains a constructor.
                            ensure!(
                                deployment.program().contains_constructor(),
                                "Invalid deployment transaction '{id}' - a new program after `ConsensusVersion::V9` must contain a constructor"
                            );
                        }
                    }
                    new_edition => {
                        // Check that the program exists.
                        ensure!(
                            is_program_in_storage,
                            "Invalid deployment transaction '{id}' - program does not exist in the store"
                        );
                        ensure!(
                            is_program_in_process,
                            "Invalid deployment transaction '{id}' - program does not exist in the process"
                        );
                        // Get the existing program.
                        // It should be the case that the stored program matches the process program.
                        let stack = self.process().read().get_stack(deployment.program_id())?;
                        let existing_program = stack.program();
                        // Check that the new edition increments the old edition by 1.
                        let old_edition = *stack.program_edition();
                        let expected_edition = old_edition
                            .checked_add(1)
                            .ok_or_else(|| anyhow!("Invalid deployment transaction '{id}' - next edition overflows"))?;
                        ensure!(
                            expected_edition == new_edition,
                            "Invalid deployment transaction '{id}' - next edition ('{new_edition}') does not match the expected edition ('{expected_edition}')",
                        );

                        // Validate the deployment depending on whether the program has a constructor.
                        // The exact rules are listed above.
                        match deployment.program().contains_constructor() {
                            false => {
                                // Check that the existing program does not have a constructor.
                                ensure!(
                                    !existing_program.contains_constructor(),
                                    "Invalid deployment transaction '{id}' - the existing program has a constructor, but the deployment program does not"
                                );
                                // Ensure the new program matches the old program.
                                ensure!(
                                    existing_program == deployment.program(),
                                    "Invalid deployment transaction '{id}' - new program does not match the old program"
                                );
                                // Ensure that the new edition is exactly one.
                                ensure!(
                                    new_edition == 1,
                                    "Invalid deployment transaction '{id}' - programs without constructors can only be re-deployed one time."
                                );
                            }
                            true => {
                                // Ensure the existing program has a constructor.
                                ensure!(
                                    existing_program.contains_constructor(),
                                    "Invalid deployment transaction '{id}' - the existing program does not have a constructor, but the deployment program does"
                                );
                                // If the consensus version is V10 or greater, then check that each function's **record** output registers match the existing program.
                                if consensus_version >= ConsensusVersion::V10 {
                                    if let Err(e) =
                                        check_output_register_indices_unchanged(existing_program, deployment.program())
                                    {
                                        bail!("Invalid deployment transaction '{id}' - {e}")
                                    }
                                }
                            }
                        }
                    }
                }

                // Enforce the syntax restrictions on the programs based on the current consensus version.
                // Note: We do not enforce this restriction for programs with non-zero editions without constructors, since they may have been deployed before the restrictions were introduced.
                //  However, we do enforce that programs with edition one, EXACTLY match their previous edition.
                if deployment.edition() == 0 || deployment.program().contains_constructor() {
                    // Check restricted keywords for the consensus version.
                    deployment.program().check_restricted_keywords_for_consensus_version(consensus_version)?;
                    // Perform additional program checks if the consensus version is V7 or beyond.
                    if consensus_version >= ConsensusVersion::V7 {
                        deployment.program().check_program_naming_structure()?;
                    }
                }
                // Check that the program does not make any calls to `credits.aleo/upgrade`.
                // Note: This is safe to check for programs deployed before `ConsensusVersion::V8` because `credits.aleo/upgrade` was not yet introduced.
                deployment.program().check_external_calls_to_credits_upgrade()?;

                // Verify the deployment if it has not been verified before.
                if !is_partially_verified {
                    // Verify the deployment.
                    match try_vm_runtime!(|| self.check_deployment_internal(deployment, rng)) {
                        Ok(result) => result?,
                        Err(_) => bail!("VM safely halted transaction '{id}' during verification"),
                    }
                }
            }
            Transaction::Execute(id, execution_id, execution, _) => {
                // Ensure the execution was not previously rejected (replay attack prevention).
                if self.block_store().contains_rejected_deployment_or_execution_id(execution_id)? {
                    bail!("Transaction '{id}' contains a previously rejected execution")
                }

                // Ensure that the transaction does not exceed the maximum number of finalize operations.
                // A transaction can include at most `2^FINALIZE_ID_DEPTH` finalize operations total (across *all* transitions).
                // This check is needed because `MAX_WRITES * MAX_TRANSITIONS` can exceed that Merkle-tree capacity.
                let max_finalize_operations = 2u16.saturating_pow(FINALIZE_ID_DEPTH as u32);
                let total_writes = transaction
                    .transitions()
                    .map(|t| -> Result<u16> {
                        let stack = self.process.read().get_stack(t.program_id())?;
                        let program = stack.program();
                        Ok(program
                            .get_function(t.function_name())?
                            .finalize_logic()
                            .map_or(0, FinalizeCore::num_writes))
                    })
                    .try_fold(0u16, |acc, r| Ok::<u16, Error>(acc.saturating_add(r?)))?;
                ensure!(
                    total_writes <= max_finalize_operations,
                    "Transaction '{id}' exceeds the maximum number of finalize operations: {total_writes} > {max_finalize_operations}"
                );

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
            self.partially_verified_transactions.write().push(cache_key, checksum);
        }

        finish!(timer, "Verify the transaction");
        Ok(())
    }

    /// Verifies the `fee` in the given transaction. On failure, returns an error.
    #[inline]
    pub fn check_fee(
        &self,
        transaction: &Transaction<N>,
        rejected_id: Option<Field<N>>,
        is_partially_verified: bool,
    ) -> Result<()> {
        let current_height = self.block_store().current_block_height();
        let consensus_version = N::CONSENSUS_VERSION(current_height)?;
        // Get the transaction spend limit.
        let transaction_spend_limit =
            consensus_config_value_by_version!(N, TRANSACTION_SPEND_LIMIT, consensus_version).unwrap();
        match transaction {
            Transaction::Deploy(id, deployment_id, _, deployment, fee) => {
                // Ensure the rejected ID is not present.
                ensure!(rejected_id.is_none(), "Transaction '{id}' should not have a rejected ID (deployment)");
                // Compute the minimum deployment cost.
                let (minimum_cost, cost_details) =
                    deployment_cost(&self.process().read(), deployment, consensus_version)?;
                // Ensure the compute cost does not exceed the transaction spend limit.
                // Comparison logic before ConsensusVersion::V10 has been pruned to simplify the code.
                if consensus_version >= ConsensusVersion::V10 {
                    let compute_spend = deploy_compute_cost_in_microcredits(cost_details, consensus_version)?;
                    ensure!(
                        compute_spend <= transaction_spend_limit,
                        "Transaction '{id}' exceeds the transaction spend limit with compute_spend: '{compute_spend}'"
                    );
                }
                // Ensure the fee is sufficient to cover the cost.
                if *fee.base_amount()? < minimum_cost {
                    bail!(
                        "Transaction '{id}' has an insufficient base fee (deployment) - requires {minimum_cost} microcredits"
                    )
                }
                // Verify the fee.
                self.check_fee_internal(fee, *deployment_id, is_partially_verified)?;
            }
            Transaction::Execute(id, execution_id, execution, fee) => {
                // Ensure the rejected ID is not present.
                ensure!(rejected_id.is_none(), "Transaction '{id}' should not have a rejected ID (execution)");
                // If the transaction contains only 1 transition, and the transition is a split or upgrade, then the fee can be skipped.
                let is_fee_required =
                    !(execution.len() == 1 && (transaction.contains_split() || transaction.contains_upgrade()));
                // Verify the fee.
                if let Some(fee) = fee {
                    // If the fee is required, then check that the base fee amount is satisfied.
                    if is_fee_required {
                        // Compute the minimum execution cost.
                        let (minimum_cost, cost_details) =
                            execution_cost(&self.process().read(), execution, consensus_version)?;
                        // Ensure the compute cost does not exceed the transaction spend limit.
                        // Comparison logic before ConsensusVersion::V10 has been pruned to simplify the code.
                        if consensus_version >= ConsensusVersion::V10 {
                            let compute_spend = execute_compute_cost_in_microcredits(cost_details, consensus_version)?;
                            ensure!(
                                compute_spend <= transaction_spend_limit,
                                "Transaction '{id}' exceeds the transaction spend limit with compute_spend: '{compute_spend}'"
                            );
                        }
                        // Ensure the fee is sufficient to cover the cost.
                        if *fee.base_amount()? < minimum_cost {
                            bail!(
                                "Transaction '{id}' has an insufficient base fee (execution) - requires {minimum_cost} microcredits"
                            )
                        }
                    } else {
                        // Ensure the base fee amount is zero.
                        ensure!(*fee.base_amount()? == 0, "Transaction '{id}' has a non-zero base fee (execution)");
                    }
                    // Verify the fee.
                    self.check_fee_internal(fee, *execution_id, is_partially_verified)?;
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
                    Some(rejected_id) => self.check_fee_internal(fee, rejected_id, is_partially_verified)?,
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
        // Retrieve the block height.
        let block_height = self.block_store().current_block_height();
        // Determine which consensus version to use.
        let consensus_version = N::CONSENSUS_VERSION(block_height)?;

        macro_rules! logic {
            ($process:expr, $network:path, $aleo:path) => {{
                // Prepare the deployment.
                let deployment = cast_ref!(&deployment as Deployment<$network>);
                // Verify the deployment.
                $process.verify_deployment::<$aleo, _>(consensus_version, &deployment, rng)
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

        // Determine which consensus version to use.
        let consensus_version = N::CONSENSUS_VERSION(block_height)?;
        // Determine which Varuna version to use.
        let varuna_version = match (ConsensusVersion::V1..=ConsensusVersion::V3).contains(&consensus_version) {
            true => VarunaVersion::V1,
            false => VarunaVersion::V2,
        };
        // Determine the inclusion version to use.
        let is_network_behind_upgrade_height = block_height < N::INCLUSION_UPGRADE_HEIGHT()?;
        let inclusion_version = match (ConsensusVersion::V1..=ConsensusVersion::V7).contains(&consensus_version)
            || is_network_behind_upgrade_height
        {
            true => InclusionVersion::V0,
            false => InclusionVersion::V1,
        };

        // Perform checks if the execution contains `credits.aleo/upgrade`.
        if execution.transitions().any(|t| t.is_upgrade()) {
            // Do not allow `credits.aleo/upgrade` calls on the previous inclusion version or until after the migration block has passed.
            if matches!(inclusion_version, InclusionVersion::V0) {
                bail!("Execution verification failed - `credits.aleo/upgrade` cannot be called yet");
            }
            // Do not allow upgrades to be callable by other programs.
            // This is to prevent local records from being upgraded, which would ignore the record block height checks.
            if execution.transitions().len() > 1 {
                bail!("Execution verification failed - `credits.aleo/upgrade` cannot be called by another program");
            }
        }

        // Verify the execution proof, if it has not been partially-verified before.
        let verification = match is_partially_verified {
            true => Ok(()),
            false => {
                self.process.read().verify_execution(consensus_version, varuna_version, inclusion_version, execution)
            }
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
    fn check_fee_internal(
        &self,
        fee: &Fee<N>,
        deployment_or_execution_id: Field<N>,
        is_partially_verified: bool,
    ) -> Result<()> {
        let timer = timer!("VM::check_fee");

        // Ensure the fee does not exceed the limit.
        let fee_amount = fee.amount()?;
        ensure!(*fee_amount <= N::MAX_FEE, "Fee verification failed: fee exceeds the maximum limit");

        // Retrieve the block height.
        let block_height = self.block_store().current_block_height();

        // Determine which Varuna version to use.
        let consensus_version = N::CONSENSUS_VERSION(block_height)?;
        let varuna_version = match (ConsensusVersion::V1..=ConsensusVersion::V3).contains(&consensus_version) {
            true => VarunaVersion::V1,
            false => VarunaVersion::V2,
        };
        // Determine the inclusion version to use.
        let is_network_behind_upgrade_height = block_height < N::INCLUSION_UPGRADE_HEIGHT()?;
        let inclusion_version = match (ConsensusVersion::V1..=ConsensusVersion::V7).contains(&consensus_version)
            || is_network_behind_upgrade_height
        {
            true => InclusionVersion::V0,
            false => InclusionVersion::V1,
        };

        // Verify the fee, if it has not been partially-verified before.
        let verification = match is_partially_verified {
            true => Ok(()),
            false => self.process.read().verify_fee(
                consensus_version,
                varuna_version,
                inclusion_version,
                fee,
                deployment_or_execution_id,
            ),
        };
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
                Ok(false) => bail!("Fee verification failed - State root {} not found", fee.global_state_root()),
                Err(error) => bail!("Fee verification failed - Storage error - {error}"),
            },
            Err(error) => bail!("Fee verification failed - {error}"),
        };
        finish!(timer, "Check the global state root");
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::vm::test_helpers::sample_finalize_state;
    use console::account::ViewKey;

    use crate::vm::test_helpers::LedgerType;
    use console::{account::Address, types::Field};
    #[cfg(feature = "test")]
    use console::{
        algorithms::{ECDSASignature, Keccak256},
        types::U8,
    };
    use snarkvm_ledger_block::{Block, Header, Metadata, Transaction, Transition};
    #[cfg(feature = "test")]
    use snarkvm_utilities::bytes_from_bits_le;

    type CurrentNetwork = test_helpers::CurrentNetwork;

    // A helper function to create the cache key for a transaction in the partially-verified transactions cache.
    fn create_cache_key(
        vm: &VM<CurrentNetwork, LedgerType>,
        transaction: &Transaction<CurrentNetwork>,
    ) -> (<CurrentNetwork as Network>::TransactionID, Vec<U16<CurrentNetwork>>) {
        // Get the program editions.
        let program_editions = transaction
            .transitions()
            .map(|transition| {
                vm.process().read().get_stack(transition.program_id()).map(|stack| stack.program_edition())
            })
            .collect::<Result<Vec<_>>>()
            .unwrap();
        // Return the cache key.
        (transaction.id(), program_editions)
    }

    #[test]
    fn test_verify() {
        let rng = &mut TestRng::default();
        let vm = crate::vm::test_helpers::sample_vm_with_genesis_block(rng);

        // Fetch a deployment transaction.
        let deployment_transaction = crate::vm::test_helpers::sample_deployment_transaction(rng);
        let cache_key = create_cache_key(&vm, &deployment_transaction);
        // Ensure the transaction verifies.
        vm.check_transaction(&deployment_transaction, None, rng).unwrap();
        // Ensure the partially_verified_transactions cache is updated.
        assert!(vm.partially_verified_transactions.read().peek(&cache_key).is_some());

        // Fetch an execution transaction.
        let execution_transaction = crate::vm::test_helpers::sample_execution_transaction_with_private_fee(rng);
        let cache_key = create_cache_key(&vm, &execution_transaction);
        // Ensure the transaction verifies.
        vm.check_transaction(&execution_transaction, None, rng).unwrap();
        // Ensure the partially_verified_transactions cache is updated.
        assert!(vm.partially_verified_transactions.read().peek(&cache_key).is_some());

        // Fetch an execution transaction.
        let execution_transaction = crate::vm::test_helpers::sample_execution_transaction_with_public_fee(rng);
        let cache_key = create_cache_key(&vm, &execution_transaction);
        // Ensure the transaction verifies.
        vm.check_transaction(&execution_transaction, None, rng).unwrap();
        // Ensure the partially_verified_transactions cache is updated.
        assert!(vm.partially_verified_transactions.read().peek(&cache_key).is_some());
    }

    #[test]
    fn test_verify_deployment() {
        let rng = &mut TestRng::default();
        let vm = crate::vm::test_helpers::sample_vm();

        // Fetch the program from the deployment.
        let program = crate::vm::test_helpers::sample_program();

        // Deploy the program.
        let deployment = vm.deploy_raw(&program, rng).unwrap();

        // Get the size of the cache.
        let cache_size = vm.partially_verified_transactions.read().len();

        // Ensure the deployment is valid.
        vm.check_deployment_internal(&deployment, rng).unwrap();
        // Ensure the partially_verified_transactions cache has the same size.
        assert_eq!(vm.partially_verified_transactions.read().len(), cache_size);

        // Ensure that deserialization doesn't break the transaction verification.
        let serialized_deployment = deployment.to_string();
        let deployment_transaction: Deployment<CurrentNetwork> = serde_json::from_str(&serialized_deployment).unwrap();
        vm.check_deployment_internal(&deployment_transaction, rng).unwrap();
        // Ensure the partially_verified_transactions cache has the same size.
        assert_eq!(vm.partially_verified_transactions.read().len(), cache_size);
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

        // Get the cache size.
        let cache_size = vm.partially_verified_transactions.read().len();

        for transaction in transactions {
            match transaction {
                Transaction::Execute(_, _, execution, _) => {
                    // Ensure the proof exists.
                    assert!(execution.proof().is_some());
                    // Verify the execution.
                    vm.check_execution_internal(&execution, false).unwrap();
                    // Ensure the partially_verified_transactions cache has the same size.
                    assert_eq!(vm.partially_verified_transactions.read().len(), cache_size);

                    // Ensure that deserialization doesn't break the transaction verification.
                    let serialized_execution = execution.to_string();
                    let recovered_execution: Execution<CurrentNetwork> =
                        serde_json::from_str(&serialized_execution).unwrap();
                    vm.check_execution_internal(&recovered_execution, false).unwrap();
                    // Ensure the partially_verified_transactions cache has the same size.
                    assert_eq!(vm.partially_verified_transactions.read().len(), cache_size);
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

        // Get the cache size.
        let cache_size = vm.partially_verified_transactions.read().len();

        for transaction in transactions {
            match transaction {
                Transaction::Execute(_, _, execution, Some(fee)) => {
                    let execution_id = execution.to_execution_id().unwrap();

                    // Ensure the proof exists.
                    assert!(fee.proof().is_some());
                    // Verify the fee.
                    vm.check_fee_internal(&fee, execution_id, false).unwrap();
                    // Ensure the partially_verified_transactions cache has the same size.
                    assert_eq!(vm.partially_verified_transactions.read().len(), cache_size);

                    // Ensure that deserialization doesn't break the transaction verification.
                    let serialized_fee = fee.to_string();
                    let recovered_fee: Fee<CurrentNetwork> = serde_json::from_str(&serialized_fee).unwrap();
                    vm.check_fee_internal(&recovered_fee, execution_id, false).unwrap();
                    // Ensure the partially_verified_transactions cache has the same size.
                    assert_eq!(vm.partially_verified_transactions.read().len(), cache_size);
                }
                _ => panic!("Expected an execution with a fee"),
            }
        }
    }

    #[test]
    #[ignore]
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
        let cache_key = create_cache_key(&vm, &valid_transaction);
        vm.check_transaction(&valid_transaction, None, rng).unwrap();
        // Ensure the partially_verified_transactions cache is updated.
        assert!(vm.partially_verified_transactions.read().peek(&cache_key).is_some());

        // Fetch a valid execution transaction with a public fee.
        let valid_transaction = crate::vm::test_helpers::sample_execution_transaction_with_public_fee(rng);
        let cache_key = create_cache_key(&vm, &valid_transaction);
        vm.check_transaction(&valid_transaction, None, rng).unwrap();
        // Ensure the partially_verified_transactions cache is updated.
        assert!(vm.partially_verified_transactions.read().peek(&cache_key).is_some());

        // Fetch a valid execution transaction with no fee.
        let valid_transaction = crate::vm::test_helpers::sample_execution_transaction_without_fee(rng);
        let cache_key = create_cache_key(&vm, &valid_transaction);
        vm.check_transaction(&valid_transaction, None, rng).unwrap();
        // Ensure the partially_verified_transactions cache is updated.
        assert!(vm.partially_verified_transactions.read().peek(&cache_key).is_some());
    }

    #[test]
    #[ignore]
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
        let cache_key = create_cache_key(&vm, &transaction);

        // Verify.
        vm.check_transaction(&transaction, None, rng).unwrap();
        // Ensure the partially_verified_transactions cache is updated.
        assert!(vm.partially_verified_transactions.read().peek(&cache_key).is_some());
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
        let cache_key = create_cache_key(&vm, &valid_transaction);
        // Ensure the partially_verified_transactions cache is updated.
        assert!(vm.partially_verified_transactions.read().peek(&cache_key).is_some());

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
        let cache_key = create_cache_key(&vm, &mutated_transaction);

        // Ensure that the mutated transaction fails verification due to an extra output.
        assert!(vm.check_transaction(&mutated_transaction, None, rng).is_err());
        // Ensure the partially_verified_transactions cache is not updated.
        assert!(vm.partially_verified_transactions.read().peek(&cache_key).is_none());
    }

    #[cfg(feature = "test")]
    #[test]
    fn test_varuna_migration() {
        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch the private key.
        let private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

        // Create a transaction with on the old version.
        let address = Address::try_from(&private_key).unwrap();
        let inputs = [
            Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
            Value::<CurrentNetwork>::from_str("1u64").unwrap(),
        ]
        .into_iter();
        let transaction_v1 =
            vm.execute(&private_key, ("credits.aleo", "transfer_public"), inputs, None, 0, None, rng).unwrap();

        // Advance the ledger past ConsensusV4
        let transactions: [Transaction<CurrentNetwork>; 0] = [];
        for _ in 0..CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V4).unwrap() {
            // Check that the v1 transaction is valid.
            assert!(vm.check_transaction(&transaction_v1, None, rng).is_ok());
            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // Check that the v1 transaction is invalid
        assert!(vm.check_transaction(&transaction_v1, None, rng).is_err());

        // Create a transaction with on the new version.
        let address = Address::try_from(&private_key).unwrap();
        let inputs = [
            Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
            Value::<CurrentNetwork>::from_str("1u64").unwrap(),
        ]
        .into_iter();
        let transaction_v2 =
            vm.execute(&private_key, ("credits.aleo", "transfer_public"), inputs, None, 0, None, rng).unwrap();

        // Check that the v2 transaction is valid
        assert!(vm.check_transaction(&transaction_v2, None, rng).is_ok());

        // Sample a new VM
        let new_vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        new_vm.add_next_block(&genesis).unwrap();

        // Check that v1 transaction is valid.
        assert!(new_vm.check_transaction(&transaction_v1, None, rng).is_ok());
        // Check that v2 transaction is invalid.
        assert!(new_vm.check_transaction(&transaction_v2, None, rng).is_err());
    }

    #[cfg(feature = "test")]
    #[test]
    fn test_program_rules_migration() {
        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch the private key.
        let private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

        // Advance the ledger past ConsensusV4 where the new varuna version starts to take place.
        let transactions: [Transaction<CurrentNetwork>; 0] = [];
        while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V4).unwrap()
        {
            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // Create a new program that contains "aleo" in the name.
        let program_1 = Program::from_str(
            r"
program testing_1_aleo.aleo;

record token:
    owner as address.private;
    amount as u64.private;

function compute:
    input r0 as u32.private;
    add r0 r0 into r1;
    output r1 as u32.public;",
        )
        .unwrap();

        // Create a new program that contains "aleo" in the record name.
        let program_2 = Program::from_str(
            r"
program testing_2.aleo;

record token_aleo:
    owner as address.private;
    amount as u64.private;

function compute:
    input r0 as u32.private;
    add r0 r0 into r1;
    output r1 as u32.public;",
        )
        .unwrap();

        // Create a new program with records names that have other record names as a prefix.
        let program_3 = Program::from_str(
            r"
program testing_3.aleo;

record token:
    owner as address.private;
    amount as u64.private;

record token_2:
    owner as address.private;
    amount as u64.private;

function compute:
    input r0 as u32.private;
    add r0 r0 into r1;
    output r1 as u32.public;",
        )
        .unwrap();

        // Create a new program with records that has an entry containing "aleo".
        let program_4 = Program::from_str(
            r"
program testing_4.aleo;

record token:
    owner as address.private;
    test_aleo as u64.private;
    amount as u64.private;

function compute:
    input r0 as u32.private;
    add r0 r0 into r1;
    output r1 as u32.public;",
        )
        .unwrap();

        // Create a deployment transaction for the first program.
        let deploy_1 = vm.deploy(&private_key, &program_1, None, 0, None, rng).unwrap();

        // Create a deployment transaction for the second program.
        let deploy_2 = vm.deploy(&private_key, &program_2, None, 0, None, rng).unwrap();

        // Create a deployment transaction for the third program.
        let deploy_3 = vm.deploy(&private_key, &program_3, None, 0, None, rng).unwrap();

        // Create a deployment transaction for the fourth program.
        let deploy_4 = vm.deploy(&private_key, &program_4, None, 0, None, rng).unwrap();

        // // Ensure that the deployments are valid.
        assert!(vm.check_transaction(&deploy_1, None, rng).is_ok());
        assert!(vm.check_transaction(&deploy_2, None, rng).is_ok());
        assert!(vm.check_transaction(&deploy_3, None, rng).is_ok());
        assert!(vm.check_transaction(&deploy_4, None, rng).is_ok());

        // Advance the ledger past ConsensusVersion::V7
        let transactions: [Transaction<CurrentNetwork>; 0] = [];
        while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V7).unwrap()
        {
            // // Ensure that the deployments are valid.
            assert!(vm.check_transaction(&deploy_1, None, rng).is_ok());
            assert!(vm.check_transaction(&deploy_2, None, rng).is_ok());
            assert!(vm.check_transaction(&deploy_3, None, rng).is_ok());
            assert!(vm.check_transaction(&deploy_4, None, rng).is_ok());
            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // Ensure that the deployments are no longer valid.
        assert!(vm.check_transaction(&deploy_1, None, rng).is_err());
        assert!(vm.check_transaction(&deploy_2, None, rng).is_err());
        assert!(vm.check_transaction(&deploy_3, None, rng).is_err());
        assert!(vm.check_transaction(&deploy_4, None, rng).is_err());

        // Check that the next block will abort the deployments.
        let deploy_1_tx_id = deploy_1.id();
        let deploy_2_tx_id = deploy_2.id();
        let deploy_3_tx_id = deploy_3.id();
        let deploy_4_tx_id = deploy_4.id();
        let next_block = crate::vm::test_helpers::sample_next_block(
            &vm,
            &private_key,
            &[deploy_1, deploy_2, deploy_3, deploy_4],
            rng,
        )
        .unwrap();
        assert_eq!(next_block.aborted_transaction_ids(), &vec![
            deploy_1_tx_id,
            deploy_2_tx_id,
            deploy_3_tx_id,
            deploy_4_tx_id
        ]);
    }

    #[cfg(feature = "test")]
    #[test]
    fn test_ecdsa_migration() {
        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch the private key.
        let private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

        // Deploy a test program to the ledger.
        let program_id = ProgramID::<CurrentNetwork>::from_str("dummy_program.aleo").unwrap();
        let program = Program::<CurrentNetwork>::from_str(&format!(
            r"
    program {program_id};
    function foo:
        input r0 as [u8; 65u32].public;
        input r1 as [u8; 20u32].public;
        input r2 as [u8; 100u32].public;
        async foo r0 r1 r2 into r3;
        output r3 as {program_id}/foo.future;
    finalize foo:
        input r0 as [u8; 65u32].public;
        input r1 as [u8; 20u32].public;
        input r2 as [u8; 100u32].public;
        ecdsa.verify.keccak256.eth r0 r1 r2 into r3;
        assert.eq r3 true;

    function foo_2:
        input r0 as [u8; 65u32].public;
        input r1 as [u8; 20u32].public;
        input r2 as [u8; 32u32].public;
        async foo_2 r0 r1 r2 into r3;
        output r3 as {program_id}/foo_2.future;
    finalize foo_2:
        input r0 as [u8; 65u32].public;
        input r1 as [u8; 20u32].public;
        input r2 as [u8; 32u32].public;
        ecdsa.verify.digest.eth r0 r1 r2 into r3;
        assert.eq r3 true;

    constructor:
        assert.eq edition 0u16;",
        ))
        .unwrap();

        // Advance the ledger past ConsensusV9 where the new varuna version and deployment version starts to take place.
        let transactions: [Transaction<CurrentNetwork>; 0] = [];
        while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V9).unwrap()
        {
            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // Construct the deployment transaction.
        let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

        // Advance the ledger past ConsensusV11 where the new varuna version starts to take place.
        let transactions: [Transaction<CurrentNetwork>; 0] = [];
        while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V11).unwrap()
        {
            // Ensure that the deployment is invalid.
            assert!(vm.check_transaction(&deployment, None, rng).is_err());

            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // Ensure that the deployment is valid after ConsensusVersion::V11.
        assert!(vm.check_transaction(&deployment, None, rng).is_ok());

        // Deploy the program.
        let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[deployment], rng).unwrap();
        vm.add_next_block(&next_block).unwrap();

        // Execute the program and ensure that the signature verifies.
        let ecdsa_signing_key = k256::ecdsa::SigningKey::random(rng);
        let ecdsa_verifying_key = k256::ecdsa::VerifyingKey::from(&ecdsa_signing_key);
        let ethereum_address = ECDSASignature::ethereum_address_from_public_key(&ecdsa_verifying_key).unwrap();
        let message: [u8; 100] = (0..100).map(|_| rng.r#gen::<u8>()).collect::<Vec<u8>>().try_into().unwrap();
        let hasher = Keccak256::default();
        let signature = ECDSASignature::sign(&ecdsa_signing_key, &hasher, &message.to_bits_le()).unwrap();
        let signature_bytes = signature.to_bytes_le().unwrap();
        let digest = bytes_from_bits_le(&hasher.hash(&message.to_bits_le()).unwrap());

        // Convert the inputs to plaintext Values.
        let ethereum_address: [U8<CurrentNetwork>; 20] =
            ethereum_address.into_iter().map(U8::new).collect::<Vec<U8<CurrentNetwork>>>().try_into().unwrap();
        let message: [U8<CurrentNetwork>; 100] =
            message.into_iter().map(U8::new).collect::<Vec<U8<CurrentNetwork>>>().try_into().unwrap();
        let signature: [U8<CurrentNetwork>; 65] =
            signature_bytes.into_iter().map(U8::new).collect::<Vec<U8<CurrentNetwork>>>().try_into().unwrap();
        let digest: [U8<CurrentNetwork>; 32] =
            digest.into_iter().map(U8::new).collect::<Vec<U8<CurrentNetwork>>>().try_into().unwrap();

        // Construct the inputs.
        let inputs = [
            Value::<CurrentNetwork>::Plaintext(Plaintext::from(signature)),
            Value::<CurrentNetwork>::Plaintext(Plaintext::from(ethereum_address)),
            Value::<CurrentNetwork>::Plaintext(Plaintext::from(message)),
        ];
        // Create the execution transaction.
        let verification_transaction =
            vm.execute(&private_key, (&program_id.to_string(), "foo"), inputs.into_iter(), None, 0, None, rng).unwrap();
        let valid_tx_id = verification_transaction.id();

        // Construct the inputs for the digest verfication.
        let inputs = [
            Value::<CurrentNetwork>::Plaintext(Plaintext::from(signature)),
            Value::<CurrentNetwork>::Plaintext(Plaintext::from(ethereum_address)),
            Value::<CurrentNetwork>::Plaintext(Plaintext::from(digest)),
        ];
        // Create the execution transaction.
        let digest_verification_transaction = vm
            .execute(&private_key, (&program_id.to_string(), "foo_2"), inputs.into_iter(), None, 0, None, rng)
            .unwrap();
        let valid_tx_id_2 = digest_verification_transaction.id();

        // Construct an invalid execution transaction by mutating the message.
        let invalid_message: [u8; 100] = (0..100).map(|_| rng.r#gen::<u8>()).collect::<Vec<u8>>().try_into().unwrap();
        let invalid_message: [U8<CurrentNetwork>; 100] =
            invalid_message.into_iter().map(U8::new).collect::<Vec<U8<CurrentNetwork>>>().try_into().unwrap();

        // Construct the inputs for the invalid execution.
        let inputs = [
            Value::<CurrentNetwork>::Plaintext(Plaintext::from(signature)),
            Value::<CurrentNetwork>::Plaintext(Plaintext::from(ethereum_address)),
            Value::<CurrentNetwork>::Plaintext(Plaintext::from(invalid_message)),
        ];

        // Create the execution transaction.
        let invalid_verification_transaction =
            vm.execute(&private_key, (&program_id.to_string(), "foo"), inputs.into_iter(), None, 0, None, rng).unwrap();
        let invalid_tx_id = invalid_verification_transaction.id();

        // Construct a block with both transactions.
        let next_block = crate::vm::test_helpers::sample_next_block(
            &vm,
            &private_key,
            &[verification_transaction, digest_verification_transaction, invalid_verification_transaction],
            rng,
        )
        .unwrap();
        vm.add_next_block(&next_block).unwrap();

        // Ensure that the valid transaction was accepted and the invalid one was rejected.
        assert_eq!(next_block.transactions().num_accepted(), 2);
        assert_eq!(next_block.transactions().num_rejected(), 1);
        assert!(vm.block_store().get_confirmed_transaction(&valid_tx_id).unwrap().unwrap().is_accepted());
        assert!(vm.block_store().get_confirmed_transaction(&valid_tx_id_2).unwrap().unwrap().is_accepted());
        assert!(vm.block_store().get_confirmed_transaction(&invalid_tx_id).unwrap().unwrap().is_rejected());
    }

    #[cfg(feature = "test")]
    #[test]
    fn test_increased_array_size() {
        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch the private key.
        let private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

        // Deploy a test program to the ledger.
        let program_id = ProgramID::<CurrentNetwork>::from_str("dummy_program.aleo").unwrap();
        let program = Program::<CurrentNetwork>::from_str(&format!(
            r"
    program {program_id};
    function foo:
        input r0 as [u8; 256u32].public;

    constructor:
        assert.eq edition 0u16;",
        ))
        .unwrap();

        // Advance the ledger past ConsensusV9 where the new varuna version and deployment version starts to take place.
        let transactions: [Transaction<CurrentNetwork>; 0] = [];
        while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V9).unwrap()
        {
            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // Construct the deployment transaction.
        let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

        // Advance the ledger past ConsensusV11 where the new varuna version starts to take place.
        let transactions: [Transaction<CurrentNetwork>; 0] = [];
        while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V11).unwrap()
        {
            // Ensure that the deployment is invalid.
            assert!(vm.check_transaction(&deployment, None, rng).is_err());

            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // Ensure that the deployment is valid after ConsensusVersion::V11.
        assert!(vm.check_transaction(&deployment, None, rng).is_ok());
    }

    #[cfg(feature = "test")]
    #[test]
    fn test_block_timestamp_migration() {
        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch the private key.
        let private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);

        // Deploy a test program to the ledger.
        let program_id = ProgramID::<CurrentNetwork>::from_str("dummy_program.aleo").unwrap();
        let program = Program::<CurrentNetwork>::from_str(&format!(
            r"
    program {program_id};
    function foo:
        input r0 as i64.public;
        async foo r0 into r1;
        output r1 as {program_id}/foo.future;
    finalize foo:
        input r0 as i64.public;
        gte r0 block.timestamp into r1;
        assert.eq r1 true;

    constructor:
        assert.eq edition 0u16;",
        ))
        .unwrap();

        // Advance the ledger past ConsensusV9 where the new varuna version and deployment version starts to take place.
        let transactions: [Transaction<CurrentNetwork>; 0] = [];
        while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V9).unwrap()
        {
            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // Construct the deployment transaction.
        let deployment = vm.deploy(&private_key, &program, None, 0, None, rng).unwrap();

        // Advance the ledger past ConsensusV12 where the new varuna version starts to take place.
        let transactions: [Transaction<CurrentNetwork>; 0] = [];
        while vm.block_store().current_block_height() < CurrentNetwork::CONSENSUS_HEIGHT(ConsensusVersion::V12).unwrap()
        {
            // Ensure that the deployment is invalid.
            assert!(vm.check_transaction(&deployment, None, rng).is_err());

            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // Ensure that the deployment is valid after ConsensusVersion::V12.
        assert!(vm.check_transaction(&deployment, None, rng).is_ok());

        // Deploy the program.
        let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[deployment], rng).unwrap();
        vm.add_next_block(&next_block).unwrap();

        // Construct the input with the valid timestamp.
        let future_timestamp = next_block.timestamp() + 100;
        let inputs = [Value::<CurrentNetwork>::Plaintext(Plaintext::from(Literal::I64(console::types::I64::new(
            future_timestamp,
        ))))];
        // Create the execution transaction.
        let valid_transaction =
            vm.execute(&private_key, (&program_id.to_string(), "foo"), inputs.into_iter(), None, 0, None, rng).unwrap();
        let valid_tx_id = valid_transaction.id();

        // Construct the input with an invalid timestamp.
        let past_timestamp = next_block.timestamp() - 100;
        let inputs = [Value::<CurrentNetwork>::Plaintext(Plaintext::from(Literal::I64(console::types::I64::new(
            past_timestamp,
        ))))];
        // Create the execution transaction.
        let invalid_transaction =
            vm.execute(&private_key, (&program_id.to_string(), "foo"), inputs.into_iter(), None, 0, None, rng).unwrap();
        let invalid_tx_id = invalid_transaction.id();

        // Construct a block with both transactions.
        let next_block = crate::vm::test_helpers::sample_next_block(
            &vm,
            &private_key,
            &[valid_transaction, invalid_transaction],
            rng,
        )
        .unwrap();
        vm.add_next_block(&next_block).unwrap();

        // Ensure that the valid transaction was accepted and the invalid one was rejected.
        assert_eq!(next_block.transactions().num_accepted(), 1);
        assert_eq!(next_block.transactions().num_rejected(), 1);
        assert!(vm.block_store().get_confirmed_transaction(&valid_tx_id).unwrap().unwrap().is_accepted());
        assert!(vm.block_store().get_confirmed_transaction(&invalid_tx_id).unwrap().unwrap().is_rejected());
    }
}

#[cfg(feature = "test")]
#[cfg(test)]
mod credits_migration_tests {
    use super::*;

    use console::{
        account::{Address, ViewKey},
        program::Entry,
    };
    use snarkvm_ledger_block::Transition;

    type CurrentNetwork = test_helpers::CurrentNetwork;

    const RECORD_UPGRADE_LIMIT: u64 = 1_000_000_000_000u64;
    const TOTAL_UPGRADE_LIMIT: u64 = 4_000_000_000_000u64;

    #[ignore]
    #[test]
    fn test_inclusion_migration() {
        // 1. Check that `upgrade` is not callable before migration
        // 2. Construct blocks until migration occurs
        // 3. Check that records generated at exactly the migration block requires `upgrade`.
        // 4. Check that records from before the `upgrade_block_height` can't be spent.
        // 5. Check that `upgrade` works on the above record.
        // 6. Check that `upgrade` does not work on already upgraded records.
        // 7. Check that the upgraded records can now be spent.
        // 8. Check that the records above 500,000 credits are properly aborted.

        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch the private key.
        let private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        let view_key = ViewKey::try_from(&private_key).unwrap();
        let address = Address::try_from(&private_key).unwrap();

        // Track the total upgraded amount.
        let mut total_upgraded = 0;

        // Fetch the unspent record.
        let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
        let genesis_records = records.values().map(|record| record.decrypt(&view_key).unwrap()).collect::<Vec<_>>();

        let split_transactions: Vec<_> = (0..4)
            .map(|i| {
                let inputs = [
                    Value::<CurrentNetwork>::Record(genesis_records[i].clone()),
                    Value::<CurrentNetwork>::from_str(&format!("{RECORD_UPGRADE_LIMIT}u64")).unwrap(),
                ]
                .into_iter();
                vm.execute(&private_key, ("credits.aleo", "split"), inputs, None, 0, None, rng).unwrap()
            })
            .collect();

        // Create a new block that includes the split.
        let next_block =
            crate::vm::test_helpers::sample_next_block(&vm, &private_key, &split_transactions, rng).unwrap();
        vm.add_next_block(&next_block).unwrap();

        // Fetch the records from the new block.
        let split_records =
            next_block.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
        let split_records = split_records.values().map(|record| record.decrypt(&view_key).unwrap()).collect::<Vec<_>>();

        // Create more splits
        let more_split_transactions: Vec<_> = (0..4)
            .map(|i| {
                let inputs = [
                    Value::<CurrentNetwork>::Record(split_records[2 * i + 1].clone()),
                    Value::<CurrentNetwork>::from_str(&format!("{RECORD_UPGRADE_LIMIT}u64")).unwrap(),
                ]
                .into_iter();
                vm.execute(&private_key, ("credits.aleo", "split"), inputs, None, 0, None, rng).unwrap()
            })
            .collect();

        // Create a new block that includes the split.
        let next_block =
            crate::vm::test_helpers::sample_next_block(&vm, &private_key, &more_split_transactions, rng).unwrap();
        vm.add_next_block(&next_block).unwrap();

        // Fetch the records from the new block.
        let additional_split_records =
            next_block.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
        let additional_split_records =
            additional_split_records.values().map(|record| record.decrypt(&view_key).unwrap()).collect::<Vec<_>>();

        // ----------------------------------------------------------------------------------------
        // 1. Check that `upgrade` is not callable before migration
        // ----------------------------------------------------------------------------------------

        let microcredits = Identifier::from_str("microcredits").unwrap();
        let upgrade_1 = {
            let record_to_spend = split_records[0].clone();
            let amount = match record_to_spend.data().get(&microcredits) {
                Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => **amount,
                _ => panic!("Invalid record"),
            };
            assert!(amount <= RECORD_UPGRADE_LIMIT);
            total_upgraded += amount;
            let inputs = [Value::<CurrentNetwork>::Record(record_to_spend)].into_iter();
            vm.execute(&private_key, ("credits.aleo", "upgrade"), inputs, None, 0, None, rng).unwrap()
        };
        assert!(vm.check_transaction(&upgrade_1, None, rng).is_err());

        // ----------------------------------------------------------------------------------------
        // 2. Construct blocks until migration occurs
        // ----------------------------------------------------------------------------------------

        let upgrade_height = CurrentNetwork::INCLUSION_UPGRADE_HEIGHT().unwrap();

        while vm.block_store().current_block_height() <= upgrade_height {
            let mut transactions = vec![];
            // Add the split transaction to the block created at exactly `INCLUSION_UPGRADE_HEIGHT`
            if vm.block_store().current_block_height() == upgrade_height - 1 {
                let split_transaction_3 = {
                    let inputs = [
                        Value::<CurrentNetwork>::Record(additional_split_records[1].clone()),
                        Value::<CurrentNetwork>::from_str(&format!("{RECORD_UPGRADE_LIMIT}u64")).unwrap(),
                    ]
                    .into_iter();
                    vm.execute(&private_key, ("credits.aleo", "split"), inputs, None, 0, None, rng).unwrap()
                };

                transactions.push(split_transaction_3.clone());
            }
            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &transactions, rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // ----------------------------------------------------------------------------------------
        // 3. Check that records generated at exactly the migration block requires `upgrade`.
        // ----------------------------------------------------------------------------------------

        // Fetch the records created at the migration block.
        let migration_block_hash = vm.block_store().get_block_hash(upgrade_height).unwrap().unwrap();
        let migration_block_transactions =
            vm.block_store().get_block_transactions(&migration_block_hash).unwrap().unwrap();
        let split_records_2 = migration_block_transactions
            .transitions()
            .cloned()
            .flat_map(Transition::into_records)
            .collect::<IndexMap<_, _>>();
        let split_records_2 =
            split_records_2.values().map(|record| record.decrypt(&view_key).unwrap()).collect::<Vec<_>>();

        // Check that a `transfer_private` is invalid.
        {
            let inputs = [
                Value::<CurrentNetwork>::Record(split_records_2[0].clone()),
                Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
            ]
            .into_iter();
            assert!(
                vm.execute(&private_key, ("credits.aleo", "transfer_private"), inputs, None, 0, None, rng).is_err()
            );
        }

        // Check that an `upgrade` is valid.
        {
            let record_to_spend = split_records_2[0].clone();
            let amount = match record_to_spend.data().get(&microcredits) {
                Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => **amount,
                _ => panic!("Invalid record"),
            };
            assert!(amount <= RECORD_UPGRADE_LIMIT);
            let inputs = [Value::<CurrentNetwork>::Record(record_to_spend)].into_iter();
            let upgrade = vm.execute(&private_key, ("credits.aleo", "upgrade"), inputs, None, 0, None, rng).unwrap();
            assert!(vm.check_transaction(&upgrade, None, rng).is_ok());
        }

        // ----------------------------------------------------------------------------------------
        // 4. Check that records from before the `upgrade_block_height` can't be spent.
        // ----------------------------------------------------------------------------------------

        let record_to_spend = split_records[0].clone();
        let inputs = [
            Value::<CurrentNetwork>::Record(record_to_spend),
            Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
            Value::<CurrentNetwork>::from_str("1u64").unwrap(),
        ]
        .into_iter();
        assert!(vm.execute(&private_key, ("credits.aleo", "transfer_private"), inputs, None, 0, None, rng).is_err());

        // ----------------------------------------------------------------------------------------
        // 5. Check that `upgrade` works on the above record.
        // ----------------------------------------------------------------------------------------

        let upgrade_2 = {
            let record_to_spend = split_records[0].clone();
            let amount = match record_to_spend.data().get(&microcredits) {
                Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => **amount,
                _ => panic!("Invalid record"),
            };
            assert!(amount <= RECORD_UPGRADE_LIMIT);
            let inputs = [Value::<CurrentNetwork>::Record(record_to_spend)].into_iter();
            vm.execute(&private_key, ("credits.aleo", "upgrade"), inputs, None, 0, None, rng).unwrap()
        };
        assert!(vm.check_transaction(&upgrade_2, None, rng).is_ok());

        let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[upgrade_2], rng).unwrap();
        vm.add_next_block(&next_block).unwrap();
        assert_eq!(next_block.transactions().len(), 1);

        // ----------------------------------------------------------------------------------------
        // 6. Check that `upgrade` does not work on already upgraded records.
        // ----------------------------------------------------------------------------------------

        // Fetch the records from the new block.
        let upgraded_records =
            next_block.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
        let upgraded_records =
            upgraded_records.values().map(|record| record.decrypt(&view_key).unwrap()).collect::<Vec<_>>();

        let record_to_spend = upgraded_records[0].clone();
        let inputs = [Value::<CurrentNetwork>::Record(record_to_spend)].into_iter();
        assert!(vm.execute(&private_key, ("credits.aleo", "upgrade"), inputs, None, 0, None, rng).is_err());

        // ----------------------------------------------------------------------------------------
        // 7. Check that the upgraded records can now be spent.
        // ----------------------------------------------------------------------------------------

        let transfer_private = {
            let record_to_spend = upgraded_records[0].clone();
            let inputs = [
                Value::<CurrentNetwork>::Record(record_to_spend),
                Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("1u64").unwrap(),
            ]
            .into_iter();
            vm.execute(&private_key, ("credits.aleo", "transfer_private"), inputs, None, 0, None, rng).unwrap()
        };

        assert!(vm.check_transaction(&transfer_private, None, rng).is_ok());

        // ----------------------------------------------------------------------------------------
        // 8. Check that the upgrades will abort if we are past the upgrade limit.
        // ----------------------------------------------------------------------------------------

        let upgrades: Vec<_> = (1..4)
            .map(|i| {
                let record_to_spend = split_records[2 * i].clone();
                let amount = match record_to_spend.data().get(&microcredits) {
                    Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => **amount,
                    _ => panic!("Invalid record"),
                };
                assert!(amount <= RECORD_UPGRADE_LIMIT);
                total_upgraded += amount;
                let inputs = [Value::<CurrentNetwork>::Record(record_to_spend)].into_iter();
                vm.execute(&private_key, ("credits.aleo", "upgrade"), inputs, None, 0, None, rng).unwrap()
            })
            .collect();

        let additional_upgrades: Vec<_> = (0..4)
            .map(|i| {
                let record_to_spend = additional_split_records[2 * i].clone();
                let amount = match record_to_spend.data().get(&microcredits) {
                    Some(Entry::Private(Plaintext::Literal(Literal::U64(amount), _))) => **amount,
                    _ => panic!("Invalid record"),
                };
                assert!(amount <= RECORD_UPGRADE_LIMIT);
                total_upgraded += amount;
                let inputs = [Value::<CurrentNetwork>::Record(record_to_spend)].into_iter();
                vm.execute(&private_key, ("credits.aleo", "upgrade"), inputs, None, 0, None, rng).unwrap()
            })
            .collect();

        let combined_upgrades = [upgrades, additional_upgrades].concat();

        let next_block =
            crate::vm::test_helpers::sample_next_block(&vm, &private_key, &combined_upgrades, rng).unwrap();
        vm.add_next_block(&next_block).unwrap();
        // Ensure that the total upgraded amount is properly bound.
        assert!(total_upgraded > TOTAL_UPGRADE_LIMIT);
        println!("\n\n TOTAL UPGRADED: {total_upgraded}\n\n");
        let num_aborted = usize::try_from((total_upgraded - TOTAL_UPGRADE_LIMIT) / RECORD_UPGRADE_LIMIT).unwrap();
        assert_eq!(next_block.transactions().len() + num_aborted, combined_upgrades.len());
        assert_eq!(next_block.aborted_transaction_ids().len(), num_aborted);
        assert!(num_aborted > 0);
    }

    #[cfg(feature = "test")]
    #[test]
    fn test_inclusion_local_records() {
        // 1. Check that records that have not been upgraded can't be spent via calls
        // 2. Check that `credits.aleo/upgrade` can be called invoked directly.
        // 3. Check that local records can be spent and checked properly.
        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch the private key.
        let private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        let view_key = ViewKey::try_from(&private_key).unwrap();
        let address = Address::try_from(&private_key).unwrap();

        // Fetch the unspent record.
        let records = genesis.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
        let genesis_records = records.values().map(|record| record.decrypt(&view_key).unwrap()).collect::<Vec<_>>();

        // Deploy the program.
        let program = Program::from_str(
            r"
import credits.aleo;

program test_local_calls.aleo;

function proxy_transfer:
    input r0 as credits.aleo/credits.record;
    input r1 as address.private;
    input r2 as u64.private;
    call credits.aleo/transfer_private r0 r1 r2 into r3 r4;
    output r3 as credits.aleo/credits.record;
    output r4 as credits.aleo/credits.record;

function local_transfer:
    input r0 as credits.aleo/credits.record;
    input r1 as address.private;
    input r2 as u64.private;
    call credits.aleo/transfer_private r0 r1 r2 into r3 r4;
    call credits.aleo/transfer_private r3 r1 r2 into r5 r6;
    output r3 as credits.aleo/credits.record;
    output r4 as credits.aleo/credits.record;
    output r5 as credits.aleo/credits.record;
    output r6 as credits.aleo/credits.record;
    ",
        )
        .unwrap();
        let deployment = vm.deploy(&private_key, &program, None, 1, None, rng).unwrap();
        vm.add_next_block(&crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[deployment], rng).unwrap())
            .unwrap();

        // Create a split transaction before the migration.
        let split_transaction = {
            let inputs = [
                Value::<CurrentNetwork>::Record(genesis_records[0].clone()),
                Value::<CurrentNetwork>::from_str("500_000_000_000u64").unwrap(), // Use the upgrade limit
            ]
            .into_iter();
            vm.execute(&private_key, ("credits.aleo", "split"), inputs, None, 0, None, rng).unwrap()
        };
        // Create a split transaction before the migration.
        let split_transaction_2 = {
            let inputs = [
                Value::<CurrentNetwork>::Record(genesis_records[1].clone()),
                Value::<CurrentNetwork>::from_str("500_000_000_000u64").unwrap(), // Use the upgrade limit
            ]
            .into_iter();
            vm.execute(&private_key, ("credits.aleo", "split"), inputs, None, 0, None, rng).unwrap()
        };
        // Create a new block that includes the split.
        let next_block = crate::vm::test_helpers::sample_next_block(
            &vm,
            &private_key,
            &[split_transaction, split_transaction_2],
            rng,
        )
        .unwrap();
        vm.add_next_block(&next_block).unwrap();

        // Fetch the records from the new block.
        let split_records =
            next_block.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
        let split_records = split_records.values().map(|record| record.decrypt(&view_key).unwrap()).collect::<Vec<_>>();

        // ----------------------------------------------------------------------------------------
        // Construct blocks until migration occurs
        // ----------------------------------------------------------------------------------------

        while vm.block_store().current_block_height() <= CurrentNetwork::INCLUSION_UPGRADE_HEIGHT().unwrap() {
            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[], rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // ----------------------------------------------------------------------------------------
        // 1. Check that records that have not been upgraded can't be spent via calls
        // ----------------------------------------------------------------------------------------

        let inputs = [
            Value::<CurrentNetwork>::Record(split_records[0].clone()),
            Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
            Value::<CurrentNetwork>::from_str("1u64").unwrap(),
        ]
        .into_iter();
        assert!(
            vm.execute(&private_key, ("test_local_calls.aleo", "proxy_transfer"), inputs, None, 0, None, rng).is_err()
        );

        // ----------------------------------------------------------------------------------------
        // 2. Check that `credits.aleo/upgrade` can be invoked by a user.
        // ----------------------------------------------------------------------------------------

        // Upgrade an old record
        let inputs = [Value::<CurrentNetwork>::Record(split_records[0].clone())].into_iter();
        let upgrade = vm.execute(&private_key, ("credits.aleo", "upgrade"), inputs, None, 0, None, rng).unwrap();
        assert!(vm.check_transaction(&upgrade, None, rng).is_ok());

        // Add the upgrade function to a new block
        let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[upgrade], rng).unwrap();
        vm.add_next_block(&next_block).unwrap();
        assert_eq!(next_block.transactions().len(), 1);

        // Fetch the records from the new block.
        let upgraded_records =
            next_block.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
        let upgraded_records =
            upgraded_records.values().map(|record| record.decrypt(&view_key).unwrap()).collect::<Vec<_>>();

        // Check that the upgraded record can't be upgraded again.
        let record_to_spend = upgraded_records[0].clone();
        let inputs = [Value::<CurrentNetwork>::Record(record_to_spend)].into_iter();
        assert!(vm.execute(&private_key, ("credits.aleo", "upgrade"), inputs, None, 0, None, rng).is_err());

        // ----------------------------------------------------------------------------------------
        // 3. Check that local transfers cannot be invoked until the program is re-deployed.
        //    After the program is re-deployed, local transfers should work.
        // ----------------------------------------------------------------------------------------

        // Get the inputs.
        let inputs = [
            Value::<CurrentNetwork>::Record(upgraded_records[0].clone()),
            Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
            Value::<CurrentNetwork>::from_str("10u64").unwrap(),
        ];

        // Create a transaction with local transfers, which should fail, because the program has not been re-deployed.
        let local_transfer = vm
            .execute(
                &private_key,
                ("test_local_calls.aleo", "local_transfer"),
                inputs.clone().into_iter(),
                None,
                0,
                None,
                rng,
            )
            .unwrap();
        let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[local_transfer], rng).unwrap();
        assert_eq!(next_block.transactions().num_accepted(), 0);
        assert_eq!(next_block.transactions().num_rejected(), 0);
        assert_eq!(next_block.aborted_transaction_ids().len(), 1);
        vm.add_next_block(&next_block).unwrap();

        // Re-deploy the program.
        let deployment = vm.deploy(&private_key, &program, None, 1, None, rng).unwrap();
        let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[deployment], rng).unwrap();
        assert_eq!(next_block.transactions().num_accepted(), 1);
        assert_eq!(next_block.transactions().num_rejected(), 0);
        assert_eq!(next_block.aborted_transaction_ids().len(), 0);
        vm.add_next_block(&next_block).unwrap();

        // Execute the local transfer again.
        let local_transfer = vm
            .execute(&private_key, ("test_local_calls.aleo", "local_transfer"), inputs.into_iter(), None, 0, None, rng)
            .unwrap();
        assert!(vm.check_transaction(&local_transfer, None, rng).is_ok());
        let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[local_transfer], rng).unwrap();
        assert_eq!(next_block.transactions().num_accepted(), 1);
        assert_eq!(next_block.transactions().num_rejected(), 0);
        assert_eq!(next_block.aborted_transaction_ids().len(), 0);
    }

    #[cfg(feature = "test")]
    #[test]
    fn test_inclusion_for_custom_records() {
        // 1. Deploy a program with custom records
        // 2. Mint the records
        // 3. Check that the records can be spent prior to migration
        // 4. Construct blocks until migration occurs
        // 5. Check that the records are be spent after migration

        let rng = &mut TestRng::default();

        // Initialize the VM.
        let vm = crate::vm::test_helpers::sample_vm();
        // Initialize the genesis block.
        let genesis = crate::vm::test_helpers::sample_genesis_block(rng);
        // Update the VM.
        vm.add_next_block(&genesis).unwrap();

        // Fetch the private key.
        let private_key = crate::vm::test_helpers::sample_genesis_private_key(rng);
        let view_key = ViewKey::try_from(&private_key).unwrap();
        let address = Address::try_from(&private_key).unwrap();

        // ----------------------------------------------------------------------------------------
        // 1. Deploy a program with custom records
        // ----------------------------------------------------------------------------------------

        // Deploy the program.
        let program = Program::from_str(
            r"
program token.aleo;

record token:
    owner as address.private;
    amount as u64.private;

function mint:
    input r0 as address.private;
    input r1 as u64.private;
    cast r0 r1 into r2 as token.record;
    output r2 as token.record;

function transfer:
    input r0 as token.record;
    input r1 as address.private;
    input r2 as u64.private;
    sub r0.amount r2 into r3;
    cast r1 r2 into r4 as token.record;
    cast r0.owner r3 into r5 as token.record;
    output r4 as token.record;
    output r5 as token.record;
    ",
        )
        .unwrap();

        let deployment = vm.deploy(&private_key, &program, None, 1, None, rng).unwrap();
        vm.add_next_block(&crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[deployment], rng).unwrap())
            .unwrap();

        // ----------------------------------------------------------------------------------------
        // 2. Mint the records
        // ----------------------------------------------------------------------------------------

        let mint = {
            let inputs = [
                Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("100000u64").unwrap(),
            ]
            .into_iter();
            vm.execute(&private_key, ("token.aleo", "mint"), inputs, None, 0, None, rng).unwrap()
        };
        assert!(vm.check_transaction(&mint, None, rng).is_ok());

        let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[mint], rng).unwrap();
        vm.add_next_block(&next_block).unwrap();
        assert_eq!(next_block.transactions().len(), 1);

        // Fetch the records from the new block.
        let minted_records =
            next_block.transitions().cloned().flat_map(Transition::into_records).collect::<IndexMap<_, _>>();
        let minted_records =
            minted_records.values().map(|record| record.decrypt(&view_key).unwrap()).collect::<Vec<_>>();

        // ----------------------------------------------------------------------------------------
        // 3. Check that the records can be spent prior to migration
        // ----------------------------------------------------------------------------------------

        let transfer_1 = {
            let inputs = [
                Value::<CurrentNetwork>::Record(minted_records[0].clone()),
                Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("1000u64").unwrap(),
            ]
            .into_iter();
            vm.execute(&private_key, ("token.aleo", "transfer"), inputs, None, 0, None, rng).unwrap()
        };
        assert!(vm.check_transaction(&transfer_1, None, rng).is_ok());

        // ----------------------------------------------------------------------------------------
        // 4. Construct blocks until migration occurs
        // ----------------------------------------------------------------------------------------

        while vm.block_store().current_block_height() < CurrentNetwork::INCLUSION_UPGRADE_HEIGHT().unwrap() {
            // Call the function
            let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[], rng).unwrap();
            vm.add_next_block(&next_block).unwrap();
        }

        // ----------------------------------------------------------------------------------------
        // 5. Check that the records can be spent after migration
        // ----------------------------------------------------------------------------------------

        // Re-deploy the program.
        let deployment = vm.deploy(&private_key, &program, None, 1, None, rng).unwrap();
        let next_block = crate::vm::test_helpers::sample_next_block(&vm, &private_key, &[deployment], rng).unwrap();
        assert_eq!(next_block.transactions().num_accepted(), 1);
        assert_eq!(next_block.transactions().num_rejected(), 0);
        assert_eq!(next_block.aborted_transaction_ids().len(), 0);
        vm.add_next_block(&next_block).unwrap();

        let transfer_2 = {
            let inputs = [
                Value::<CurrentNetwork>::Record(minted_records[0].clone()),
                Value::<CurrentNetwork>::from_str(&address.to_string()).unwrap(),
                Value::<CurrentNetwork>::from_str("1000u64").unwrap(),
            ]
            .into_iter();
            vm.execute(&private_key, ("token.aleo", "transfer"), inputs, None, 0, None, rng).unwrap()
        };
        assert!(vm.check_transaction(&transfer_2, None, rng).is_ok());
    }
}
