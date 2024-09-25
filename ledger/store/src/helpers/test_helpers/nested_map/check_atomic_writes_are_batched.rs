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

use std::borrow::Cow;

pub fn check_atomic_writes_are_batched(map: impl for<'a> NestedMap<'a, usize, usize, String>) {
    // The number of items that will be inserted into the map.
    const NUM_ITEMS: usize = 10;

    // Sanity check.
    assert!(map.iter_confirmed().next().is_none());

    /* test atomic insertions */

    // Start an atomic write batch.
    map.start_atomic();

    // Queue (since a batch is in progress) NUM_ITEMS insertions.
    for i in 0..NUM_ITEMS {
        map.insert(i, i, i.to_string()).unwrap();
        // Ensure that the item is queued for insertion.
        assert_eq!(map.get_value_pending(&i, &i), Some(Some(i.to_string())));
        // Ensure that the item can be found with a speculative get.
        assert_eq!(map.get_value_speculative(&i, &i).unwrap(), Some(Cow::Owned(i.to_string())));
    }

    // The map should still contain no items.
    assert!(map.iter_confirmed().next().is_none());

    // Finish the current atomic write batch.
    map.finish_atomic().unwrap();

    // Check that the items are present in the map now.
    for i in 0..NUM_ITEMS {
        assert_eq!(map.get_value_confirmed(&i, &i).unwrap(), Some(Cow::Borrowed(&i.to_string())));
    }

    /* test atomic removals */

    // Start an atomic write batch.
    map.start_atomic();

    // Queue (since a batch is in progress) NUM_ITEMS removals.
    for i in 0..NUM_ITEMS {
        map.remove_key(&i, &i).unwrap();
        // Ensure that the item is NOT queued for insertion.
        assert_eq!(map.get_value_pending(&i, &i), Some(None));
    }

    // The map should still contains all the items.
    assert_eq!(map.iter_confirmed().count(), NUM_ITEMS);

    // Finish the current atomic write batch.
    map.finish_atomic().unwrap();

    // Check that the map is empty now.
    assert!(map.iter_confirmed().next().is_none());
}
