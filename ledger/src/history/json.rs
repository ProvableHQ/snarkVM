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

//! Imports `credits.aleo` staking history from the JSON snapshots that data-snarkVM writes.
//!
//! For every block from 1 on, data-snarkVM writes
//! `{dir}/group-{height / 65535}/block-{height}/block-{height}-{name}.json`. For `name` in
//! [`JSON_MAPPINGS`], the file is a JSON array of `[key, value]` plaintext strings holding the
//! whole mapping at the end of the block. For [`STAKING_REWARDS`], it is a JSON object from each
//! staker to `[validator, reward in microcredits]`.

use super::*;

use indexmap::IndexMap;
use snarkvm_ledger_store::{
    FinalizeStorage,
    FinalizeStore,
    HistoricalMappingValue,
    HistoryEvent,
    helpers::memory::ConsensusMemory,
};
use std::{
    collections::{BTreeMap, HashMap},
    ops::RangeInclusive,
    path::{Path, PathBuf},
    time::Instant,
};

/// The `credits.aleo` mappings a JSON snapshot holds, by file name.
const JSON_MAPPINGS: [&str; 5] = ["bonded", "delegated", "metadata", "unbonding", "withdraw"];

/// The index of `bonded` in [`JSON_MAPPINGS`].
const BONDED: usize = 0;

/// The file name of a block's staking rewards.
const STAKING_REWARDS: &str = "staking_rewards";

/// Heights per `group-*` directory.
const HEIGHTS_PER_GROUP: u32 = u16::MAX as u32;

/// Heights read and written per import batch.
const JSON_BATCH_BLOCKS: u32 = 2_048;

/// The [`JSON_MAPPINGS`] at the end of one block, in that order, from key to value strings.
type Snapshot = [HashMap<String, String>; 5];

/// Each rewarded staker's validator and reward, as strings and microcredits.
type Rewards = BTreeMap<String, (String, u64)>;

impl<N: Network, C: ConsensusStorage<N>> Ledger<N, C> {
    /// Returns the history scope of a JSON import: the `credits.aleo` mappings in the snapshots.
    pub fn history_json_scope() -> Result<HistoryScope<N>> {
        let credits = ProgramID::from_str("credits.aleo")?;
        let mappings =
            JSON_MAPPINGS.iter().map(|name| Ok((credits, Identifier::from_str(name)?))).collect::<Result<_>>()?;
        Ok(HistoryScope { programs: IndexMap::new(), mappings })
    }

    /// Indexes history through the current tip from the JSON snapshots in `dir`, without
    /// replaying blocks.
    ///
    /// Records the scope of [`Self::history_json_scope`], and fails if history was recorded for
    /// another scope. Block 0 is indexed by finalizing the genesis block on a throwaway VM.
    /// Fails before indexing anything if the files of the tip are missing. Stops at the first
    /// missing or invalid file, and a later call continues there.
    pub fn import_history_json(&self, dir: &Path) -> Result<()> {
        ensure!(!self.record_history.load(Ordering::SeqCst), "History recording must be off to import JSON history");
        self.configure_history(Self::history_json_scope()?)?;
        let tip = self.latest_height();
        if self.history_synced_height() > tip {
            return Ok(());
        }
        // Block 0 has no files; it is indexed from a throwaway genesis finalize.
        for name in JSON_MAPPINGS.into_iter().chain([STAKING_REWARDS]).filter(|_| tip > 0) {
            let path = json_path(dir, tip, name);
            ensure!(path.is_file(), "The JSON history does not reach block {tip}: {} is missing", path.display());
        }

        let mut genesis = None;
        if self.history_synced_height() == 0 {
            genesis = Some(self.import_genesis_history_snapshot()?);
        }
        let start = self.history_synced_height();
        ensure!(start > 0, "Genesis history was not indexed; reset the history");
        if start > tip {
            return Ok(());
        }
        let genesis = match (start, genesis) {
            (1, Some(snapshot)) => Some(snapshot),
            (1, None) => Some(self.genesis_json_snapshot()?),
            _ => None,
        };
        info!("Importing history for blocks {start} to {tip} from {}", dir.display());

        let decoder = JsonDecoder::new()?;
        let mut window = (Instant::now(), start);
        for first in (start..=tip).step_by(JSON_BATCH_BLOCKS as usize) {
            let last = first.saturating_add(JSON_BATCH_BLOCKS - 1).min(tip);
            let blocks = decoder.read_blocks(dir, first..=last, genesis.as_ref())?;
            self.vm.finalize_store().import_history_events(blocks)?;
            self.vm.finalize_store().set_history_synced_height(last + 1)?;

            let elapsed = window.0.elapsed();
            if elapsed >= PROGRESS_INTERVAL || last == tip {
                let rate = f64::from(last + 1 - window.1) / elapsed.as_secs_f64();
                info!(
                    "Imported JSON history to block {last}/{tip} ({rate:.1} blocks/s, ETA {})",
                    format_eta(time_to_finish(tip - last, rate))
                );
                window = (Instant::now(), last + 1);
            }
        }
        Ok(())
    }

