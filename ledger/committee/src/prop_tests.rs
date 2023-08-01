// Copyright (C) 2019-2023 Aleo Systems Inc.
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
use crate::{MAX_COMMITTEE_SIZE, MIN_STAKE};

use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

use anyhow::Result;
use proptest::{
    collection::{hash_set, SizeRange},
    prelude::{any, Arbitrary, BoxedStrategy, Just, Strategy},
    sample::size_range,
};
use rand::SeedableRng;
use test_strategy::proptest;

type CurrentNetwork = console::network::Testnet3;

#[derive(Debug, Clone)]
pub struct Validator {
    pub address: Address<CurrentNetwork>,
    pub stake: u64,
    pub is_open: bool,
}

impl Arbitrary for Validator {
    type Parameters = ();
    type Strategy = BoxedStrategy<Validator>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        any_valid_validator()
    }
}

impl PartialEq<Self> for Validator {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

impl Eq for Validator {}

impl Hash for Validator {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address.hash(state);
    }
}

fn to_committee((round, ValidatorSet(validators)): (u64, ValidatorSet)) -> Result<Committee<CurrentNetwork>> {
    Committee::new(round, validators.iter().map(|v| (v.address, (v.stake, v.is_open))).collect())
}

fn validator_set<T: Strategy<Value = Validator>>(
    element: T,
    size: impl Into<SizeRange>,
) -> impl Strategy<Value = ValidatorSet> {
    hash_set(element, size).prop_map(ValidatorSet)
}

// TODO remove the allow(dead_code)s once there's a fix in test-strategy crate
#[allow(dead_code)]
fn invalid_round_committee() -> BoxedStrategy<Result<Committee<CurrentNetwork>>> {
    (Just(0), validator_set(any_valid_validator(), size_range(4..=MAX_COMMITTEE_SIZE as usize)))
        .prop_map(to_committee)
        .boxed()
}

#[allow(dead_code)]
fn too_small_committee() -> BoxedStrategy<Result<Committee<CurrentNetwork>>> {
    (1u64.., validator_set(any_valid_validator(), 0..4)).prop_map(to_committee).boxed()
}

#[allow(dead_code)]
fn too_low_stake_committee() -> BoxedStrategy<Result<Committee<CurrentNetwork>>> {
    (1u64.., validator_set(invalid_stake_validator(), 4..=4)).prop_map(to_committee).boxed()
}

#[derive(Debug, Clone)]
pub struct CommitteeContext(pub Committee<CurrentNetwork>, pub ValidatorSet);

impl Default for CommitteeContext {
    fn default() -> Self {
        let validators = ValidatorSet::default();
        let committee = to_committee((u64::default(), validators.clone())).unwrap();
        Self(committee, validators)
    }
}

impl Arbitrary for CommitteeContext {
    type Parameters = ValidatorSet;
    type Strategy = BoxedStrategy<CommitteeContext>;

    fn arbitrary() -> Self::Strategy {
        any::<ValidatorSet>()
            .prop_map(|validators| CommitteeContext(to_committee((1, validators.clone())).unwrap(), validators))
            .boxed()
    }

    fn arbitrary_with(validator_set: Self::Parameters) -> Self::Strategy {
        Just(validator_set)
            .prop_map(|validators| CommitteeContext(to_committee((1, validators.clone())).unwrap(), validators))
            .boxed()
    }
}

#[derive(Debug, Clone)]
pub struct ValidatorSet(pub HashSet<Validator>);

impl Default for ValidatorSet {
    fn default() -> Self {
        ValidatorSet(
            (0..4u64)
                .map(|i| {
                    let rng = &mut rand_chacha::ChaChaRng::seed_from_u64(i);
                    Validator { address: Address::new(rng.gen()), stake: MIN_STAKE, is_open: false }
                })
                .collect(),
        )
    }
}

impl Arbitrary for ValidatorSet {
    type Parameters = ();
    type Strategy = BoxedStrategy<ValidatorSet>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        // use minimal validator set to speed up tests that require signing from the committee members
        validator_set(any_valid_validator(), size_range(4..=4usize)).boxed()
    }
}

pub fn any_valid_validator() -> BoxedStrategy<Validator> {
    (MIN_STAKE..5_000_000_000, any_valid_address(), any::<bool>())
        .prop_map(|(stake, address, is_open)| Validator { address, stake, is_open })
        .boxed()
}

#[allow(dead_code)]
fn invalid_stake_validator() -> BoxedStrategy<Validator> {
    (0..MIN_STAKE, any_valid_address(), any::<bool>())
        .prop_map(|(stake, address, is_open)| Validator { address, stake, is_open })
        .boxed()
}

pub fn any_valid_address() -> BoxedStrategy<Address<CurrentNetwork>> {
    any::<u64>()
        .prop_map(|seed| {
            let rng = &mut rand_chacha::ChaChaRng::seed_from_u64(seed);
            Address::new(rng.gen())
        })
        .boxed()
}

#[proptest]
fn committee_advance(input: CommitteeContext) {
    let CommitteeContext(committee, _) = input;

    let current_round = committee.starting_round();
    let current_members = committee.members();

    let committee = committee.to_next_round();
    assert_eq!(committee.starting_round(), current_round + 1);
    assert_eq!(committee.members(), current_members);
}

#[proptest]
fn committee_members(input: CommitteeContext) {
    let CommitteeContext(committee, ValidatorSet(validators)) = input;

    let mut total_stake = 0u64;
    for v in validators.iter() {
        total_stake += v.stake;
    }

    assert_eq!(committee.num_members(), validators.len());
    assert_eq!(committee.total_stake(), total_stake);
    for v in validators.iter() {
        let address = v.address;
        assert!(committee.is_committee_member(address));
        assert_eq!(committee.get_stake(address), v.stake);
    }
    let quorum_threshold = committee.quorum_threshold();
    let availability_threshold = committee.availability_threshold();
    // (2f + 1) + (f + 1) - 1 = 3f + 1 = N
    assert_eq!(quorum_threshold + availability_threshold - 1, total_stake);
}

#[proptest]
fn invalid_stakes(#[strategy(too_low_stake_committee())] committee: Result<Committee<CurrentNetwork>>) {
    assert!(committee.is_err());
    if let Err(err) = committee {
        assert_eq!(err.to_string().as_str(), "All members must have sufficient stake");
    }
}

#[proptest]
fn invalid_member_count(#[strategy(too_small_committee())] committee: Result<Committee<CurrentNetwork>>) {
    assert!(matches!(committee, Err(e) if e.to_string().as_str() == "Committee must have at least 4 members"))
}

#[proptest]
fn invalid_round(#[strategy(invalid_round_committee())] committee: Result<Committee<CurrentNetwork>>) {
    assert!(matches!(committee, Err(e) if e.to_string().as_str() == "Round must be nonzero"))
}