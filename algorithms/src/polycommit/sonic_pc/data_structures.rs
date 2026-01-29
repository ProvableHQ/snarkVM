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

use super::{LabeledPolynomial, PolynomialInfo};
use crate::{
    crypto_hash::sha256::sha256,
    fft::EvaluationDomain,
    polycommit::kzg10::{self, DegreeInfo},
    prelude::PCError,
};
use snarkvm_curves::PairingEngine;
use snarkvm_fields::{ConstraintFieldError, Field, PrimeField, ToConstraintField};
use snarkvm_parameters::mainnet::MAX_NUM_POWERS;
use snarkvm_utilities::{FromBytes, ToBytes, error, into_io_error, serialize::*};

use anyhow::{Result, anyhow, bail, ensure};
use hashbrown::HashMap;
use itertools::Itertools;
use std::{
    borrow::{Borrow, Cow},
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt,
    io,
    mem,
    ops::{AddAssign, MulAssign, SubAssign},
};

/// `UniversalParams` are the universal parameters for the KZG10 scheme.
pub type UniversalParams<E> = kzg10::UniversalParams<E>;

/// `Randomness` is the randomness for the KZG10 scheme.
pub type Randomness<E> = kzg10::KZGRandomness<E>;

/// `Commitment` is the commitment for the KZG10 scheme.
pub type Commitment<E> = kzg10::KZGCommitment<E>;

/// `CommitterKey` is used to commit to, and create evaluation proofs for, a
/// given polynomial.
#[derive(Debug)]
pub struct CommitterKey<E: PairingEngine> {
    /// The key used to commit to polynomials.
    pub powers_of_beta_g: Vec<E::G1Affine>,

    /// The key used to commit to polynomials in Lagrange basis.
    /// This is empty if `self` does not support lagrange bases
    pub lagrange_bases_at_beta_g: BTreeMap<usize, Vec<E::G1Affine>>,

    /// The key used to commit to hiding polynomials.
    pub powers_of_beta_times_gamma_g: Vec<E::G1Affine>,

    /// The powers used to commit to shifted polynomials.
    /// This is `None` if `self` does not support enforcing any degree bounds.
    pub shifted_powers_of_beta_g: Option<Vec<E::G1Affine>>,

    /// The powers used to commit to shifted hiding polynomials.
    /// This is `None` if `self` does not support enforcing any degree bounds.
    pub shifted_powers_of_beta_times_gamma_g: Option<BTreeMap<usize, Vec<E::G1Affine>>>,

    /// The degree bounds that are supported by `self`.
    /// Sorted in ascending order from smallest bound to largest bound.
    /// This is `None` if `self` does not support enforcing any degree bounds.
    pub enforced_degree_bounds: Option<Vec<usize>>,
}

impl<E: PairingEngine> Default for CommitterKey<E> {
    fn default() -> Self {
        Self {
            powers_of_beta_g: Vec::new(),
            lagrange_bases_at_beta_g: Default::default(),
            powers_of_beta_times_gamma_g: Vec::new(),
            shifted_powers_of_beta_g: None,
            shifted_powers_of_beta_times_gamma_g: None,
            enforced_degree_bounds: None,
        }
    }
}