    /// Indexes the genesis block's history from a throwaway finalize of this ledger's genesis
    /// block.
    pub fn import_genesis_history(&self) -> Result<()> {
        self.import_genesis_history_snapshot().map(|_| ())
    }

    /// Indexes genesis history and returns the [`JSON_MAPPINGS`] after that block.
    fn import_genesis_history_snapshot(&self) -> Result<Snapshot> {
        ensure!(self.history_synced_height() == 0, "Genesis history is already indexed");
        let vm = self.genesis_history_vm(HistoryRecording::Events)?;
        let events = vm.finalize_store().history_events(0)?;
        ensure!(!events.is_empty(), "Genesis produced no history events");
        self.vm.finalize_store().import_history_events(vec![(0, events)])?;
        self.vm.finalize_store().set_history_synced_height(1)?;
        json_snapshot_from_store(vm.finalize_store())
    }

    /// Returns the [`JSON_MAPPINGS`] after the genesis block.
    fn genesis_json_snapshot(&self) -> Result<Snapshot> {
        json_snapshot_from_store(self.genesis_history_vm(HistoryRecording::Off)?.finalize_store())
    }

    /// Finalizes this ledger's genesis block on a throwaway in-memory VM.
    fn genesis_history_vm(&self, recording: HistoryRecording) -> Result<VM<N, ConsensusMemory<N>>> {
        let store = ConsensusStore::<N, ConsensusMemory<N>>::open(StorageMode::new_test(None))?;
        let vm = VM::from(store)?;
        vm.finalize_store().set_history_scope(self.vm.finalize_store().history_scope());
        vm.finalize_store().set_history_recording(recording);
        vm.replay_finalize(self.genesis_block.clone()).with_context(|| "Failed to finalize genesis for history")?;
        Ok(vm)
    }
}

/// Returns the [`JSON_MAPPINGS`] of `credits.aleo` in `store`.
fn json_snapshot_from_store<N: Network, F: FinalizeStorage<N>>(store: &FinalizeStore<N, F>) -> Result<Snapshot> {
    let credits = ProgramID::from_str("credits.aleo")?;
    let mut snapshot = Snapshot::default();
    for (entries, name) in snapshot.iter_mut().zip_eq(JSON_MAPPINGS) {
        let mapping = store.get_mapping_confirmed(credits, Identifier::from_str(name)?)?;
        *entries = mapping.into_iter().map(|(key, value)| (key.to_string(), value.to_string())).collect();
    }
    Ok(snapshot)
}

/// Turns JSON snapshots into history events.
struct JsonDecoder<N: Network> {
    /// The `credits.aleo` program ID.
    credits: ProgramID<N>,
    /// The [`JSON_MAPPINGS`] names, in that order.
    names: Vec<Identifier<N>>,
    /// Addresses decoded so far, by string, since decoding one checks a curve point.
    addresses: RwLock<HashMap<String, Address<N>>>,
}

impl<N: Network> JsonDecoder<N> {
    /// Returns a decoder with no cached addresses.
    fn new() -> Result<Self> {
        Ok(Self {
            credits: ProgramID::from_str("credits.aleo")?,
            names: JSON_MAPPINGS.into_iter().map(Identifier::from_str).collect::<Result<_>>()?,
            addresses: Default::default(),
        })
    }

