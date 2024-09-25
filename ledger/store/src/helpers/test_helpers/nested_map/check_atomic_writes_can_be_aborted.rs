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

use crate::helpers::NestedMap;

pub fn check_atomic_writes_can_be_aborted(map: impl for<'a> NestedMap<'a, usize, usize, String>) {
    // The number of items that will be queued to be inserted into the map.
    const NUM_ITEMS: usize = 10;

    // Sanity check.
    assert!(map.iter_confirmed().next().is_none());

    // Start an atomic write batch.
    map.start_atomic();

    // Queue (since a batch is in progress) NUM_ITEMS insertions.
    for i in 0..NUM_ITEMS {
        map.insert(i, i, i.to_string()).unwrap();
    }

    // The map should still contain no items.
    assert!(map.iter_confirmed().next().is_none());

    // Abort the current atomic write batch.
    map.abort_atomic();

    // The map should still contain no items.
    assert!(map.iter_confirmed().next().is_none());

    // Start another atomic write batch.
    map.start_atomic();

    // Queue (since a batch is in progress) NUM_ITEMS insertions.
    for i in 0..NUM_ITEMS {
        map.insert(i, i, i.to_string()).unwrap();
    }

    // Finish the current atomic write batch.
    map.finish_atomic().unwrap();

    // The map should contain NUM_ITEMS items now.
    assert_eq!(map.iter_confirmed().count(), NUM_ITEMS);
}
