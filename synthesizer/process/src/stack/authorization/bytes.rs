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

impl<N: Network> FromBytes for Authorization<N> {
    /// Reads the authorization from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid authorization version"));
        }

        // Read the number of requests.
        let num_requests = u8::read_le(&mut reader)?;
        // Ensure the number of requests is nonzero.
        if num_requests == 0 {
            return Err(error("Authorization (from 'read_le') has no requests"));
        }
        // Read the requests.
        let requests = (0..num_requests).map(|_| Request::read_le(&mut reader)).collect::<IoResult<Vec<_>>>()?;

        // Read the number of transitions.
        let num_transitions = u8::read_le(&mut reader)?;
        // Ensure the number of transitions is nonzero.
        if num_transitions == 0 {
            return Err(error("Authorization (from 'read_le') has no transitions"));
        }
        // Read the transitions.
        let transitions =
            (0..num_transitions).map(|_| Transition::read_le(&mut reader)).collect::<IoResult<Vec<_>>>()?;

        // Return the new `Authorization` instance.
        Self::try_from((requests, transitions)).map_err(error)
    }
}

impl<N: Network> ToBytes for Authorization<N> {
    /// Writes the authorization to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Acquire the read locks.
        let requests = self.requests.read();
        let transitions = self.transitions.read();

        // Write the version.
        1u8.write_le(&mut writer)?;
        // Write the number of requests.
        u8::try_from(requests.len()).map_err(error)?.write_le(&mut writer)?;
        // Write the requests.
        requests.iter().try_for_each(|request| request.write_le(&mut writer))?;
        // Write the number of transitions.
        u8::try_from(transitions.len()).map_err(error)?.write_le(&mut writer)?;
        // Write the transitions.
        transitions.values().try_for_each(|transition| transition.write_le(&mut writer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() -> Result<()> {
        let rng = &mut TestRng::default();

        // Construct a new authorization.
        let expected = crate::stack::authorization::test_helpers::sample_authorization(rng);

        // Check the byte representation.
        let expected_bytes = expected.to_bytes_le()?;
        assert_eq!(expected, Authorization::read_le(&expected_bytes[..])?);
        Ok(())
    }
}
