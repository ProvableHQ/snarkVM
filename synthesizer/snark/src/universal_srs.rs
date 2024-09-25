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

#[derive(Clone)]
pub struct UniversalSRS<N: Network> {
    /// The universal SRS parameter.
    srs: Arc<OnceCell<varuna::UniversalSRS<N::PairingCurve>>>,
}

impl<N: Network> UniversalSRS<N> {
    /// Initializes the universal SRS.
    pub fn load() -> Result<Self> {
        Ok(Self { srs: Arc::new(OnceCell::new()) })
    }

    /// Returns the circuit proving and verifying key.
    pub fn to_circuit_key(
        &self,
        function_name: &str,
        assignment: &circuit::Assignment<N::Field>,
    ) -> Result<(ProvingKey<N>, VerifyingKey<N>)> {
        #[cfg(feature = "aleo-cli")]
        let timer = std::time::Instant::now();

        let (proving_key, verifying_key) = Varuna::<N>::circuit_setup(self, assignment)?;

        #[cfg(feature = "aleo-cli")]
        println!("{}", format!(" • Built '{function_name}' (in {} ms)", timer.elapsed().as_millis()).dimmed());

        Ok((
            ProvingKey::new(Arc::new(proving_key)),
            VerifyingKey::new(Arc::new(verifying_key), assignment.num_variables()),
        ))
    }
}

impl<N: Network> FromBytes for UniversalSRS<N> {
    /// Reads the universal SRS from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        Ok(Self { srs: Arc::new(OnceCell::with_value(FromBytes::read_le(&mut reader)?)) })
    }
}

impl<N: Network> ToBytes for UniversalSRS<N> {
    /// Writes the universal SRS to a buffer.
    fn write_le<W: Write>(&self, writer: W) -> IoResult<()> {
        self.deref().write_le(writer)
    }
}

impl<N: Network> Deref for UniversalSRS<N> {
    type Target = varuna::UniversalSRS<N::PairingCurve>;

    #[allow(clippy::let_and_return)]
    fn deref(&self) -> &Self::Target {
        self.srs.get_or_init(|| {
            #[cfg(feature = "aleo-cli")]
            let timer = std::time::Instant::now();

            // Load the universal SRS.
            let universal_srs = varuna::UniversalSRS::load().expect("Failed to load the universal SRS");

            #[cfg(feature = "aleo-cli")]
            println!("{}", format!(" • Loaded universal setup (in {} ms)", timer.elapsed().as_millis()).dimmed());

            universal_srs
        })
    }
}