impl<E: PairingEngine> FromBytes for CommitterKey<E> {
    fn read_le<R: Read>(mut reader: R) -> io::Result<Self> {
        // Deserialize `powers`.
        let powers_len: u32 = FromBytes::read_le(&mut reader)?;
        // Ensure the number of powers is within bounds.
        if powers_len as usize > MAX_NUM_POWERS {
            return Err(error(format!(
                "CommitterKey (from 'read_le') has too many points ({powers_len} > {MAX_NUM_POWERS})"
            )));
        }
        let mut powers_of_beta_g = Vec::with_capacity(powers_len as usize);
        for _ in 0..powers_len {
            let power: E::G1Affine = FromBytes::read_le(&mut reader)?;
            powers_of_beta_g.push(power);
        }

        // Deserialize `lagrange_basis_at_beta`.
        let lagrange_bases_at_beta_len: u32 = FromBytes::read_le(&mut reader)?;
        let mut lagrange_bases_at_beta_g = BTreeMap::new();
        for _ in 0..lagrange_bases_at_beta_len {
            let size: u32 = FromBytes::read_le(&mut reader)?;
            let mut basis = Vec::with_capacity(size as usize);
            for _ in 0..size {
                let power: E::G1Affine = FromBytes::read_le(&mut reader)?;
                basis.push(power);
            }
            lagrange_bases_at_beta_g.insert(size as usize, basis);
        }

        // Deserialize `powers_of_beta_times_gamma_g`.
        let powers_of_beta_times_gamma_g_len: u32 = FromBytes::read_le(&mut reader)?;
        let mut powers_of_beta_times_gamma_g = Vec::with_capacity(powers_of_beta_times_gamma_g_len as usize);
        for _ in 0..powers_of_beta_times_gamma_g_len {
            let powers_of_g: E::G1Affine = FromBytes::read_le(&mut reader)?;
            powers_of_beta_times_gamma_g.push(powers_of_g);
        }

        // Deserialize `shifted_powers_of_beta_g`.
        let has_shifted_powers_of_beta_g: bool = FromBytes::read_le(&mut reader)?;
        let shifted_powers_of_beta_g = match has_shifted_powers_of_beta_g {
            true => {
                let shifted_powers_len: u32 = FromBytes::read_le(&mut reader)?;
                let mut shifted_powers_of_beta_g = Vec::with_capacity(shifted_powers_len as usize);
                for _ in 0..shifted_powers_len {
                    let shifted_power: E::G1Affine = FromBytes::read_le(&mut reader)?;
                    shifted_powers_of_beta_g.push(shifted_power);
                }

                Some(shifted_powers_of_beta_g)
            }
            false => None,
        };

        // Deserialize `shifted_powers_of_beta_times_gamma_g`.
        let has_shifted_powers_of_beta_times_gamma_g: bool = FromBytes::read_le(&mut reader)?;
        let shifted_powers_of_beta_times_gamma_g = match has_shifted_powers_of_beta_times_gamma_g {
            true => {
                let mut shifted_powers_of_beta_times_gamma_g = BTreeMap::new();
                let shifted_powers_of_beta_times_gamma_g_num_elements: u32 = FromBytes::read_le(&mut reader)?;
                for _ in 0..shifted_powers_of_beta_times_gamma_g_num_elements {
                    let key: u32 = FromBytes::read_le(&mut reader)?;

                    let value_len: u32 = FromBytes::read_le(&mut reader)?;
                    let mut value = Vec::with_capacity(value_len as usize);
                    for _ in 0..value_len {
                        let val: E::G1Affine = FromBytes::read_le(&mut reader)?;
                        value.push(val);
                    }

                    shifted_powers_of_beta_times_gamma_g.insert(key as usize, value);
                }

                Some(shifted_powers_of_beta_times_gamma_g)
            }
            false => None,
        };

        // Deserialize `enforced_degree_bounds`.
        let has_enforced_degree_bounds: bool = FromBytes::read_le(&mut reader)?;
        let enforced_degree_bounds = match has_enforced_degree_bounds {
            true => {
                let enforced_degree_bounds_len: u32 = FromBytes::read_le(&mut reader)?;
                let mut enforced_degree_bounds = Vec::with_capacity(enforced_degree_bounds_len as usize);
                for _ in 0..enforced_degree_bounds_len {
                    let enforced_degree_bound: u32 = FromBytes::read_le(&mut reader)?;
                    enforced_degree_bounds.push(enforced_degree_bound as usize);
                }

                Some(enforced_degree_bounds)
            }
            false => None,
        };

        // Construct the hash of the group elements.
        let mut hash_input = powers_of_beta_g.to_bytes_le().map_err(|_| error("Could not serialize powers"))?;
        powers_of_beta_times_gamma_g
            .write_le(&mut hash_input)
            .map_err(|_| error("Could not serialize powers_of_beta_times_gamma_g"))?;

        if let Some(shifted_powers_of_beta_g) = &shifted_powers_of_beta_g {
            shifted_powers_of_beta_g
                .write_le(&mut hash_input)
                .map_err(|_| error("Could not serialize shifted_powers_of_beta_g"))?;
        }

        if let Some(shifted_powers_of_beta_times_gamma_g) = &shifted_powers_of_beta_times_gamma_g {
            for value in shifted_powers_of_beta_times_gamma_g.values() {
                value.write_le(&mut hash_input).map_err(|_| error("Could not serialize shifted_power_of_gamma_g"))?;
            }
        }

        // Deserialize `hash`.
        let hash = sha256(&hash_input);
        let expected_hash: [u8; 32] = FromBytes::read_le(&mut reader)?;

        // Enforce the group elements construct the expected hash.
        if expected_hash != hash {
            return Err(error("Mismatching group elements"));
        }

        Ok(Self {
            powers_of_beta_g,
            lagrange_bases_at_beta_g,
            powers_of_beta_times_gamma_g,
            shifted_powers_of_beta_g,
            shifted_powers_of_beta_times_gamma_g,
            enforced_degree_bounds,
        })
    }
}

