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

// Originally derived from WHIR (https://github.com/WizardOfMenlo/whir),
// licensed under Apache-2.0 OR MIT.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
    sync::{Arc, RwLock},
};

/// A trait *family*: for each `T`, specifies the corresponding erased dyn-trait
/// object type.
pub trait Family {
    type Dyn<T: 'static>: ?Sized + Send + Sync + 'static;
}

/// A map from types `T` to objects of type `Arc<F::Dyn<T>>`.
pub struct TypeMap<F: Family> {
    inner: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
    _marker: PhantomData<F>,
}

struct Holder<F: Family, T: 'static>(Arc<F::Dyn<T>>);

impl<F: 'static + Family> Default for TypeMap<F> {
    fn default() -> Self {
        Self { inner: RwLock::new(HashMap::default()), _marker: PhantomData }
    }
}

impl<F: 'static + Family> TypeMap<F> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: 'static>(&self, v: Arc<F::Dyn<T>>) {
        self.inner
            .write()
            .expect("Lock poisoned")
            .insert(TypeId::of::<T>(), Box::new(Holder::<F, T>(v)) as Box<dyn Any + Send + Sync>);
    }

    pub fn get<T: 'static>(&self) -> Option<Arc<F::Dyn<T>>> {
        self.inner
            .read()
            .expect("Lock poisoned")
            .get(&TypeId::of::<T>())
            .and_then(|a| a.downcast_ref::<Holder<F, T>>())
            .map(|h| h.0.clone())
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.inner.read().expect("Lock poisoned").contains_key(&TypeId::of::<T>())
    }

    pub fn remove<T: 'static>(&self) -> Option<Arc<F::Dyn<T>>> {
        self.inner
            .write()
            .expect("Lock poisoned")
            .remove(&TypeId::of::<T>())
            .and_then(|a| a.downcast::<Holder<F, T>>().ok())
            .map(|h| h.0)
    }
}
