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

use crate::{
    helpers::memory::{MemoryMap, TransitionMemory},
    DeploymentStorage,
    DeploymentStore,
    ExecutionStorage,
    ExecutionStore,
    FeeStorage,
    FeeStore,
    TransactionStorage,
    TransactionType,
    TransitionStore,
};
use console::{
    prelude::*,
    program::{Identifier, ProgramID, ProgramOwner},
};
use synthesizer_program::Program;
use synthesizer_snark::{Certificate, Proof, VerifyingKey};

/// An in-memory transaction storage.
#[derive(Clone)]
pub struct TransactionMemory<N: Network> {
    /// The mapping of `transaction ID` to `transaction type`.
    id_map: MemoryMap<N::TransactionID, TransactionType>,
    /// The deployment store.
    deployment_store: DeploymentStore<N, DeploymentMemory<N>>,
    /// The execution store.
    execution_store: ExecutionStore<N, ExecutionMemory<N>>,
    /// The fee store.
    fee_store: FeeStore<N, FeeMemory<N>>,
}

#[rustfmt::skip]
impl<N: Network> TransactionStorage<N> for TransactionMemory<N> {
    type IDMap = MemoryMap<N::TransactionID, TransactionType>;
    type DeploymentStorage = DeploymentMemory<N>;
    type ExecutionStorage = ExecutionMemory<N>;
    type FeeStorage = FeeMemory<N>;
    type TransitionStorage = TransitionMemory<N>;

    /// Initializes the transaction storage.
    fn open(transition_store: TransitionStore<N, Self::TransitionStorage>) -> Result<Self> {
        // Initialize the fee store.
        let fee_store = FeeStore::<N, FeeMemory<N>>::open(transition_store)?;
        // Initialize the deployment store.
        let deployment_store = DeploymentStore::<N, DeploymentMemory<N>>::open(fee_store.clone())?;
        // Initialize the execution store.
        let execution_store = ExecutionStore::<N, ExecutionMemory<N>>::open(fee_store.clone())?;
        // Return the transaction storage.
        Ok(Self { id_map: MemoryMap::default(), deployment_store, execution_store, fee_store })
    }

    /// Returns the ID map.
    fn id_map(&self) -> &Self::IDMap {
        &self.id_map
    }

    /// Returns the deployment store.
    fn deployment_store(&self) -> &DeploymentStore<N, Self::DeploymentStorage> {
        &self.deployment_store
    }

    /// Returns the execution store.
    fn execution_store(&self) -> &ExecutionStore<N, Self::ExecutionStorage> {
        &self.execution_store
    }

    /// Returns the fee store.
    fn fee_store(&self) -> &FeeStore<N, Self::FeeStorage> {
        &self.fee_store
    }
}

/// An in-memory deployment storage.
#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct DeploymentMemory<N: Network> {
    /// The ID map.
    id_map: MemoryMap<N::TransactionID, ProgramID<N>>,
    /// The edition map.
    edition_map: MemoryMap<ProgramID<N>, u16>,
    /// The reverse ID map.
    reverse_id_map: MemoryMap<(ProgramID<N>, u16), N::TransactionID>,
    /// The owner map.
    owner_map: MemoryMap<(ProgramID<N>, u16), ProgramOwner<N>>,
    /// The program map.
    program_map: MemoryMap<(ProgramID<N>, u16), Program<N>>,
    /// The verifying key map.
    verifying_key_map: MemoryMap<(ProgramID<N>, Identifier<N>, u16), VerifyingKey<N>>,
    /// The certificate map.
    certificate_map: MemoryMap<(ProgramID<N>, Identifier<N>, u16), Certificate<N>>,
    /// The fee store.
    fee_store: FeeStore<N, FeeMemory<N>>,
}

#[rustfmt::skip]
impl<N: Network> DeploymentStorage<N> for DeploymentMemory<N> {
    type IDMap = MemoryMap<N::TransactionID, ProgramID<N>>;
    type EditionMap = MemoryMap<ProgramID<N>, u16>;
    type ReverseIDMap = MemoryMap<(ProgramID<N>, u16), N::TransactionID>;
    type OwnerMap = MemoryMap<(ProgramID<N>, u16), ProgramOwner<N>>;
    type ProgramMap = MemoryMap<(ProgramID<N>, u16), Program<N>>;
    type VerifyingKeyMap = MemoryMap<(ProgramID<N>, Identifier<N>, u16), VerifyingKey<N>>;
    type CertificateMap = MemoryMap<(ProgramID<N>, Identifier<N>, u16), Certificate<N>>;
    type FeeStorage = FeeMemory<N>;