impl<E: PairingEngine> ToBytes for CommitterKey<E> {
    fn write_le<W: Write>(&self, mut writer: W) -> io::Result<()> {
        // Serialize `powers_of_beta_g`.
        (self.powers_of_beta_g.len() as u32).write_le(&mut writer)?;
        for power in &self.powers_of_beta_g {
            power.write_le(&mut writer)?;
        }

        // Serialize `lagrange_bases_at_beta_g`.
        (self.lagrange_bases_at_beta_g.len() as u32).write_le(&mut writer)?;
        for (size, powers) in &self.lagrange_bases_at_beta_g {
            (*size as u32).write_le(&mut writer)?;
            for power in powers {
                power.write_le(&mut writer)?;
            }
        }

        // Serialize `powers_of_beta_times_gamma_g`.
        (self.powers_of_beta_times_gamma_g.len() as u32).write_le(&mut writer)?;
        for power_of_gamma_g in &self.powers_of_beta_times_gamma_g {
            power_of_gamma_g.write_le(&mut writer)?;
        }

        // Serialize `shifted_powers_of_beta_g`.
        self.shifted_powers_of_beta_g.is_some().write_le(&mut writer)?;
        if let Some(shifted_powers_of_beta_g) = &self.shifted_powers_of_beta_g {
            (shifted_powers_of_beta_g.len() as u32).write_le(&mut writer)?;
            for shifted_power in shifted_powers_of_beta_g {
                shifted_power.write_le(&mut writer)?;
            }
        }

        // Serialize `shifted_powers_of_beta_times_gamma_g`.
        self.shifted_powers_of_beta_times_gamma_g.is_some().write_le(&mut writer)?;
        if let Some(shifted_powers_of_beta_times_gamma_g) = &self.shifted_powers_of_beta_times_gamma_g {
            (shifted_powers_of_beta_times_gamma_g.len() as u32).write_le(&mut writer)?;
            for (key, shifted_powers_of_beta_g) in shifted_powers_of_beta_times_gamma_g {
                (*key as u32).write_le(&mut writer)?;
                (shifted_powers_of_beta_g.len() as u32).write_le(&mut writer)?;
                for shifted_power in shifted_powers_of_beta_g {
                    shifted_power.write_le(&mut writer)?;
                }
            }
        }

        // Serialize `enforced_degree_bounds`.
        self.enforced_degree_bounds.is_some().write_le(&mut writer)?;
        if let Some(enforced_degree_bounds) = &self.enforced_degree_bounds {
            (enforced_degree_bounds.len() as u32).write_le(&mut writer)?;
            for enforced_degree_bound in enforced_degree_bounds {
                (*enforced_degree_bound as u32).write_le(&mut writer)?;
            }
        }

        // Construct the hash of the group elements.
        let mut hash_input = self.powers_of_beta_g.to_bytes_le().map_err(|_| error("Could not serialize powers"))?;
        self.powers_of_beta_times_gamma_g
            .write_le(&mut hash_input)
            .map_err(|_| error("Could not serialize powers_of_beta_times_gamma_g"))?;

        if let Some(shifted_powers_of_beta_g) = &self.shifted_powers_of_beta_g {
            shifted_powers_of_beta_g
                .write_le(&mut hash_input)
                .map_err(|_| error("Could not serialize shifted_powers_of_beta_g"))?;
        }

        if let Some(shifted_powers_of_beta_times_gamma_g) = &self.shifted_powers_of_beta_times_gamma_g {
            for value in shifted_powers_of_beta_times_gamma_g.values() {
                value.write_le(&mut hash_input).map_err(|_| error("Could not serialize shifted_power_of_gamma_g"))?;
            }
        }

        // Serialize `hash`
        let hash = sha256(&hash_input);
        hash.write_le(&mut writer)
    }
}