    /// Returns the events of each height in `heights`, in height order.
    ///
    /// The heights are split into one contiguous part per thread. `genesis` is the state before
    /// block 1, and must be given when `heights` starts at 1.
    fn read_blocks(
        &self,
        dir: &Path,
        heights: RangeInclusive<u32>,
        genesis: Option<&Snapshot>,
    ) -> Result<Vec<(u32, Vec<HistoryEvent<N>>)>> {
        let (first, last) = (*heights.start(), *heights.end());
        let threads = u32::try_from(rayon::current_num_threads()).unwrap_or(u32::MAX).max(1);
        let part_len = (last - first + 1).div_ceil(threads);
        let parts = (first..=last).step_by(part_len as usize).map(|start| start..=(start + part_len - 1).min(last));
        let parts = cfg_into_iter!(parts.collect_vec())
            .map(|part| self.read_part(dir, part, genesis))
            .collect::<Result<Vec<_>>>()?;
        Ok(parts.into_iter().flatten().collect())
    }

    /// Returns the events of each height in `heights`, reading them in order.
    fn read_part(
        &self,
        dir: &Path,
        heights: RangeInclusive<u32>,
        genesis: Option<&Snapshot>,
    ) -> Result<Vec<(u32, Vec<HistoryEvent<N>>)>> {
        let first = *heights.start();
        let mut previous = match first {
            1 => genesis.cloned().ok_or_else(|| anyhow!("The state before block 1 is missing"))?,
            _ => read_snapshot(dir, first - 1)?,
        };
        let mut blocks = Vec::with_capacity(heights.clone().count());
        for height in heights {
            let current = read_snapshot(dir, height)?;
            let path = json_path(dir, height, STAKING_REWARDS);
            let rewards: Rewards = serde_json::from_slice(&read_file(&path)?)
                .with_context(|| format!("Failed to parse {}", path.display()))?;
            let events = self.block_events(&previous, &current, &rewards).with_context(|| format!("Block {height}"))?;
            blocks.push((height, events));
            previous = current;
        }
        Ok(blocks)
    }

    /// Returns the events of the block that turned `previous` into `current` and paid
    /// `rewards`.
    ///
    /// A rewarded staker's bond is read from their staking reward, so their `bonded` entry is not
    /// recorded.
    fn block_events(&self, previous: &Snapshot, current: &Snapshot, rewards: &Rewards) -> Result<Vec<HistoryEvent<N>>> {
        let mut events = Vec::with_capacity(rewards.len());
        for (staker, (validator, reward)) in rewards {
            let bond =
                current[BONDED].get(staker).ok_or_else(|| anyhow!("Staker {staker} is rewarded, but not bonded"))?;
            let (bond_validator, new_stake) = parse_bond_state(bond)?;
            ensure!(
                bond_validator == validator,
                "Staker {staker} is rewarded for validator {validator}, but bonded to {bond_validator}"
            );
            // data-snarkVM writes each staker's stake instead of a reward for a block that pays no
            // staking rewards, and a stake is never all reward.
            let reward = if *reward == new_stake { 0 } else { *reward };
            ensure!(
                reward < new_stake || reward == 0,
                "Staker {staker} is rewarded {reward}, above their stake {new_stake}"
            );
            events.push(HistoryEvent::Staking {
                staker: self.address(staker)?,
                validator: self.address(validator)?,
                reward,
                new_stake,
            });
        }
        for (index, (name, (previous, current))) in
            self.names.iter().zip_eq(previous.iter().zip_eq(current)).enumerate()
        {
            let event = |key: &str, value: HistoricalMappingValue<N>| -> Result<HistoryEvent<N>> {
                Ok(HistoryEvent::Mapping {
                    program_id: self.credits,
                    mapping_name: *name,
                    key: self.plaintext(key)?,
                    value: Box::new(value),
                })
            };
            for (key, value) in current {
                if previous.get(key) == Some(value) || (index == BONDED && rewards.contains_key(key)) {
                    continue;
                }
                events.push(event(key, HistoricalMappingValue::Present(Value::Plaintext(self.plaintext(value)?)))?);
            }
            for key in previous.keys().filter(|key| !current.contains_key(*key)) {
                events.push(event(key, HistoricalMappingValue::Absent)?);
            }
        }
        Ok(events)
    }

