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

mod assignment;
pub use assignment::*;

#[cfg(test)]
pub mod tests;

use crate::Stack;

use circuit::{
    Inject,
    traits::ToGroup,
};

use console::{
    network::prelude::*,
    program::{DynamicRecord, Record, Plaintext, ProgramID, Identifier, RECORD_DATA_TREE_DEPTH},
    types::{Field, Group},
};
use snarkvm_ledger_block::Transition;
use snarkvm_synthesizer_program::{Function, Instruction};
use snarkvm_synthesizer_snark::VerifyingKey;

use std::collections::HashMap;
use std::marker::PhantomData;

// #[derive(Clone, Debug, Default)]
pub struct Translation<N: Network> { 
    _phantom: PhantomData<N>,
}

impl<N: Network> Translation<N> {
    /// Returns the verifier public inputs for the given call graph and transitions.
    pub fn prepare_verifier_inputs<'a>(
        translation_verifying_keys: &HashMap<(ProgramID<N>, Identifier<N>), VerifyingKey<N>>,
        transitions: &HashMap<N::TransitionID, (&Transition<N>, Function<N>)>,
        call_graph: &HashMap<N::TransitionID, Vec<N::TransitionID>>,
    ) -> Result<HashMap<VerifyingKey<N>, Vec<Vec<N::Field>>>> {
        // Determine the number of transitions.
        let num_transitions = transitions.len();

        // Initialize a vector for the batch verifier inputs.
        let mut batch_verifier_inputs = HashMap::new();

        let mut translation_count = 0;

        for (parent, children) in call_graph.iter() {
            let (parent_transition, parent_function) = transitions.get(parent).ok_or_else(|| bail!("Transition not found in the call graph")).unwrap(); // TODO: handle this error
            let parent_program_id = parent_transition.program_id();
            let parent_function_name = parent_transition.function_name();

            let call_instructions = parent_function.instructions().iter().filter(|instruction| {
                matches!(instruction, Instruction::Call(_) | Instruction::CallDynamic(_))
            }).collect::<Vec<_>>();

            ensure!(call_instructions.len() == children.len(), "The number of call instructions does not match the number of children");

            for (child, call_instruction) in children.iter().zip(call_instructions.iter()) {
                let (child_transition, child_function) = transitions.get(child).ok_or_else(|| bail!("Transition not found in the call graph")).unwrap(); // TODO: handle this error
                let child_program_id = child_transition.program_id();
                let child_function_name = child_transition.function_name();

                let Instruction::CallDynamic(dynamic_call) = call_instruction else {
                    // Only dynamic calls can invoke a translation from a dynamic to a static record.
                    continue;
                };

                // Determine if any record translation proofs are required.
                ensure!(dynamic_call.operand_types().len() == child_function.inputs().len(), "The number of call operands does not match the number of function inputs");
                for ((call_operand, call_operand_type), input) in dynamic_call.operands().iter().zip(dynamic_call.operand_types().iter()).zip(child_function.inputs().iter()) {
                    match (call_operand_type, input.value_type()) {
                        (ValueType::DynamicRecord, ValueType::Record(record_identifier)) => {
                            // Case 1: the dynamic call passes in a dynamic record which becomes a concrete record.
                            let verifying_key = translation_verifying_keys.get(&(*child_program_id, record_identifier)).ok_or_else(|| bail!("Translation verifying key not found for {}/{}", child_program_id, record_identifier)).unwrap(); // TODO: handle this error
                            let dynamic_record_fid = parent_function.name().to_field()?;
                            let dynamic_record_id = call_operand.record_id(); // TODO(dynamic_dispatch): this is a placeholder. How does the verifier obtain this...?
                            let static_record_id = *input.id();
                            let to_static_record = Field::<N>::one();
                            let translation_count_field = Field::<N>::from_bits_le(&translation_count.to_bits_le());
                            let mut verifier_inputs_for_vk = batch_verifier_inputs.get_mut(verifying_key).unwrap_or_default();
                            verifier_inputs_for_vk.push(vec![translation_count_field, dynamic_record_fid, dynamic_record_id, static_record_id, to_static_record]);
                            translation_count += 1;
                        }
                        (ValueType::Record(record_identifier), ValueType::DynamicRecord) => {
                            // Case 2: the dynamic call passes in a concrete record which becomes a dynamic record.
                            let verifying_key = translation_verifying_keys.get(&(*parent_program_id, record_identifier)).ok_or_else(|| bail!("Translation verifying key not found for {}/{}", parent_program_id, record_identifier)).unwrap(); // TODO: handle this error
                            let dynamic_record_fid = child_function.name().to_field()?;
                            let dynamic_record_id = *input.id();
                            let static_record_id = call_operand.record_id(); // TODO: how to get this...?
                            let to_static_record = Field::<N>::zero();
                            let translation_count_field = Field::<N>::from_bits_le(&translation_count.to_bits_le());
                            let mut verifier_inputs_for_vk = batch_verifier_inputs.get_mut(verifying_key).unwrap_or_default();
                            verifier_inputs_for_vk.push(vec![translation_count_field, dynamic_record_fid, dynamic_record_id, static_record_id, to_static_record]);
                            translation_count += 1;
                        }
                        _ => { } // No translation to perform.
                    }
                }
                ensure!(dynamic_call.destination_types().len() == child_function.outputs().len(), "The number of call destinations does not match the number of function outputs");
                for ((call_destination, call_destination_type), output) in dynamic_call.destinations().iter().zip(dynamic_call.destination_types().iter()).zip(child_function.outputs().iter()) {
                    match (call_destination_type, output.value_type()) {
                        (ValueType::Record(record_identifier), ValueType::DynamicRecord) => {
                            // Case 3: the dynamic call returns a dynamic record which becomes a concrete record.
                            let verifying_key = translation_verifying_keys.get(&(*parent_program_id, record_identifier)).ok_or_else(|| bail!("Translation verifying key not found for {}/{}", parent_program_id, record_identifier)).unwrap(); // TODO: handle this error
                            let dynamic_record_fid = child_function.name().to_field()?;
                            let dynamic_record_id = *output.id();
                            let static_record_id = call_destination.record_id(); // TODO: how to get this...?
                            let to_static_record = Field::<N>::one();
                            let translation_count_field = Field::<N>::from_bits_le(&translation_count.to_bits_le());
                            let mut verifier_inputs_for_vk = batch_verifier_inputs.get_mut(verifying_key).unwrap_or_default();
                            verifier_inputs_for_vk.push(vec![translation_count_field, dynamic_record_fid, dynamic_record_id, static_record_id, to_static_record]);
                            translation_count += 1;
                        }
                        (ValueType::DynamicRecord, ValueType::Record(record_identifier)) => {
                            // Case 4: the dynamic call returns a concrete record which becomes a dynamic record.
                            let verifying_key = translation_verifying_keys.get(&(*child_program_id, record_identifier)).ok_or_else(|| bail!("Translation verifying key not found for {}/{}", child_program_id, record_identifier)).unwrap(); // TODO: handle this error
                            let dynamic_record_fid = parent_function.name().to_field()?;
                            let dynamic_record_id = call_destination.record_id(); // TODO: how to get this...?
                            let static_record_id = *output.id();
                            let to_static_record = Field::<N>::zero();
                            let translation_count_field = Field::<N>::from_bits_le(&translation_count.to_bits_le());
                            let mut verifier_inputs_for_vk = batch_verifier_inputs.get_mut(verifying_key).unwrap_or_default();
                            verifier_inputs_for_vk.push(vec![translation_count_field, dynamic_record_fid, dynamic_record_id, static_record_id, to_static_record]);
                            translation_count += 1;
                        }
                        _ => { } // No translation to perform.
                    }
                }
            }
        }

        Ok(batch_verifier_inputs)
    }
}