impl<E: PairingEngine> CommitterKey<E> {
    /// Update the committer_key
    /// The `powers_of_beta_g` and `shifted_powers_of_beta_g` may grow or shrink
    /// as per the new supported_degree The `enforced_degree_bounds` and
    /// `shifted_powers_of_beta_times_gamma_g` will adjust to the new
    /// degree_bounds The `lagrange_bases_at_beta_g` will optionally adjust
    /// to the new degree_bounds The `powers_of_beta_times_gamma_g` is only
    /// dependent on the `hiding_bound` so doesn't change This only works if
    /// the SRS max_degree is fixed across specializations Note that this
    /// implementation is not atomic. If specialize fails halfway through,
    pub fn update(&mut self, srs: &UniversalParams<E>, degree_info: &DegreeInfo) -> Result<()> {
        let trim_time = start_timer!(|| "Trimming public parameters");

        // The maximum degree required by the circuit polynomials
        let supported_degree = degree_info.max_degree;
        // The maximum degree of the entire SRS
        let max_degree = srs.max_degree();
        // The hiding bound for the circuit polynomials
        let supported_hiding_bound = degree_info.hiding_bound;

        // Update shifted_powers_of_beta_g, shifted_powers_of_beta_times_gamma_g,
        // enforced_degree_bounds
        match degree_info.degree_bounds.as_ref() {
            None => {
                self.shifted_powers_of_beta_g = None;
                self.shifted_powers_of_beta_times_gamma_g = None;
                self.enforced_degree_bounds = None;
            }
            Some(enforced_degree_bounds) => {
                // Retrieve new highest degree bound
                let new_highest_degree_bound =
                    *enforced_degree_bounds.last().ok_or(anyhow!("No degree bound found"))?;
                ensure!(new_highest_degree_bound < supported_degree);

                // Retrieve current degree bounds and shifted powers
                let degree_bounds = self.enforced_degree_bounds.take().unwrap_or_default();
                let mut highest_degree_bound = degree_bounds.iter().copied().sorted().last().unwrap_or_default();
                let mut shifted_powers_of_beta_g = self.shifted_powers_of_beta_g.take().unwrap_or_default();
                let mut shifted_powers_of_beta_times_gamma_g =
                    self.shifted_powers_of_beta_times_gamma_g.take().unwrap_or_default();

                // We add 1 to any existing upper bound, in congruence with `max_degree + 1`
                // below. This is because the proof system assumes we need this
                // extra degree. This can optionally be refactored to ensure the
                // extra degree is already encoded in degree_bounds.
                if highest_degree_bound > 0 {
                    highest_degree_bound =
                        highest_degree_bound.checked_add(1).ok_or(error("Overflow highest_degree_bound"))?;
                }

                let shifted_ck_time = start_timer!(|| "Constructing `shifted_powers_of_beta_g`");
                match new_highest_degree_bound.cmp(&highest_degree_bound) {
                    Ordering::Greater => {
                        let lowest_shift_degree = max_degree - new_highest_degree_bound;
                        let highest_shift_degree = max_degree + 1 - highest_degree_bound;
                        let mut new_powers = srs.powers_of_beta_g(lowest_shift_degree, highest_shift_degree)?;
                        new_powers.extend_from_slice(&shifted_powers_of_beta_g);
                        shifted_powers_of_beta_g = new_powers;
                    }
                    Ordering::Equal => (), // No need to adjust shifted_powers_of_beta_g
                    Ordering::Less => {
                        let degree_diff = highest_degree_bound - new_highest_degree_bound;
                        shifted_powers_of_beta_g = shifted_powers_of_beta_g.drain(degree_diff - 1..).collect_vec();
                    }
                };
                self.shifted_powers_of_beta_g = Some(shifted_powers_of_beta_g);
                end_timer!(shifted_ck_time);

                // because we might batch prove circuits of different sizes, we consider each
                // required degree_bound individually
                let shifted_ck_time = start_timer!(|| "Constructing `shifted_powers_of_beta_times_gamma_g`");
                let mut new_shifted_powers_of_beta_times_gamma_g = BTreeMap::new();
                for degree_bound in enforced_degree_bounds {
                    match shifted_powers_of_beta_times_gamma_g.remove(degree_bound) {
                        Some(found_powers) => {
                            new_shifted_powers_of_beta_times_gamma_g.insert(*degree_bound, found_powers);
                        }
                        None => {
                            let shift_degree = max_degree - *degree_bound;
                            let mut powers_for_degree_bound =
                                Vec::with_capacity((max_degree + 2).saturating_sub(shift_degree));
                            for i in 0..=supported_hiding_bound
                                .checked_add(1)
                                .ok_or(anyhow!("Overflow supported_hiding_bound"))?
                            {
                                // We have an additional degree in `powers_of_beta_times_gamma_g` beyond
                                // `powers_of_beta_g`.
                                if shift_degree + i < max_degree + 2 {
                                    powers_for_degree_bound
                                        .push(srs.powers_of_beta_times_gamma_g()[&(shift_degree + i)]);
                                }
                            }
                            new_shifted_powers_of_beta_times_gamma_g.insert(*degree_bound, powers_for_degree_bound);
                        }
                    }
                }
                self.enforced_degree_bounds = Some(enforced_degree_bounds.iter().copied().collect_vec());
                self.shifted_powers_of_beta_times_gamma_g = Some(new_shifted_powers_of_beta_times_gamma_g);
                end_timer!(shifted_ck_time);
            }
        }

        // Update powers_of_beta_g
        let old_supported_degree = self.powers_of_beta_g.len();
        match supported_degree.cmp(&old_supported_degree) {
            Ordering::Greater => {
                let mut new_powers = srs.powers_of_beta_g(old_supported_degree, supported_degree)?;
                self.powers_of_beta_g.append(&mut new_powers);
            }
            Ordering::Equal => (), // No need to adjust powers_of_beta_g
            Ordering::Less => {
                let degree_difference = old_supported_degree - supported_degree;
                self.powers_of_beta_g.truncate(old_supported_degree - degree_difference);
            }
        }

        // Set powers_of_beta_times_gamma_g
        self.powers_of_beta_times_gamma_g =
            (0..=(supported_hiding_bound.checked_add(1).ok_or(anyhow!("Overflow supported_hiding_bound"))?))
                .map(|i| {
                    srs.powers_of_beta_times_gamma_g().get(&i).copied().ok_or(PCError::HidingBoundToolarge {
                        hiding_poly_degree: supported_hiding_bound,
                        num_powers: 0,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

        // Update lagrange_bases_at_beta_g
        let mut lagrange_bases_at_beta_g = BTreeMap::new();
        if let Some(supported_lagrange_sizes) = degree_info.lagrange_sizes.as_ref() {
            let mut old_lagrange_bases_at_beta_g = mem::take(&mut self.lagrange_bases_at_beta_g);
            for size in supported_lagrange_sizes {
                let domain = EvaluationDomain::new(*size).unwrap();
                match old_lagrange_bases_at_beta_g.remove(&domain.size()) {
                    Some(found_lagrange_bases) => {
                        lagrange_bases_at_beta_g.insert(domain.size(), found_lagrange_bases);
                    }
                    None => {
                        let lagrange_time = start_timer!(|| format!("Constructing `lagrange_bases` of size {size}"));
                        if !size.is_power_of_two() {
                            bail!("The Lagrange basis size ({size}) is not a power of two")
                        }
                        if *size > srs.max_degree() + 1 {
                            bail!(
                                "The Lagrange basis size ({size}) is larger than the supported degree ({})",
                                srs.max_degree() + 1
                            )
                        }
                        let lagrange_basis_at_beta_g = srs.lagrange_basis(domain)?;
                        assert!(lagrange_basis_at_beta_g.len().is_power_of_two());
                        lagrange_bases_at_beta_g.insert(domain.size(), lagrange_basis_at_beta_g);
                        end_timer!(lagrange_time);
                    }
                }
            }
            self.lagrange_bases_at_beta_g = lagrange_bases_at_beta_g;
        }
        end_timer!(trim_time);
        Ok(())
    }

    /// Obtain powers for the underlying KZG10 construction
    pub fn powers(&self) -> kzg10::Powers<E> {
        kzg10::Powers {
            powers_of_beta_g: self.powers_of_beta_g.as_slice().into(),
            powers_of_beta_times_gamma_g: self.powers_of_beta_times_gamma_g.as_slice().into(),
        }
    }

    /// Obtain powers for committing to shifted polynomials.
    pub fn shifted_powers_of_beta_g(&self, degree_bound: impl Into<Option<usize>>) -> Option<kzg10::Powers<E>> {
        match (&self.shifted_powers_of_beta_g, &self.shifted_powers_of_beta_times_gamma_g) {
            (Some(shifted_powers_of_beta_g), Some(shifted_powers_of_beta_times_gamma_g)) => {
                let max_bound = self.enforced_degree_bounds.as_ref().unwrap().last().unwrap();
                let (bound, powers_range) = if let Some(degree_bound) = degree_bound.into() {
                    assert!(self.enforced_degree_bounds.as_ref().unwrap().contains(&degree_bound));
                    (degree_bound, (max_bound - degree_bound)..)
                } else {
                    (*max_bound, 0..)
                };

                let ck = kzg10::Powers {
                    powers_of_beta_g: shifted_powers_of_beta_g[powers_range].into(),
                    powers_of_beta_times_gamma_g: shifted_powers_of_beta_times_gamma_g[&bound].clone().into(),
                };

                Some(ck)
            }

            (_, _) => None,
        }
    }

    /// Obtain elements of the SRS in the lagrange basis powers, for use with
    /// the underlying KZG10 construction.
    pub fn lagrange_basis(&self, domain: EvaluationDomain<E::Fr>) -> Option<kzg10::LagrangeBasis<E>> {
        self.lagrange_bases_at_beta_g.get(&domain.size()).map(|basis| kzg10::LagrangeBasis {
            lagrange_basis_at_beta_g: Cow::Borrowed(basis),
            powers_of_beta_times_gamma_g: Cow::Borrowed(&self.powers_of_beta_times_gamma_g),
            domain,
        })
    }
}

/// Evaluation proof at a query set.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, CanonicalSerialize, CanonicalDeserialize)]
pub struct BatchProof<E: PairingEngine>(pub(crate) Vec<kzg10::KZGProof<E>>);

impl<E: PairingEngine> BatchProof<E> {
    pub fn is_hiding(&self) -> bool {
        self.0.iter().any(|c| c.is_hiding())
    }
}

/// Labels a `LabeledPolynomial` or a `LabeledCommitment`.
pub type PolynomialLabel = String;

/// A commitment along with information about its degree bound (if any).
#[derive(Clone, Debug, CanonicalSerialize, PartialEq, Eq)]
pub struct LabeledCommitment<C: CanonicalSerialize + 'static> {
    label: PolynomialLabel,
    commitment: C,
    degree_bound: Option<usize>,
}

impl<F: Field, C: CanonicalSerialize + ToConstraintField<F>> ToConstraintField<F> for LabeledCommitment<C> {
    fn to_field_elements(&self) -> Result<Vec<F>, ConstraintFieldError> {
        self.commitment.to_field_elements()
    }
}

// NOTE: Serializing the LabeledCommitments struct is done by serializing
// _WITHOUT_ the labels or the degree bound. Deserialization is _NOT_ supported,
// and you should construct the struct via the `LabeledCommitment::new` method
// after deserializing the Commitment.
impl<C: CanonicalSerialize + ToBytes> ToBytes for LabeledCommitment<C> {
    fn write_le<W: Write>(&self, mut writer: W) -> io::Result<()> {
        CanonicalSerialize::serialize_compressed(&self.commitment, &mut writer)
            .map_err(|_| error("could not serialize struct"))
    }
}

impl<C: CanonicalSerialize> LabeledCommitment<C> {
    /// Instantiate a new polynomial_context.
    pub fn new(label: PolynomialLabel, commitment: C, degree_bound: Option<usize>) -> Self {
        Self { label, commitment, degree_bound }
    }

    pub fn new_with_info(info: &PolynomialInfo, commitment: C) -> Self {
        Self { label: info.label().to_string(), commitment, degree_bound: info.degree_bound() }
    }

    /// Return the label for `self`.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Retrieve the commitment from `self`.
    pub fn commitment(&self) -> &C {
        &self.commitment
    }

    /// Retrieve the degree bound in `self`.
    pub fn degree_bound(&self) -> Option<usize> {
        self.degree_bound
    }
}

/// A term in a linear combination.
#[derive(Hash, Ord, PartialOrd, Clone, Eq, PartialEq)]
pub enum LCTerm {
    /// The constant term representing `one`.
    One,
    /// Label for a polynomial.
    PolyLabel(String),
}

impl fmt::Debug for LCTerm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LCTerm::One => write!(f, "1"),
            LCTerm::PolyLabel(label) => write!(f, "{label}"),
        }
    }
}

