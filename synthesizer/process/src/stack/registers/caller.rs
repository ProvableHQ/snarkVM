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

impl<N: Network, A: circuit::Aleo<Network = N>> RegistersSigner<N> for Registers<N, A> {
    /// Returns the transition signer.
    #[inline]
    fn signer(&self) -> Result<Address<N>> {
        self.signer.ok_or_else(|| anyhow!("Signer address (console) is not set in the registers."))
    }

    /// Sets the transition signer.
    #[inline]
    fn set_signer(&mut self, signer: Address<N>) {
        self.signer = Some(signer);
    }

    /// Returns the root transition view key.
    #[inline]
    fn root_tvk(&self) -> Result<Field<N>> {
        self.root_tvk.ok_or_else(|| anyhow!("Root tvk (console) is not set in the registers."))
    }

    /// Sets the root transition view key.
    #[inline]
    fn set_root_tvk(&mut self, root_tvk: Field<N>) {
        self.root_tvk = Some(root_tvk);
    }

    /// Returns the transition caller.
    #[inline]
    fn caller(&self) -> Result<Address<N>> {
        self.caller.ok_or_else(|| anyhow!("Caller address (console) is not set in the registers."))
    }

    /// Sets the transition caller.
    #[inline]
    fn set_caller(&mut self, caller: Address<N>) {
        self.caller = Some(caller);
    }

    /// Returns the transition view key.
    #[inline]
    fn tvk(&self) -> Result<Field<N>> {
        self.tvk.ok_or_else(|| anyhow!("Transition view key (console) is not set in the registers."))
    }

    /// Sets the transition view key.
    #[inline]
    fn set_tvk(&mut self, tvk: Field<N>) {
        self.tvk = Some(tvk);
    }
}

impl<N: Network, A: circuit::Aleo<Network = N>> RegistersSignerCircuit<N, A> for Registers<N, A> {
    /// Returns the transition signer, as a circuit.
    #[inline]
    fn signer_circuit(&self) -> Result<circuit::Address<A>> {
        self.signer_circuit.clone().ok_or_else(|| anyhow!("Signer address (circuit) is not set in the registers."))
    }

    /// Sets the transition signer, as a circuit.
    #[inline]
    fn set_signer_circuit(&mut self, signer_circuit: circuit::Address<A>) {
        self.signer_circuit = Some(signer_circuit);
    }

    /// Returns the root transition view key, as a circuit.
    #[inline]
    fn root_tvk_circuit(&self) -> Result<circuit::Field<A>> {
        self.root_tvk_circuit.clone().ok_or_else(|| anyhow!("Root tvk (circuit) is not set in the registers."))
    }

    /// Sets the root transition view key, as a circuit.
    #[inline]
    fn set_root_tvk_circuit(&mut self, root_tvk_circuit: circuit::Field<A>) {
        self.root_tvk_circuit = Some(root_tvk_circuit);
    }

    /// Returns the transition caller, as a circuit.
    #[inline]
    fn caller_circuit(&self) -> Result<circuit::Address<A>> {
        self.caller_circuit.clone().ok_or_else(|| anyhow!("Caller address (circuit) is not set in the registers."))
    }

    /// Sets the transition caller, as a circuit.
    #[inline]
    fn set_caller_circuit(&mut self, caller_circuit: circuit::Address<A>) {
        self.caller_circuit = Some(caller_circuit);
    }

    /// Returns the transition view key, as a circuit.
    #[inline]
    fn tvk_circuit(&self) -> Result<circuit::Field<A>> {
        self.tvk_circuit.clone().ok_or_else(|| anyhow!("Transition view key (circuit) is not set in the registers."))
    }

    /// Sets the transition view key, as a circuit.
    #[inline]
    fn set_tvk_circuit(&mut self, tvk_circuit: circuit::Field<A>) {
        self.tvk_circuit = Some(tvk_circuit);
    }
}
