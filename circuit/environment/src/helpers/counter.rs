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

use crate::*;
use snarkvm_fields::PrimeField;

use std::{mem, rc::Rc};

#[derive(Debug, Default)]
pub(crate) struct Counter<F: PrimeField> {
    scope: Scope,
    constraints: Vec<Rc<Constraint<F>>>,
    constants: u64,
    public: u64,
    private: u64,
    nonzeros: (u64, u64, u64),
    parents: Vec<(Scope, Vec<Rc<Constraint<F>>>, u64, u64, u64, (u64, u64, u64))>,
}

impl<F: PrimeField> Counter<F> {
    /// Saves and switches from the current scope to a new scope.
    pub(crate) fn push<S: Into<String>>(&mut self, name: S) -> Result<(), String> {
        let name = name.into();
        match name.contains('.') {
            true => Err("Scope names cannot contain periods (\".\")".to_string()),
            false => {
                // Construct the scope name.
                let scope = match self.scope.is_empty() {
                    true => name,
                    false => format!("{}.{}", self.scope, name),
                };

                // Save the current scope members.
                self.parents.push((
                    self.scope.clone(),
                    mem::take(&mut self.constraints),
                    self.constants,
                    self.public,
                    self.private,
                    self.nonzeros,
                ));

                // Initialize the new scope members.
                self.scope = scope;
                self.constants = 0;
                self.public = 0;
                self.private = 0;
                self.nonzeros = (0, 0, 0);

                Ok(())
            }
        }
    }

    /// Discards the current scope, reverting to the previous scope.
    pub(crate) fn pop<S: Into<String>>(&mut self, name: S) -> Result<(), String> {
        // Pop the current scope from the full scope.
        let (_previous_scope, current_scope) = match self.scope.rsplit_once('.') {
            Some((previous_scope, current_scope)) => (previous_scope, current_scope),
            None => ("", self.scope.as_str()),
        };

        // Ensure the current scope is the last pushed scope.
        match current_scope == name.into() {
            true => {
                if let Some((scope, constraints, constants, public, private, nonzeros)) = self.parents.pop() {
                    self.scope = scope;
                    self.constraints = constraints;
                    self.constants = constants;
                    self.public = public;
                    self.private = private;
                    self.nonzeros = nonzeros;
                }
            }
            false => {
                return Err("Mismatching scope. Scopes must return in the reverse order they are created".to_string());
            }
        }

        Ok(())
    }

    /// Increments the number of constraints by 1.
    pub(crate) fn add_constraint(&mut self, constraint: Rc<Constraint<F>>) {
        let (a_nonzeros, b_nonzeros, c_nonzeros) = constraint.num_nonzeros();
        self.nonzeros.0 += a_nonzeros;
        self.nonzeros.1 += b_nonzeros;
        self.nonzeros.2 += c_nonzeros;

        self.constraints.push(constraint);
    }

    /// Returns `true` if all constraints in the scope are satisfied.
    pub(crate) fn is_satisfied_in_scope(&self) -> bool {
        self.constraints.iter().all(|constraint| constraint.is_satisfied())
    }

    /// Returns the current scope.
    pub(crate) fn scope(&self) -> Scope {
        self.scope.clone()
    }

    /// Increments the number of constants by 1.
    pub(crate) fn increment_constant(&mut self) {
        self.constants += 1;
    }

    /// Increments the number of public variables by 1.
    pub(crate) fn increment_public(&mut self) {
        self.public += 1;
    }

    /// Increments the number of private variables by 1.
    pub(crate) fn increment_private(&mut self) {
        self.private += 1;
    }

    /// Returns the number of constants in scope in scope.
    pub(crate) fn num_constants_in_scope(&self) -> u64 {
        self.constants
    }

    /// Returns the number of public variables in scope.
    pub(crate) fn num_public_in_scope(&self) -> u64 {
        self.public
    }

    /// Returns the number of private variables in scope.
    pub(crate) fn num_private_in_scope(&self) -> u64 {
        self.private
    }

    /// Returns the number of constraints in scope.
    pub(crate) fn num_constraints_in_scope(&self) -> u64 {
        self.constraints.len() as u64
    }

    /// Returns the number of nonzeros in scope.
    pub(crate) fn num_nonzeros_in_scope(&self) -> (u64, u64, u64) {
        self.nonzeros
    }
}