impl LCTerm {
    /// Returns `true` if `self == LCTerm::One`
    #[inline]
    pub fn is_one(&self) -> bool {
        matches!(self, LCTerm::One)
    }
}

impl From<PolynomialLabel> for LCTerm {
    fn from(other: PolynomialLabel) -> Self {
        Self::PolyLabel(other)
    }
}

impl From<&'_ str> for LCTerm {
    fn from(other: &str) -> Self {
        Self::PolyLabel(other.into())
    }
}

impl core::convert::TryInto<PolynomialLabel> for LCTerm {
    type Error = ();

    fn try_into(self) -> Result<PolynomialLabel, ()> {
        match self {
            Self::One => Err(()),
            Self::PolyLabel(l) => Ok(l),
        }
    }
}

impl<'a> core::convert::TryInto<&'a PolynomialLabel> for &'a LCTerm {
    type Error = ();

    fn try_into(self) -> Result<&'a PolynomialLabel, ()> {
        match self {
            LCTerm::One => Err(()),
            LCTerm::PolyLabel(l) => Ok(l),
        }
    }
}

impl<B: Borrow<String>> PartialEq<B> for LCTerm {
    fn eq(&self, other: &B) -> bool {
        match self {
            Self::One => false,
            Self::PolyLabel(l) => l == other.borrow(),
        }
    }
}

