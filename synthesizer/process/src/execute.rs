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

impl<N: Network> Process<N> {
    /// Executes the given authorization.
    #[inline]
    pub fn execute<A: circuit::Aleo<Network = N>, R: CryptoRng + Rng>(
        &self,
        authorization: Authorization<N>,
        rng: &mut R,
    ) -> Result<(Response<N>, Trace<N>)> {
        let timer = timer!("Process::execute");

        // Retrieve the main request (without popping it).
        let request = authorization.peek_next()?;
        // Construct the locator.
        let locator = Locator::new(*request.program_id(), *request.function_name());

        #[cfg(feature = "aleo-cli")]
        println!("{}", format!(" • Executing '{locator}'...",).dimmed());

        // This is the root request and does not have a caller.
        let caller = None;
        // This is the root request and we do not have a root_tvk to pass on.
        let root_tvk = None;
        // Initialize the trace.
        let trace = Arc::new(RwLock::new(Trace::new()));
        // Initialize the call stack.
        let call_stack = CallStack::execute(authorization, trace.clone())?;
        lap!(timer, "Initialize call stack");

        // Retrieve the stack.
        let stack = self.get_stack(request.program_id())?;
        // Execute the circuit.
        let response = stack.execute_function::<A, R>(call_stack, caller, root_tvk, rng)?;
        lap!(timer, "Execute the function");

        // Extract the trace.
        let trace = Arc::try_unwrap(trace).unwrap().into_inner();
        // Ensure the trace is not empty.
        ensure!(!trace.transitions().is_empty(), "Execution of '{locator}' is empty");

        finish!(timer);
        Ok((response, trace))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::types::Address;

    type CurrentNetwork = console::network::MainnetV0;
    type CurrentAleo = circuit::AleoV0;

    #[test]
    fn test_execute_fee_private() {
        let rng = &mut TestRng::default();

        // Initialize the process.
        let process = Process::<CurrentNetwork>::load_testing_only().unwrap();

        // Sample a private key.
        let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
        let owner = Address::try_from(private_key).unwrap();

        // Sample a base fee in microcredits.
        let base_fee_in_microcredits = rng.gen_range(1_000_000..u64::MAX / 2);
        // Sample a priority fee in microcredits.
        let priority_fee_in_microcredits = rng.gen_range(0..u64::MAX / 2);
        // Sample a deployment or execution ID.
        let deployment_or_execution_id = Field::rand(rng);

        // Sample a credits record.
        let fee_in_microcredits = base_fee_in_microcredits.saturating_add(priority_fee_in_microcredits);
        let credits = Record::<CurrentNetwork, Plaintext<_>>::from_str(&format!(
            "{{ owner: {owner}.private, microcredits: {fee_in_microcredits}u64.private, _nonce: 0group.public }}"
        ))
        .unwrap();

        // Initialize the authorization.
        let authorization = process
            .authorize_fee_private::<CurrentAleo, _>(
                &private_key,
                credits,
                base_fee_in_microcredits,
                priority_fee_in_microcredits,
                deployment_or_execution_id,
                rng,
            )
            .unwrap();
        assert!(authorization.is_fee_private(), "Authorization must be for a call to 'credits.aleo/fee_private'");

        // Execute the authorization.
        let (response, trace) = process.execute::<CurrentAleo, _>(authorization, rng).unwrap();
        // Ensure the response has 1 output.
        assert_eq!(response.outputs().len(), 1, "Execution of 'credits.aleo/fee_private' must contain 1 output");
        // Ensure the response has 1 output ID.
        assert_eq!(response.output_ids().len(), 1, "Execution of 'credits.aleo/fee_private' must contain 1 output ID");
        // Ensure the trace contains 1 transition.
        assert_eq!(trace.transitions().len(), 1, "Execution of 'credits.aleo/fee_private' must contain 1 transition");

        // Retrieve the transition.
        let transition = trace.transitions()[0].clone();
        assert!(transition.is_fee_private(), "Transition must be for 'credits.aleo/fee_private'");
    }

    #[test]
    fn test_execute_fee_public() {
        let rng = &mut TestRng::default();

        // Initialize the process.
        let process = Process::<CurrentNetwork>::load_testing_only().unwrap();

        // Sample a private key.
        let private_key = PrivateKey::new(rng).unwrap();
        // Sample a base fee in microcredits.
        let base_fee_in_microcredits = rng.gen_range(1_000_000..u64::MAX / 2);
        // Sample a priority fee in microcredits.
        let priority_fee_in_microcredits = rng.gen_range(0..u64::MAX / 2);
        // Sample a deployment or execution ID.
        let deployment_or_execution_id = Field::rand(rng);

        // Compute the authorization.
        let authorization = process
            .authorize_fee_public::<CurrentAleo, _>(
                &private_key,
                base_fee_in_microcredits,
                priority_fee_in_microcredits,
                deployment_or_execution_id,
                rng,
            )
            .unwrap();
        assert!(authorization.is_fee_public(), "Authorization must be for a call to 'credits.aleo/fee_public'");

        // Execute the authorization.
        let (response, trace) = process.execute::<CurrentAleo, _>(authorization, rng).unwrap();
        // Ensure the response has 1 outputs.
        assert_eq!(response.outputs().len(), 1, "Execution of 'credits.aleo/fee_public' must contain 1 output");
        // Ensure the response has 1 output IDs.
        assert_eq!(response.output_ids().len(), 1, "Execution of 'credits.aleo/fee_public' must contain 1 output ID");
        // Ensure the trace contains 1 transition.
        assert_eq!(trace.transitions().len(), 1, "Execution of 'credits.aleo/fee_public' must contain 1 transition");

        // Retrieve the transition.
        let transition = trace.transitions()[0].clone();
        assert!(transition.is_fee_public(), "Transition must be for 'credits.aleo/fee_public'");
    }
}
