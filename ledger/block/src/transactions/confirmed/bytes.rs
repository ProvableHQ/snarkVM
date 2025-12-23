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
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        fn read_finalize_ops<N: Network, R: Read>(
            mut reader: R,
            num_finalize: NumFinalizeSize,
        ) -> IoResult<Vec<FinalizeOperation<N>>> {
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

                // Basic shape check only.
                if !matches!(transaction, Transaction::Deploy(..)) {
                    return Err(error("AcceptedDeploy must contain a deploy transaction".to_string()));
                }
                Ok(Self::AcceptedDeploy(index, transaction, finalize))
            }
            1 => {
                let index = u32::read_le(&mut reader)?;
                let transaction = Transaction::<N>::read_le(&mut reader)?;
                let num_finalize = NumFinalizeSize::read_le(&mut reader)?;
                let finalize = read_finalize_ops::<N, _>(&mut reader, num_finalize)?;

                if !matches!(transaction, Transaction::Execute(..)) {
                    return Err(error("AcceptedExecute must contain an execute transaction".to_string()));
                }
                Ok(Self::AcceptedExecute(index, transaction, finalize))
            }
            2 => {
                let index = u32::read_le(&mut reader)?;
                let transaction = Transaction::<N>::read_le(&mut reader)?;
                let rejected = Rejected::<N>::read_le(&mut reader)?;
                let num_finalize = NumFinalizeSize::read_le(&mut reader)?;
                let finalize = read_finalize_ops::<N, _>(&mut reader, num_finalize)?;

                // Rejected variants carry the *fee* transaction.
                if !transaction.is_fee() {
                    return Err(error("RejectedDeploy must contain a fee transaction".to_string()));
                }
                if !rejected.is_deployment() {
                    return Err(error("RejectedDeploy must contain a rejected deployment".to_string()));
                }
                Ok(Self::RejectedDeploy(index, transaction, rejected, finalize))
            }
            3 => {
                let index = u32::read_le(&mut reader)?;
                let transaction = Transaction::<N>::read_le(&mut reader)?;
                let rejected = Rejected::<N>::read_le(&mut reader)?;
                let num_finalize = NumFinalizeSize::read_le(&mut reader)?;
                let finalize = read_finalize_ops::<N, _>(&mut reader, num_finalize)?;

                if !transaction.is_fee() {
                    return Err(error("RejectedExecute must contain a fee transaction".to_string()));
                }
                if !rejected.is_execution() {
                    return Err(error("RejectedExecute must contain a rejected execution".to_string()));
                }
                Ok(Self::RejectedExecute(index, transaction, rejected, finalize))
            }
            _ => Err(error(format!("Failed to decode confirmed transaction variant {variant}"))),
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

    fn heavy_mode() -> bool {
        std::env::var("SNARKVM_BYTES_HEAVY").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false)
    }

    fn sample_limit() -> usize {
        std::env::var("SNARKVM_BYTES_SAMPLES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(usize::MAX)
    }

    static FAST_SAMPLES: Lazy<Vec<ConfirmedTransaction<CurrentNetwork>>> = Lazy::new(|| {
        let rng = &mut TestRng::fixed(123456789);

        let accepted_exec_fee_priv =
            crate::transactions::confirmed::test_helpers::sample_accepted_execute(0, true, rng);
        let accepted_exec_fee_pub =
            crate::transactions::confirmed::test_helpers::sample_accepted_execute(1, false, rng);

        let rejected_exec_fee_priv =
            crate::transactions::confirmed::test_helpers::sample_rejected_execute(2, true, rng);
        let rejected_exec_fee_pub =
            crate::transactions::confirmed::test_helpers::sample_rejected_execute(3, false, rng);

        vec![accepted_exec_fee_priv, accepted_exec_fee_pub, rejected_exec_fee_priv, rejected_exec_fee_pub]
    });

    /// Optional heavy fixtures (Deploy / RejectedDeploy).
    static HEAVY_SAMPLES: Lazy<Vec<ConfirmedTransaction<CurrentNetwork>>> =
        Lazy::new(crate::transactions::confirmed::test_helpers::sample_confirmed_transactions);

    static FAST_BYTES: Lazy<Vec<Vec<u8>>> =
        Lazy::new(|| FAST_SAMPLES.iter().map(|tx| tx.to_bytes_le().expect("to_bytes_le")).collect());

    static HEAVY_BYTES: Lazy<Vec<Vec<u8>>> =
        Lazy::new(|| HEAVY_SAMPLES.iter().map(|tx| tx.to_bytes_le().expect("to_bytes_le")).collect());

    fn iter_cases<'a>() -> Box<dyn Iterator<Item = (&'a ConfirmedTransaction<CurrentNetwork>, &'a [u8])> + 'a> {
        let lim = sample_limit();

        if heavy_mode() {
            Box::new(HEAVY_SAMPLES.iter().zip(HEAVY_BYTES.iter().map(|v| v.as_slice())).take(lim))
        } else {
            Box::new(FAST_SAMPLES.iter().zip(FAST_BYTES.iter().map(|v| v.as_slice())).take(lim))
        }
    }

    #[test]
    fn test_bytes() {
        for (expected, bytes) in iter_cases() {
            assert_eq!(*expected, ConfirmedTransaction::read_le(bytes).unwrap());
        }
    }
}
