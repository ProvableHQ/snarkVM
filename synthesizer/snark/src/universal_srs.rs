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

#[cfg(feature = "locktick")]
use locktick::parking_lot::RwLock;
#[cfg(not(feature = "locktick"))]
use parking_lot::RwLock;

#[derive(Clone, Debug)]
pub struct UniversalSRS<N: Network> {
    /// The universal SRS parameters.
    srs: Arc<OnceLock<varuna::UniversalSRS<N::PairingCurve>>>,
}

impl<N: Network> Default for UniversalSRS<N> {
    fn default() -> Self {
        Self { srs: Arc::new(OnceLock::new()) }
    }
}

impl<N: Network> UniversalSRS<N> {
    /// Initializes the universal SRS.
    pub fn load() -> Result<Self> {
        Ok(Self { srs: Arc::new(OnceLock::new()) })
    }

    /// Returns the circuit proving and verifying key.
    pub fn to_circuit_key(
        &self,
        universal_prover: &RwLock<UniversalProver<N>>,
        _function_name: &str,
        assignment: &circuit::Assignment<N::Field>,
    ) -> Result<(ProvingKey<N>, VerifyingKey<N>)> {
        #[cfg(feature = "dev-print")]
        let timer = std::time::Instant::now();

        // Locks the universal_prover.
        let (proving_key, verifying_key) =
            Varuna::<N>::circuit_setup(self.deref(), &mut universal_prover.write(), assignment)?;

        #[cfg(feature = "dev-print")]
        {
            let _elapsed = timer.elapsed().as_millis();
            dev_println!(" • Built '{_function_name}' (in {_elapsed} ms)");
        }

        Ok((
            ProvingKey::new(Arc::new(proving_key)),
            VerifyingKey::new(Arc::new(verifying_key), assignment.num_variables()),
        ))
    }
}

impl<N: Network> FromBytes for UniversalSRS<N> {
    /// Reads the universal SRS from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let lock = OnceLock::new();
        lock.set(FromBytes::read_le(&mut reader)?).unwrap();
        Ok(Self { srs: Arc::new(lock) })
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
            #[cfg(feature = "dev-print")]
            let timer = std::time::Instant::now();

            // Load the universal SRS.
            let universal_srs = varuna::UniversalSRS::load().expect("Failed to load the universal SRS");

            #[cfg(feature = "dev-print")]
            {
                let _elapsed = timer.elapsed().as_millis();
                dev_println!(" • Loaded universal setup (in {_elapsed} ms)");
            }

            universal_srs
        })
    }
}

#[derive(Debug)]
pub struct UniversalProver<N: Network> {
    /// The universal prover - trimmed from the SRS
    universal_prover: varuna::UniversalProver<N::PairingCurve>,
}

impl<N: Network> Default for UniversalProver<N> {
    fn default() -> Self {
        Self { universal_prover: varuna::UniversalProver::default() }
    }
}

impl<N: Network> UniversalProver<N> {
    /// Initializes the universal Prover.
    pub fn load() -> Result<Self> {
        Ok(Self { universal_prover: varuna::UniversalProver::default() })
    }
}

impl<N: Network> DerefMut for UniversalProver<N> {
    fn deref_mut(&mut self) -> &mut varuna::UniversalProver<N::PairingCurve> {
        &mut self.universal_prover
    }
}

impl<N: Network> Deref for UniversalProver<N> {
    type Target = varuna::UniversalProver<N::PairingCurve>;

    fn deref(&self) -> &Self::Target {
        &self.universal_prover
    }
}