/// A labeled linear combinations of polynomials.
#[derive(Clone, Debug)]
pub struct LinearCombination<F> {
    /// The label.
    pub label: String,
    /// The linear combination of `(poly_label, coeff)` pairs.
    pub terms: BTreeMap<LCTerm, F>,
}

#[allow(clippy::or_fun_call)]
impl<F: Field> LinearCombination<F> {
    /// Construct an empty labeled linear combination.
    pub fn empty(label: impl Into<String>) -> Self {
        Self { label: label.into(), terms: BTreeMap::new() }
    }

    /// Construct a new labeled linear combination.
    /// with the terms specified in `term`.
    pub fn new(label: impl Into<String>, _terms: impl IntoIterator<Item = (F, impl Into<LCTerm>)>) -> Self {
        let mut terms = BTreeMap::new();
        for (c, l) in _terms.into_iter().map(|(c, t)| (c, t.into())) {
            *terms.entry(l).or_insert(F::zero()) += c;
        }

        Self { label: label.into(), terms }
    }

    /// Returns the label of the linear combination.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns `true` if the linear combination has no terms.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Add a term to the linear combination.
    pub fn add(&mut self, c: F, t: impl Into<LCTerm>) -> &mut Self {
        let t = t.into();
        *self.terms.entry(t.clone()).or_insert(F::zero()) += c;
        if self.terms[&t].is_zero() {
            self.terms.remove(&t);
        }
        self
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&F, &LCTerm)> {
        self.terms.iter().map(|(t, c)| (c, t))
    }
}

