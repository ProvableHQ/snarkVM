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

mod check_atomic_writes_are_batched;
pub use check_atomic_writes_are_batched::*;

mod check_atomic_writes_can_be_aborted;
pub use check_atomic_writes_can_be_aborted::*;

mod check_contains_key;
pub use check_contains_key::*;

mod check_get_map;
pub use check_get_map::*;

mod check_insert_and_get_value_speculative;
pub use check_insert_and_get_value_speculative::*;

mod check_iterators_match;
pub use check_iterators_match::*;

mod check_remove_and_get_value_speculative;
pub use check_remove_and_get_value_speculative::*;

fn ensure_map_is_empty(map: &impl for<'a> crate::helpers::NestedMap<'a, usize, usize, String>) {
    // Sanity check.
    assert!(map.iter_pending().next().is_none());
    assert!(map.iter_confirmed().next().is_none());
    assert!(map.keys_confirmed().next().is_none());
    assert!(map.values_confirmed().next().is_none());
}
