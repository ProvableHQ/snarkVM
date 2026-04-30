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
use snarkvm_synthesizer_error::*;

impl<N: Network> Stack<N> {
    /// Authorizes a call to the program function for the given inputs.
    #[inline]
    pub fn authorize<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(
        &self,
        private_key: &PrivateKey<N>,
        function_name: impl TryInto<Identifier<N>>,
        inputs: impl ExactSizeIterator<Item = impl TryInto<Value<N>>>,
        rng: &mut R,
    ) -> Result<Authorization<N>, StackAuthError> {
        let timer = timer!("Stack::authorize");

        // Get the program ID.
        let program_id = *self.program.id();
        // Prepare the function name.
        let function_name = function_name.try_into().map_err(|_| anyhow!("Invalid function name"))?;
        // Retrieve the input types.
        let input_types = self.get_function(&function_name)?.input_types();
        lap!(timer, "Retrieve the input types");
        // Set is_root to true.
        let is_root = true;
        // Retrieve the program checksum, if the program has a constructor.
        let program_checksum = match self.program().contains_constructor() {
            true => Some(self.program_checksum_as_field()?),
            false => None,
        };

        // This is the root request and does not have a caller.
        let caller = None;
        // This is the root request and we do not have a root_tvk to pass on.
        let root_tvk = None;
        // Compute the request.
        let request = Request::sign(
            private_key,
            program_id,
            function_name,
            inputs,
            &input_types,
            root_tvk,
            is_root,
            program_checksum,
            false,
            rng,
        )?;
        lap!(timer, "Compute the request");
        // Initialize the authorization.
        let authorization = Authorization::new(request.clone());
        // Construct the call stack.
        let call_stack = CallStack::Authorize(vec![request], Some(*private_key), authorization.clone());
        // Construct the authorization from the function.
        let _response = self.execute_function::<A, R>(call_stack, caller, root_tvk, rng)?;
        finish!(timer, "Construct the authorization from the function");

        // Return the authorization.
        Ok(authorization)
    }

    /// Authorizes a call to the program function for the given inputs.
    /// Compared to `authorize`, this method does not check for circuit satisfiability of the request.
    #[inline]
    pub fn authorize_unchecked<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(
        &self,
        private_key: &PrivateKey<N>,
        function_name: impl TryInto<Identifier<N>>,
        inputs: impl ExactSizeIterator<Item = impl TryInto<Value<N>>>,
        rng: &mut R,
    ) -> Result<Authorization<N>, StackAuthError> {
        let timer = timer!("Stack::authorize_unchecked");

        // Get the program ID.
        let program_id = *self.program.id();
        // Prepare the function name.
        let function_name = function_name.try_into().map_err(|_| anyhow!("Invalid function name"))?;
        // Retrieve the input types.
        let input_types = self.get_function(&function_name)?.input_types();
        lap!(timer, "Retrieve the input types");
        // Set is_root to true.
        let is_root = true;

        // This is the root request and does not have a caller.
        let caller = None;
        // This is the root request and we do not have a root_tvk to pass on.
        let root_tvk = None;
        // Retrieve the program checksum, if the program has a constructor.
        let program_checksum = match self.program().contains_constructor() {
            true => Some(self.program_checksum_as_field()?),
            false => None,
        };
        // Compute the request.
        let request = Request::sign(
            private_key,
            program_id,
            function_name,
            inputs,
            &input_types,
            root_tvk,
            is_root,
            program_checksum,
            false,
            rng,
        )?;
        lap!(timer, "Compute the request");
        // Initialize the authorization.
        let authorization = Authorization::new(request.clone());
        // Construct the call stack.
        let call_stack = CallStack::Authorize(vec![request], Some(*private_key), authorization.clone());
        // Construct the authorization from the function.
        let _response = self.evaluate_function::<A, R>(call_stack, caller, root_tvk, rng)?;
        finish!(timer, "Construct the authorization from the function");

        // Return the authorization.
        Ok(authorization)
    }

    /// Produces a mocked `Authorization` for a call to the given function on
    /// the supplied inputs using the provided caller address. The resulting
    /// `Authorization` has the same size as the one which would be produced
    /// (and signed) using the private key corresponding to that address and can
    /// therefore be used to compute the cost of the associated `Execution`, but
    /// many of its values (such as the input IDs in the `Request`s) may not be
    /// correct. This method does not check circuit satisfiability or `Request`
    /// validity.
    #[inline]
    pub fn sample_authorization<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(
        &self,
        address: Address<A::Network>,
        program_id: ProgramID<A::Network>,
        function_name: Identifier<A::Network>,
        inputs: impl ExactSizeIterator<Item = impl TryInto<Value<A::Network>>>,
        rng: &mut R,
    ) -> Result<Authorization<N>, StackAuthError> {
        let timer = timer!("Stack::sample_authorization");

        if program_id != *self.program.id() {
            return Err(anyhow!("Program ID mismatch").into());
        }

        // Get the program ID.
        let program_id = *self.program.id();
        // Retrieve the input types.
        let input_types = self.get_function(&function_name)?.input_types();
        lap!(timer, "Retrieve the input types");

        // This is the root request and does not have a caller.
        let caller = None;
        // This is the root request and we do not have a root_tvk to pass on.
        let root_tvk = None;

        // Compute the mock request.
        let mocked_request = Request::sample(address, program_id, function_name, inputs, &input_types, false, rng)?;

        lap!(timer, "Compute the mocked request");
        // Initialize the authorization.
        let authorization = Authorization::new(mocked_request.clone());
        // Construct the call stack.
        let call_stack = CallStack::AuthorizeMocked(vec![mocked_request], address, authorization.clone());
        // Construct the authorization from the function.
        let _response = self.evaluate_function::<A, R>(call_stack, caller, root_tvk, rng)?;
        finish!(timer, "Construct the mocked authorization from the function");

        // Return the authorization.
        Ok(authorization)
    }

    /// Authorizes a call to a public function for the given request.
    /// Compared to `authorize`, no private key is needed, but this only works for single public requests.
    #[inline]
    pub fn authorize_request<A: circuit::Aleo<Network = N>, R: Rng + CryptoRng>(
        &self,
        request: Request<N>,
        rng: &mut R,
    ) -> Result<Authorization<N>, StackAuthError> {
        let timer = timer!("Stack::authorize_request");

        // Initialize the authorization.
        let authorization = Authorization::new(request.clone());
        // Construct the call stack.
        let call_stack = CallStack::Authorize(vec![request], None, authorization.clone());
        // This is the root request and does not have a caller.
        let caller = None;
        // This is the root request and we do not have a root_tvk to pass on.
        let root_tvk = None;
        // Construct the authorization from the function.
        let _response = self.evaluate_function::<A, R>(call_stack, caller, root_tvk, rng)?;
        finish!(timer, "Construct the authorization from the function");

        // Return the authorization.
        Ok(authorization)
    }
}
