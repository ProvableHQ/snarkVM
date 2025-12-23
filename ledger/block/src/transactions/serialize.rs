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

impl<N: Network> Serialize for Transactions<N> {
    /// Serializes the transactions to a JSON-string or buffer.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => {
                let mut transactions = serializer.serialize_seq(Some(self.transactions.len()))?;
                for transaction in self.transactions.values() {
                    transactions.serialize_element(transaction)?;
                }
                transactions.end()
            }
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for Transactions<N> {
    /// Deserializes the transactions from a JSON-string or buffer.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                use core::marker::PhantomData;

                struct TransactionsDeserializer<N: Network>(PhantomData<N>);

                impl<'de, N: Network> Visitor<'de> for TransactionsDeserializer<N> {
                    type Value = Vec<ConfirmedTransaction<N>>;

                    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                        formatter.write_str("Vec<ConfirmedTransaction> sequence.")
                    }

                    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                        let mut transactions = Vec::new();
                        while let Some(transaction) = seq.next_element()? {
                            transactions.push(transaction);
                        }
                        Ok(transactions)
                    }
                }

                Ok(Self::from(&deserializer.deserialize_seq(TransactionsDeserializer(PhantomData))?))
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "transactions"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;
    use once_cell::sync::Lazy;

    type CurrentNetwork = MainnetV0;

    /// Default number of samples used by these tests.
    /// Override with SNARKVM_TX_SERDE_SAMPLES (e.g. 1, 2, 4).
    fn sample_limit() -> usize {
        std::env::var("SNARKVM_TX_SERDE_SAMPLES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(2)
    }

    /// If we explicitly want heavy coverage (slow), run with:
    /// SNARKVM_TX_SERDE_HEAVY=1
    fn heavy_mode() -> bool {
        std::env::var("SNARKVM_TX_SERDE_HEAVY").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false)
    }

    /// Small, curated fixtures that still cover:
    /// - accepted execute
    /// - rejected execute
    /// - accepted deploy (1 only)
    /// - rejected deploy (1 only)
    ///
    /// We build them once per test binary.
    static SMALL_SETS: Lazy<Vec<Transactions<CurrentNetwork>>> = Lazy::new(|| {
        let rng = &mut TestRng::fixed(123456789);

        let set_exec_only: Transactions<CurrentNetwork> = [
            crate::transactions::confirmed::test_helpers::sample_accepted_execute(0, true, rng),
            crate::transactions::confirmed::test_helpers::sample_accepted_execute(1, false, rng),
            crate::transactions::confirmed::test_helpers::sample_rejected_execute(2, true, rng),
            crate::transactions::confirmed::test_helpers::sample_rejected_execute(3, false, rng),
        ]
        .into_iter()
        .collect();

        let set_mixed_small: Transactions<CurrentNetwork> = [
            // one deploy + one rejected deploy to keep coverage without blowing up size
            crate::transactions::confirmed::test_helpers::sample_accepted_deploy(10, 1, 1, false, rng),
            crate::transactions::confirmed::test_helpers::sample_rejected_deploy(11, 1, 1, false, rng),
            // plus two executes
            crate::transactions::confirmed::test_helpers::sample_accepted_execute(12, true, rng),
            crate::transactions::confirmed::test_helpers::sample_rejected_execute(13, false, rng),
        ]
        .into_iter()
        .collect();

        vec![set_exec_only, set_mixed_small]
    });

    /// Optional heavy fixture: sample a full block’s transactions once.
    static HEAVY_BLOCK: Lazy<Transactions<CurrentNetwork>> = Lazy::new(|| {
        let rng = &mut TestRng::fixed(123456789);
        crate::transactions::test_helpers::sample_block_transactions(rng)
    });

    fn iter_sets<'a>() -> Box<dyn Iterator<Item = &'a Transactions<CurrentNetwork>> + 'a> {
        let lim = sample_limit();
        if heavy_mode() {
            Box::new(SMALL_SETS.iter().chain(std::iter::once(&*HEAVY_BLOCK)).take(lim.max(1)))
        } else {
            Box::new(SMALL_SETS.iter().take(lim))
        }
    }

    fn json_roundtrip<T>(value: &T)
    where
        T: Serialize + for<'a> Deserialize<'a> + PartialEq + Eq + core::fmt::Debug,
    {
        let s = serde_json::to_string(value).unwrap();
        let back = serde_json::from_str::<T>(&s).unwrap();
        assert_eq!(*value, back);
    }

    fn bincode_roundtrip<T>(value: &T)
    where
        T: Serialize + for<'a> Deserialize<'a> + PartialEq + Eq + core::fmt::Debug,
    {
        let bytes = bincode::serialize(value).unwrap();
        let back: T = bincode::deserialize(&bytes).unwrap();
        assert_eq!(*value, back);
    }

    fn tobytes_roundtrip<T>(value: &T)
    where
        T: ToBytes + FromBytes + PartialEq + Eq + core::fmt::Debug,
    {
        let bytes = value.to_bytes_le().unwrap();
        let back = T::read_le(&bytes[..]).unwrap();
        assert_eq!(*value, back);
    }

    fn bincode_payload_matches_tobytes<T>(value: &T)
    where
        T: Serialize + ToBytes,
    {
        let tobytes = value.to_bytes_le().unwrap();
        let bincode_bytes = bincode::serialize(value).unwrap();
        assert_eq!(&tobytes[..], &bincode_bytes[8..]);
    }

    #[test]
    fn test_serde_json() {
        for txs in iter_sets() {
            json_roundtrip(txs);
        }
    }

    #[test]
    fn test_bincode() {
        // bincode roundtrip only (fast).
        for txs in iter_sets() {
            bincode_roundtrip(txs);
        }

        for txs in iter_sets().take(1) {
            bincode_payload_matches_tobytes(txs);
        }
    }

    #[test]
    fn test_bytes() {
        for txs in iter_sets() {
            tobytes_roundtrip(txs);
        }
    }

    #[test]
    fn test_display_fromstr_smoke() {
        let mut it = iter_sets();
        if let Some(txs) = it.next() {
            let s = txs.to_string();
            let back = Transactions::<CurrentNetwork>::from_str(&s).unwrap();
            assert_eq!(*txs, back);
        }

        if heavy_mode() {
            // In heavy mode, also check one more set.
            if let Some(txs) = it.next() {
                let s = txs.to_string();
                let back = Transactions::<CurrentNetwork>::from_str(&s).unwrap();
                assert_eq!(*txs, back);
            }
        }
    }
}