    /// Initializes the deployment storage.
    fn open(fee_store: FeeStore<N, Self::FeeStorage>) -> Result<Self> {
        Ok(Self {
            id_map: MemoryMap::default(),
            edition_map: MemoryMap::default(),
            reverse_id_map: MemoryMap::default(),
            owner_map: MemoryMap::default(),
            program_map: MemoryMap::default(),
            verifying_key_map: MemoryMap::default(),
            certificate_map: MemoryMap::default(),
            fee_store,
        })
    }

    /// Returns the ID map.
    fn id_map(&self) -> &Self::IDMap {
        &self.id_map
    }

    /// Returns the edition map.
    fn edition_map(&self) -> &Self::EditionMap {
        &self.edition_map
    }

    /// Returns the reverse ID map.
    fn reverse_id_map(&self) -> &Self::ReverseIDMap {
        &self.reverse_id_map
    }

    /// Returns the owner map.
    fn owner_map(&self) -> &Self::OwnerMap {
        &self.owner_map
    }

    /// Returns the program map.
    fn program_map(&self) -> &Self::ProgramMap {
        &self.program_map
    }

    /// Returns the verifying key map.
    fn verifying_key_map(&self) -> &Self::VerifyingKeyMap {
        &self.verifying_key_map
    }

    /// Returns the certificate map.
    fn certificate_map(&self) -> &Self::CertificateMap {
        &self.certificate_map
    }

    /// Returns the fee store.
    fn fee_store(&self) -> &FeeStore<N, Self::FeeStorage> {
        &self.fee_store
    }
}

/// An in-memory execution storage.
#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct ExecutionMemory<N: Network> {
    /// The ID map.
    id_map: MemoryMap<N::TransactionID, (Vec<N::TransitionID>, bool)>,
    /// The reverse ID map.
    reverse_id_map: MemoryMap<N::TransitionID, N::TransactionID>,
    /// The inclusion map.
    inclusion_map: MemoryMap<N::TransactionID, (N::StateRoot, Option<Proof<N>>)>,
    /// The fee store.
    fee_store: FeeStore<N, FeeMemory<N>>,
}

#[rustfmt::skip]
impl<N: Network> ExecutionStorage<N> for ExecutionMemory<N> {
    type IDMap = MemoryMap<N::TransactionID, (Vec<N::TransitionID>, bool)>;
    type ReverseIDMap = MemoryMap<N::TransitionID, N::TransactionID>;
    type InclusionMap = MemoryMap<N::TransactionID, (N::StateRoot, Option<Proof<N>>)>;
    type FeeStorage = FeeMemory<N>;

    /// Initializes the execution storage.
    fn open(fee_store: FeeStore<N, Self::FeeStorage>) -> Result<Self> {
        Ok(Self {
            id_map: MemoryMap::default(),
            reverse_id_map: MemoryMap::default(),
            inclusion_map: MemoryMap::default(),
            fee_store
        })
    }

    /// Returns the ID map.
    fn id_map(&self) -> &Self::IDMap {
        &self.id_map
    }

    /// Returns the reverse ID map.
    fn reverse_id_map(&self) -> &Self::ReverseIDMap {
        &self.reverse_id_map
    }

    /// Returns the inclusion map.
    fn inclusion_map(&self) -> &Self::InclusionMap {
        &self.inclusion_map
    }

    /// Returns the fee store.
    fn fee_store(&self) -> &FeeStore<N, Self::FeeStorage> {
        &self.fee_store
    }
}

/// An in-memory fee storage.
#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct FeeMemory<N: Network> {
    /// The fee map.
    fee_map: MemoryMap<N::TransactionID, (N::TransitionID, N::StateRoot, Option<Proof<N>>)>,
    /// The reverse fee map.
    reverse_fee_map: MemoryMap<N::TransitionID, N::TransactionID>,
    /// The transition store.
    transition_store: TransitionStore<N, TransitionMemory<N>>,
}

#[rustfmt::skip]
impl<N: Network> FeeStorage<N> for FeeMemory<N> {
    type FeeMap = MemoryMap<N::TransactionID, (N::TransitionID, N::StateRoot, Option<Proof<N>>)>;
    type ReverseFeeMap = MemoryMap<N::TransitionID, N::TransactionID>;
    type TransitionStorage = TransitionMemory<N>;

    /// Initializes the fee storage.
    fn open(transition_store: TransitionStore<N, Self::TransitionStorage>) -> Result<Self> {
        Ok(Self {
            fee_map: MemoryMap::default(),
            reverse_fee_map: MemoryMap::default(),
            transition_store,
        })
    }

    /// Returns the fee map.
    fn fee_map(&self) -> &Self::FeeMap {
        &self.fee_map
    }

    /// Returns the reverse fee map.
    fn reverse_fee_map(&self) -> &Self::ReverseFeeMap {
        &self.reverse_fee_map
    }

    /// Returns the transition store.
    fn transition_store(&self) -> &TransitionStore<N, Self::TransitionStorage> {
        &self.transition_store
    }
}