impl<'a, F: Field> AddAssign<(F, &'a LinearCombination<F>)> for LinearCombination<F> {
    #[allow(clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, (coeff, other): (F, &'a LinearCombination<F>)) {
        for (t, c) in other.terms.iter() {
            self.add(coeff * c, t.clone());
        }
    }
}

impl<'a, F: Field> SubAssign<(F, &'a LinearCombination<F>)> for LinearCombination<F> {
    #[allow(clippy::suspicious_op_assign_impl)]
    fn sub_assign(&mut self, (coeff, other): (F, &'a LinearCombination<F>)) {
        for (t, c) in other.terms.iter() {
            self.add(-coeff * c, t.clone());
        }
    }
}

impl<'a, F: Field> AddAssign<&'a LinearCombination<F>> for LinearCombination<F> {
    fn add_assign(&mut self, other: &'a LinearCombination<F>) {
        for (t, c) in other.terms.iter() {
            self.add(*c, t.clone());
        }
    }
}

impl<'a, F: Field> SubAssign<&'a LinearCombination<F>> for LinearCombination<F> {
    fn sub_assign(&mut self, other: &'a LinearCombination<F>) {
        for (t, &c) in other.terms.iter() {
            self.add(-c, t.clone());
        }
    }
}

impl<F: Field> AddAssign<F> for LinearCombination<F> {
    fn add_assign(&mut self, coeff: F) {
        self.add(coeff, LCTerm::One);
    }
}

impl<F: Field> SubAssign<F> for LinearCombination<F> {
    fn sub_assign(&mut self, coeff: F) {
        self.add(-coeff, LCTerm::One);
    }
}

impl<F: Field> MulAssign<F> for LinearCombination<F> {
    fn mul_assign(&mut self, coeff: F) {
        self.terms.values_mut().for_each(|c| *c *= &coeff);
    }
}

/// `QuerySet` is the set of queries that are to be made to a set of labeled
/// polynomials/equations `p` that have previously been committed to. Each
/// element of a `QuerySet` is a `(label, query)` pair, where `label` is the
/// label of a polynomial in `p`, and `query` is the field element
/// that `p[label]` is to be queried at.
///
/// Added the third field: the point name.
pub type QuerySet<T> = BTreeSet<(String, (String, T))>;

/// `Evaluations` is the result of querying a set of labeled polynomials or
/// equations `p` at a `QuerySet` `Q`. It maps each element of `Q` to the
/// resulting evaluation. That is, if `(label, query)` is an element of `Q`,
/// then `evaluation.get((label, query))` should equal
/// `p[label].evaluate(query)`.
pub type Evaluations<F> = BTreeMap<(String, F), F>;

/// Evaluate the given polynomials at `query_set`.
pub fn evaluate_query_set<'a, F: PrimeField>(
    polys: impl IntoIterator<Item = &'a LabeledPolynomial<F>>,
    query_set: &QuerySet<F>,
) -> Evaluations<F> {
    let polys: HashMap<_, _> = polys.into_iter().map(|p| (p.label(), p)).collect();
    let mut evaluations = Evaluations::new();
    for (label, (_point_name, point)) in query_set {
        let poly = polys.get(label as &str).expect("polynomial in evaluated lc is not found");
        let eval = poly.evaluate(*point);
        evaluations.insert((label.clone(), *point), eval);
    }
    evaluations
}