    /// Parses `text` as a plaintext, taking addresses from the cache.
    fn plaintext(&self, text: &str) -> Result<Plaintext<N>> {
        match text.starts_with("aleo1") {
            true => Ok(Plaintext::from(Literal::Address(self.address(text)?))),
            false => Plaintext::from_str(text),
        }
    }

    /// Parses `text` as an address, taking it from the cache when it was parsed before.
    fn address(&self, text: &str) -> Result<Address<N>> {
        if let Some(address) = self.addresses.read().get(text) {
            return Ok(*address);
        }
        let address = Address::from_str(text)?;
        self.addresses.write().insert(text.to_string(), address);
        Ok(address)
    }
}

/// Returns the path of the JSON file `name` of the block at `height`.
fn json_path(dir: &Path, height: u32, name: &str) -> PathBuf {
    dir.join(format!("group-{}", height / HEIGHTS_PER_GROUP))
        .join(format!("block-{height}"))
        .join(format!("block-{height}-{name}.json"))
}

/// Returns the contents of `path`.
fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))
}

/// Reads the [`JSON_MAPPINGS`] at the end of the block at `height`.
fn read_snapshot(dir: &Path, height: u32) -> Result<Snapshot> {
    let mut snapshot = Snapshot::default();
    for (entries, name) in snapshot.iter_mut().zip_eq(JSON_MAPPINGS) {
        let path = json_path(dir, height, name);
        let pairs: Vec<(String, String)> = serde_json::from_slice(&read_file(&path)?)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        *entries = pairs.into_iter().collect();
    }
    Ok(snapshot)
}

