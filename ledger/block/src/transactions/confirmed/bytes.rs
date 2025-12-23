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

use super::*;

impl<N: Network> FromBytes for ConfirmedTransaction<N> {
    /// Reads the confirmed transaction from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        fn read_finalize_ops<N: Network, R: Read>(
            mut reader: R,
            num_finalize: NumFinalizeSize,
        ) -> IoResult<Vec<FinalizeOperation<N>>> {
            // Ensure the number of finalize operations is within bounds.
            if num_finalize as usize > N::MAX_COMMANDS {
                return Err(error(format!(
                    "ConfirmedTransaction (from 'read_le') has too many finalize operations ({} > {})",
                    num_finalize,
                    N::MAX_COMMANDS
                )));
            }

            let n = num_finalize as usize;
            let mut finalize = Vec::with_capacity(n);
            for _ in 0..n {
                finalize.push(FinalizeOperation::<N>::read_le(&mut reader)?);
            }
            Ok(finalize)
        }

        let variant = u8::read_le(&mut reader)?;
        match variant {
            0 => {
                let index = u32::read_le(&mut reader)?;
                let transaction = Transaction::<N>::read_le(&mut reader)?;
                let num_finalize = NumFinalizeSize::read_le(&mut reader)?;
                let finalize = read_finalize_ops::<N, _>(&mut reader, num_finalize)?;
                Self::accepted_deploy(index, transaction, finalize).map_err(error)
            }
            1 => {
                let index = u32::read_le(&mut reader)?;
                let transaction = Transaction::<N>::read_le(&mut reader)?;
                let num_finalize = NumFinalizeSize::read_le(&mut reader)?;
                let finalize = read_finalize_ops::<N, _>(&mut reader, num_finalize)?;
                Self::accepted_execute(index, transaction, finalize).map_err(error)
            }
            2 => {
                let index = u32::read_le(&mut reader)?;
                let transaction = Transaction::<N>::read_le(&mut reader)?;
                let rejected = Rejected::<N>::read_le(&mut reader)?;
                let num_finalize = NumFinalizeSize::read_le(&mut reader)?;
                let finalize = read_finalize_ops::<N, _>(&mut reader, num_finalize)?;
                Self::rejected_deploy(index, transaction, rejected, finalize).map_err(error)
            }
            3 => {
                let index = u32::read_le(&mut reader)?;
                let transaction = Transaction::<N>::read_le(&mut reader)?;
                let rejected = Rejected::<N>::read_le(&mut reader)?;
                let num_finalize = NumFinalizeSize::read_le(&mut reader)?;
                let finalize = read_finalize_ops::<N, _>(&mut reader, num_finalize)?;
                Self::rejected_execute(index, transaction, rejected, finalize).map_err(error)
            }
            4.. => Err(error(format!("Failed to decode confirmed transaction variant {variant}"))),
        }
    }
}

impl<N: Network> ToBytes for ConfirmedTransaction<N> {
    /// Writes the confirmed transaction to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::AcceptedDeploy(index, transaction, finalize) => {
                // Write the variant.
                0u8.write_le(&mut writer)?;
                // Write the index.
                index.write_le(&mut writer)?;
                // Write the transaction.
                transaction.write_le(&mut writer)?;
                // Write the number of finalize operations.
                NumFinalizeSize::try_from(finalize.len()).map_err(error)?.write_le(&mut writer)?;
                // Write the finalize operations.
                finalize.iter().try_for_each(|finalize| finalize.write_le(&mut writer))
            }
            Self::AcceptedExecute(index, transaction, finalize) => {
                // Write the variant.
                1u8.write_le(&mut writer)?;
                // Write the index.
                index.write_le(&mut writer)?;
                // Write the transaction.
                transaction.write_le(&mut writer)?;
                // Write the number of finalize operations.
                NumFinalizeSize::try_from(finalize.len()).map_err(error)?.write_le(&mut writer)?;
                // Write the finalize operations.
                finalize.iter().try_for_each(|finalize| finalize.write_le(&mut writer))
            }
            Self::RejectedDeploy(index, transaction, rejected, finalize) => {
                // Write the variant.
                2u8.write_le(&mut writer)?;
                // Write the index.
                index.write_le(&mut writer)?;
                // Write the transaction.
                transaction.write_le(&mut writer)?;
                // Write the rejected deployment.
                rejected.write_le(&mut writer)?;
                // Write the number of finalize operations.
                NumFinalizeSize::try_from(finalize.len()).map_err(error)?.write_le(&mut writer)?;
                // Write the finalize operations.
                finalize.iter().try_for_each(|finalize| finalize.write_le(&mut writer))
            }
            Self::RejectedExecute(index, transaction, rejected, finalize) => {
                // Write the variant.
                3u8.write_le(&mut writer)?;
                // Write the index.
                index.write_le(&mut writer)?;
                // Write the transaction.
                transaction.write_le(&mut writer)?;
                // Write the rejected execution.
                rejected.write_le(&mut writer)?;
                // Write the number of finalize operations.
                NumFinalizeSize::try_from(finalize.len()).map_err(error)?.write_le(&mut writer)?;
                // Write the finalize operations.
                finalize.iter().try_for_each(|finalize| finalize.write_le(&mut writer))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;
    use once_cell::sync::Lazy;

    type CurrentNetwork = MainnetV0;

    // Heavy fixtures built once per test-binary.
    static SAMPLES: Lazy<Vec<ConfirmedTransaction<CurrentNetwork>>> =
        Lazy::new(crate::transactions::confirmed::test_helpers::sample_confirmed_transactions);

    // Precompute bytes once too.
    static SAMPLE_BYTES: Lazy<Vec<Vec<u8>>> =
        Lazy::new(|| SAMPLES.iter().map(|tx| tx.to_bytes_le().expect("to_bytes_le")).collect());

    fn sample_limit() -> usize {
        std::env::var("SNARKVM_BYTES_SAMPLES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(usize::MAX)
    }

    fn iter_bytes<'a>() -> impl Iterator<Item = (&'a ConfirmedTransaction<CurrentNetwork>, &'a [u8])> {
        let lim = sample_limit();
        SAMPLES.iter().zip(SAMPLE_BYTES.iter().map(|v| v.as_slice())).take(lim)
    }

    #[test]
    fn test_bytes() {
        for (expected, expected_bytes) in iter_bytes() {
            assert_eq!(*expected, ConfirmedTransaction::read_le(expected_bytes).unwrap());
        }
    }
}
