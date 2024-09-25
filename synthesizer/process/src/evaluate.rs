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
    /// Evaluates a program function on the given request.
    #[inline]
    pub fn evaluate<A: circuit::Aleo<Network = N>>(&self, authorization: Authorization<N>) -> Result<Response<N>> {
        let timer = timer!("Process::evaluate");

        // Retrieve the top-level request (without popping it).
        let request = authorization.peek_next()?;

        #[cfg(feature = "aleo-cli")]
        println!("{}", format!(" • Evaluating '{}/{}'...", request.program_id(), request.function_name()).dimmed());

        // Retrieve the stack.
        let stack = self.get_stack(request.program_id())?;
        // Evaluate the function.
        let response = stack.evaluate_function::<A>(CallStack::evaluate(authorization)?, None);
        lap!(timer, "Evaluate the function");

        finish!(timer);

        response
    }
}