/// Returns the validator and microcredits of a `credits.aleo/bonded` value, as the value's
/// display writes them.
fn parse_bond_state(value: &str) -> Result<(&str, u64)> {
    let parse = || {
        let rest = value.strip_prefix("{\n  validator: ")?;
        let (validator, rest) = rest.split_once(",\n  microcredits: ")?;
        Some((validator, rest.strip_suffix("u64\n}")?.parse().ok()?))
    };
    parse().ok_or_else(|| anyhow!("Invalid bond state '{value}'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{CurrentLedger, CurrentNetwork, sample_ledger};
    use console::{account::PrivateKey, prelude::TestRng};
    use indexmap::IndexSet;
    use snarkvm_ledger_store::helpers::MapRead;

    /// Writes the JSON files data-snarkVM writes for `ledger`'s latest block.
    fn write_json_block(ledger: &CurrentLedger, dir: &Path) {
        let height = ledger.latest_height();
        let store = ledger.vm.finalize_store();
        let credits = ProgramID::from_str("credits.aleo").unwrap();
        std::fs::create_dir_all(json_path(dir, height, STAKING_REWARDS).parent().unwrap()).unwrap();
        let mut rewards = IndexMap::new();
        for name in JSON_MAPPINGS {
            let mapping = store.get_mapping_confirmed(credits, Identifier::from_str(name).unwrap()).unwrap();
            if name == JSON_MAPPINGS[BONDED] {
                for (key, _) in &mapping {
                    let Plaintext::Literal(Literal::Address(staker), _) = key else {
                        panic!("Unexpected staker {key}")
                    };
                    let (validator, reward, _) = store.get_staking_reward(*staker, height).unwrap().unwrap();
                    rewards.insert(*staker, (validator, reward));
                }
            }
            std::fs::write(json_path(dir, height, name), serde_json::to_string_pretty(&mapping).unwrap()).unwrap();
        }
        std::fs::write(json_path(dir, height, STAKING_REWARDS), serde_json::to_string_pretty(&rewards).unwrap())
            .unwrap();
    }

    /// Returns the keys currently in `ledger`'s [`JSON_MAPPINGS`].
    fn mapping_keys(ledger: &CurrentLedger) -> Vec<(Identifier<CurrentNetwork>, Plaintext<CurrentNetwork>)> {
        let credits = ProgramID::from_str("credits.aleo").unwrap();
        JSON_MAPPINGS
            .into_iter()
            .flat_map(|name| {
                let name = Identifier::from_str(name).unwrap();
                let mapping = ledger.vm.finalize_store().get_mapping_confirmed(credits, name).unwrap();
                mapping.into_iter().map(move |(key, _)| (name, key))
            })
            .collect()
    }

    /// Returns the value of each of `keys` at every indexed height, and every staking reward.
    fn json_history(
        ledger: &CurrentLedger,
        keys: &IndexSet<(Identifier<CurrentNetwork>, Plaintext<CurrentNetwork>)>,
    ) -> (Vec<String>, Vec<String>) {
        let store = ledger.vm.finalize_store();
        let credits = ProgramID::from_str("credits.aleo").unwrap();
        let mut values = Vec::new();
        for height in 0..ledger.history_synced_height() {
            for (name, key) in keys {
                let value = store.get_historical_mapping_value(credits, *name, key.clone(), height).unwrap();
                values.push(format!("{name}[{key}] at {height}: {:?}", value.map(|value| value.into_owned())));
            }
        }
        let mut rewards = store
            .staking_rewards_map()
            .iter_confirmed()
            .map(|(key, value)| format!("{key:?}: {value:?}"))
            .collect_vec();
        rewards.sort();
        (values, rewards)
    }

    #[test]
    fn test_import_history_json_matches_live_recording() {
        let rng = &mut TestRng::default();
        let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
        let address = Address::try_from(&private_key).unwrap();
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("snarkvm-history-json-{}-{nanos}", std::process::id()));

        // `live` records the JSON scope while blocks are added, and writes their JSON files.
        let live = sample_ledger(private_key, rng);
        let genesis = live.get_block(0).unwrap();
        live.reset_history().unwrap();
        live.import_history_json(&dir).unwrap();
        assert_eq!(live.history_synced_height(), 1);
        live.set_record_history(true);
        // `imported` adds the same blocks without recording.
        let imported = CurrentLedger::load(genesis, StorageMode::new_test(None)).unwrap();
        imported.set_record_history(false);
        imported.reset_history().unwrap();

        let advance = |transactions: Vec<Transaction<CurrentNetwork>>, rng: &mut TestRng| {
            let block =
                live.prepare_advance_to_next_beacon_block(&private_key, vec![], vec![], transactions, rng).unwrap();
            assert!(block.transactions().iter().all(|transaction| transaction.is_accepted()));
            live.advance_to_next_block(&block).unwrap();
            imported.advance_to_next_block(&block).unwrap();
            write_json_block(&live, &dir);
        };
        let execute = |caller: &PrivateKey<CurrentNetwork>, function: &str, inputs: &[String], rng: &mut TestRng| {
            let inputs = inputs.iter().map(|input| Value::<CurrentNetwork>::from_str(input).unwrap()).collect_vec();
            live.vm().execute(caller, ("credits.aleo", function), inputs.iter(), None, 0, None, rng).unwrap()
        };
        let mut keys = IndexSet::<_>::from_iter(mapping_keys(&live));
        advance(vec![], rng);
        // A delegator bonds to the genesis validator, then unbonds everything.
        let delegator_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
        let delegator = Address::try_from(&delegator_key).unwrap().to_string();
        let fund = execute(&private_key, "transfer_public", &[delegator.clone(), "30000000000u64".into()], rng);
        advance(vec![fund], rng);
        let bond = execute(
            &delegator_key,
            "bond_public",
            &[address.to_string(), delegator.clone(), "20000000000u64".into()],
            rng,
        );
        advance(vec![bond], rng);
        keys.extend(mapping_keys(&live));
        let unbond = execute(&delegator_key, "unbond_public", &[delegator.clone(), "20000000000u64".into()], rng);
        advance(vec![unbond], rng);
        keys.extend(mapping_keys(&live));
        advance(vec![], rng);
        let tip = live.latest_height();
        assert_eq!(live.history_synced_height(), tip + 1);
        let delegator = Plaintext::from_str(&delegator).unwrap();
        for name in ["bonded", "unbonding", "withdraw"] {
            assert!(keys.contains(&(Identifier::from_str(name).unwrap(), delegator.clone())), "{name}");
        }

        // Files that stop before the tip are refused before anything is indexed.
        let tip_dir = json_path(&dir, tip, STAKING_REWARDS).parent().unwrap().to_path_buf();
        let hidden = dir.join("hidden");
        std::fs::rename(&tip_dir, &hidden).unwrap();
        let error = imported.import_history_json(&dir).unwrap_err().to_string();
        assert!(error.contains(&format!("does not reach block {tip}")), "{error}");
        assert_eq!(imported.history_synced_height(), 0);
        std::fs::rename(&hidden, &tip_dir).unwrap();

        imported.import_history_json(&dir).unwrap();
        assert_eq!(imported.history_synced_height(), tip + 1);
        let expected = json_history(&live, &keys);
        assert!(!expected.1.is_empty());
        assert_eq!(json_history(&imported, &keys), expected);

        // Nothing is left to index.
        imported.import_history_json(&dir).unwrap();
        assert_eq!(imported.history_synced_height(), tip + 1);

        // Recording continues on the imported history, within the JSON scope.
        imported.set_record_history(true);
        advance(vec![], rng);
        keys.extend(mapping_keys(&live));
        assert_eq!(json_history(&imported, &keys), json_history(&live, &keys));
        let credits = ProgramID::from_str("credits.aleo").unwrap();
        assert!(imported.records_mapping_history_of(&credits, &Identifier::from_str("delegated").unwrap()));
        assert!(!imported.records_mapping_history_of(&credits, &Identifier::from_str("account").unwrap()));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_block_events() {
        let rng = &mut TestRng::default();
        let staker = Address::try_from(PrivateKey::<CurrentNetwork>::new(rng).unwrap()).unwrap();
        let validator = Address::try_from(PrivateKey::<CurrentNetwork>::new(rng).unwrap()).unwrap();
        let bond = |stake: u64| format!("{{\n  validator: {validator},\n  microcredits: {stake}u64\n}}");
        let decoder = JsonDecoder::<CurrentNetwork>::new().unwrap();
        let mapping = |name: &str, value| HistoryEvent::Mapping {
            program_id: decoder.credits,
            mapping_name: Identifier::from_str(name).unwrap(),
            key: Plaintext::from(Literal::Address(staker)),
            value: Box::new(value),
        };

        let mut previous = Snapshot::default();
        previous[BONDED].insert(staker.to_string(), bond(100));
        previous[4].insert(staker.to_string(), validator.to_string());
        let mut current = Snapshot::default();
        current[BONDED].insert(staker.to_string(), bond(100));

        // A reward equal to the stake is data-snarkVM's record of a block without staking rewards.
        let rewards = Rewards::from([(staker.to_string(), (validator.to_string(), 100))]);
        let events = decoder.block_events(&previous, &current, &rewards).unwrap();
        assert_eq!(events, vec![
            HistoryEvent::Staking { staker, validator, reward: 0, new_stake: 100 },
            mapping("withdraw", HistoricalMappingValue::Absent),
        ]);

        let rewards = Rewards::from([(staker.to_string(), (validator.to_string(), 101))]);
        let error = decoder.block_events(&previous, &current, &rewards).unwrap_err().to_string();
        assert!(error.contains("above their stake"), "{error}");
        let rewards = Rewards::from([(staker.to_string(), (staker.to_string(), 1))]);
        let error = decoder.block_events(&previous, &current, &rewards).unwrap_err().to_string();
        assert!(error.contains("but bonded to"), "{error}");

        // A bond change without a reward is recorded in `bonded`.
        current[BONDED].insert(staker.to_string(), bond(150));
        let events = decoder.block_events(&previous, &current, &Rewards::new()).unwrap();
        assert_eq!(events, vec![
            mapping("bonded", HistoricalMappingValue::Present(Value::from_str(&bond(150)).unwrap())),
            mapping("withdraw", HistoricalMappingValue::Absent),
        ]);
    }

    #[test]
    fn test_parse_bond_state_of_display() {
        let validator = "aleo1rhgdu77hgyqd3xjj8ucu3jj9r2krwz6mnzyd80gncr5fxcwlh5rsvzp9px";
        let value =
            Value::<CurrentNetwork>::from_str(&format!("{{ validator: {validator}, microcredits: 42u64 }}")).unwrap();
        assert_eq!(parse_bond_state(&value.to_string()).unwrap(), (validator, 42));
        assert!(parse_bond_state("{ validator: x, microcredits: 42u64 }").is_err());
        assert!(parse_bond_state(&format!("{{\n  validator: {validator},\n  microcredits: 42u32\n}}")).is_err());
    }

    #[test]
    fn test_json_path() {
        let path = json_path(Path::new("/history-0"), 65_536, "bonded");
        assert_eq!(path, Path::new("/history-0/group-1/block-65536/block-65536-bonded.json"));
    }

    /// Writes data-snarkVM-shaped JSON for `heights`, with a mainnet-sized staker set.
    ///
    /// Every height pays a small reward and grows each staker's bond, so the importer writes a
    /// staking row per staker and a delegated update per validator.
    fn write_dummy_json(dir: &Path, heights: RangeInclusive<u32>, rng: &mut TestRng) {
        const VALIDATORS: usize = 26;
        const DELEGATORS: usize = 423;
        let validators = (0..VALIDATORS)
            .map(|_| Address::try_from(PrivateKey::<CurrentNetwork>::new(rng).unwrap()).unwrap().to_string())
            .collect_vec();
        let stakers = validators
            .iter()
            .cloned()
            .chain(
                (0..DELEGATORS)
                    .map(|_| Address::try_from(PrivateKey::<CurrentNetwork>::new(rng).unwrap()).unwrap().to_string()),
            )
            .collect_vec();
        let validator_of = |index: usize| validators[index % VALIDATORS].as_str();
        let bond = |staker_index: usize, stake: u64| {
            format!("{{\n  validator: {},\n  microcredits: {stake}u64\n}}", validator_of(staker_index))
        };
        let metadata = serde_json::to_string_pretty(&[(
            "aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc",
            "26u32",
        )])
        .unwrap();
        let unbonding = "[]";
        let withdraw = serde_json::to_string_pretty(
            &stakers.iter().enumerate().map(|(index, staker)| (staker, validator_of(index))).collect_vec(),
        )
        .unwrap();

        cfg_into_iter!(heights.collect_vec()).for_each(|height| {
            std::fs::create_dir_all(json_path(dir, height, STAKING_REWARDS).parent().unwrap()).unwrap();
            let bonded = stakers
                .iter()
                .enumerate()
                .map(|(index, staker)| (staker, bond(index, 1_000_000_000 + u64::from(height))))
                .collect_vec();
            let delegated = validators
                .iter()
                .map(|validator| {
                    let bonded_to = 1 + DELEGATORS / VALIDATORS;
                    (validator, format!("{}u64", bonded_to as u64 * (1_000_000_000 + u64::from(height))))
                })
                .collect_vec();
            let rewards: BTreeMap<_, _> =
                stakers.iter().enumerate().map(|(index, staker)| (staker, (validator_of(index), 1_000u64))).collect();
            std::fs::write(json_path(dir, height, "bonded"), serde_json::to_string_pretty(&bonded).unwrap()).unwrap();
            std::fs::write(json_path(dir, height, "delegated"), serde_json::to_string_pretty(&delegated).unwrap())
                .unwrap();
            std::fs::write(json_path(dir, height, "metadata"), &metadata).unwrap();
            std::fs::write(json_path(dir, height, "unbonding"), unbonding).unwrap();
            std::fs::write(json_path(dir, height, "withdraw"), &withdraw).unwrap();
            std::fs::write(json_path(dir, height, STAKING_REWARDS), serde_json::to_string_pretty(&rewards).unwrap())
                .unwrap();
        });
    }

    /// Returns the total size of regular files under `dir`.
    fn dir_size(dir: &Path) -> u64 {
        fn walk(path: &Path) -> u64 {
            let Ok(entries) = std::fs::read_dir(path) else { return 0 };
            entries
                .flatten()
                .map(|entry| {
                    let path = entry.path();
                    let Ok(meta) = entry.metadata() else { return 0 };
                    if meta.is_dir() { walk(&path) } else { meta.len() }
                })
                .sum()
        }
        walk(dir)
    }

    /// Times a 1000-block dummy JSON import and prints a mainnet extrapolation.
    ///
    /// The ledger stays at genesis; the timed loop is the same decode-and-write path
    /// [`Ledger::import_history_json`] uses after block 0. Dummy snapshots hold 26 validators and
    /// 423 delegators, the staker counts of a recent mainnet tip.
    #[test]
    #[ignore]
    fn test_import_history_json_1000_dummy_blocks() {
        const BLOCKS: u32 = 1_000;
        const MAINNET_HEIGHT: u32 = 22_240_447;
        let rng = &mut TestRng::default();
        let private_key = PrivateKey::<CurrentNetwork>::new(rng).unwrap();
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("snarkvm-history-json-bench-{}-{nanos}", std::process::id()));
        struct RemoveDir(PathBuf);
        impl Drop for RemoveDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = RemoveDir(dir.clone());

        let ledger = sample_ledger(private_key, rng);
        ledger.set_record_history(false);
        ledger.reset_history().unwrap();
        ledger.import_history_json(&dir).unwrap();
        assert_eq!(ledger.history_synced_height(), 1);

        let wrote = Instant::now();
        write_dummy_json(&dir, 1..=BLOCKS, rng);
        println!("Wrote {BLOCKS} dummy JSON heights in {:.1}s", wrote.elapsed().as_secs_f64());
        let json_bytes = dir_size(&dir);
        println!(
            "JSON: {:.1} MB total ({:.1} KB/block); height 1 files: bonded {:.1} KB, staking_rewards {:.1} KB, delegated {:.1} KB, withdraw {:.1} KB, metadata {} B, unbonding {} B",
            json_bytes as f64 / 1e6,
            json_bytes as f64 / 1e3 / f64::from(BLOCKS),
            std::fs::metadata(json_path(&dir, 1, "bonded")).unwrap().len() as f64 / 1e3,
            std::fs::metadata(json_path(&dir, 1, STAKING_REWARDS)).unwrap().len() as f64 / 1e3,
            std::fs::metadata(json_path(&dir, 1, "delegated")).unwrap().len() as f64 / 1e3,
            std::fs::metadata(json_path(&dir, 1, "withdraw")).unwrap().len() as f64 / 1e3,
            std::fs::metadata(json_path(&dir, 1, "metadata")).unwrap().len(),
            std::fs::metadata(json_path(&dir, 1, "unbonding")).unwrap().len(),
        );

        let store_dir = aleo_std::aleo_ledger_dir(CurrentNetwork::ID, ledger.vm.finalize_store().storage_mode());
        let before = dir_size(&store_dir);

        let decoder = JsonDecoder::<CurrentNetwork>::new().unwrap();
        let genesis = ledger.genesis_json_snapshot().unwrap();
        let started = Instant::now();
        let mut events = 0usize;
        for first in (1..=BLOCKS).step_by(JSON_BATCH_BLOCKS as usize) {
            let last = first.saturating_add(JSON_BATCH_BLOCKS - 1).min(BLOCKS);
            let blocks = decoder.read_blocks(&dir, first..=last, Some(&genesis)).unwrap();
            events += blocks.iter().map(|(_, events)| events.len()).sum::<usize>();
            ledger.vm.finalize_store().import_history_events(blocks).unwrap();
            ledger.vm.finalize_store().set_history_synced_height(last + 1).unwrap();
        }
        let elapsed = started.elapsed();
        let seconds = elapsed.as_secs_f64();
        let rate = f64::from(BLOCKS) / seconds;
        let mainnet = Duration::from_secs_f64(f64::from(MAINNET_HEIGHT) / rate);
        println!(
            "Imported {BLOCKS} dummy JSON blocks in {seconds:.2}s ({rate:.1} blocks/s, {events} events, {:.0} events/block); mainnet {MAINNET_HEIGHT} at this rate is {}",
            events as f64 / f64::from(BLOCKS),
            format_eta(Some(mainnet)),
        );
        let after = dir_size(&store_dir);
        let history_bytes = after.saturating_sub(before);
        println!(
            "RocksDB history: {:.1} MB for {BLOCKS} blocks ({:.0} bytes/block, {:.1} bytes/event); scaled to mainnet {MAINNET_HEIGHT}: {:.1} GB. JSON scaled to mainnet: {:.1} GB ({:.0}x).",
            history_bytes as f64 / 1e6,
            history_bytes as f64 / f64::from(BLOCKS),
            history_bytes as f64 / events as f64,
            history_bytes as f64 / f64::from(BLOCKS) * f64::from(MAINNET_HEIGHT) / 1e9,
            json_bytes as f64 / f64::from(BLOCKS) * f64::from(MAINNET_HEIGHT) / 1e9,
            json_bytes as f64 / history_bytes.max(1) as f64,
        );
        assert_eq!(ledger.history_synced_height(), BLOCKS + 1);
        assert!(events >= (BLOCKS as usize) * 449);
    }
}