/// A proof of satisfaction of linear combinations.
#[derive(Clone, Debug, PartialEq, Eq, CanonicalSerialize, CanonicalDeserialize)]
pub struct BatchLCProof<E: PairingEngine> {
    /// Evaluation proof.
    pub proof: BatchProof<E>,
}

impl<E: PairingEngine> BatchLCProof<E> {
    pub fn is_hiding(&self) -> bool {
        self.proof.is_hiding()
    }
}

impl<E: PairingEngine> FromBytes for BatchLCProof<E> {
    fn read_le<R: Read>(mut reader: R) -> io::Result<Self> {
        CanonicalDeserialize::deserialize_compressed(&mut reader)
            .map_err(|err| into_io_error(anyhow::Error::from(err).context("could not deserialize struct")))
    }

    fn read_le_unchecked<R: Read>(mut reader: R) -> io::Result<Self> {
        CanonicalDeserialize::deserialize_compressed_unchecked(&mut reader)
            .map_err(|err| into_io_error(anyhow::Error::from(err).context("could not deserialize struct")))
    }
}

impl<E: PairingEngine> ToBytes for BatchLCProof<E> {
    fn write_le<W: Write>(&self, mut writer: W) -> io::Result<()> {
        CanonicalSerialize::serialize_compressed(self, &mut writer)
            .map_err(|err| into_io_error(anyhow::Error::from(err).context("could not serialize struct")))
    }
}
