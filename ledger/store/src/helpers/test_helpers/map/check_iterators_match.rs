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

use super::ensure_map_is_empty;
use crate::helpers::Map;

fn check_pending_iterator(map: &impl for<'a> Map<'a, usize, String>, expected_length: usize) {
    let mut counter = 0;

    // Retrieve the iterator.
    let mut iter_pending = map.iter_pending();

    while counter < expected_length {
        let _ = iter_pending.next().unwrap();

        counter += 1;
    }

    // Ensure there is nothing left.
    assert!(iter_pending.next().is_none());
}

fn check_confirmed_iterators_match(map: &impl for<'a> Map<'a, usize, String>, expected_length: usize) {
    let mut counter = 0;

    // Retrieve the iterators.
    let mut iter_confirmed = map.iter_confirmed();
    let mut keys_confirmed = map.keys_confirmed();
    let mut values_confirmed = map.values_confirmed();

    while counter < expected_length {
        let (key_0, value_0) = iter_confirmed.next().unwrap();
        let key_1 = keys_confirmed.next().unwrap();
        let value_1 = values_confirmed.next().unwrap();

        // Ensure they match.
        assert_eq!(key_0, key_1);
        assert_eq!(value_0, value_1);

        counter += 1;
    }

    // Ensure there is nothing left.
    assert!(iter_confirmed.next().is_none(), "iter_confirmed should be empty");
    assert!(keys_confirmed.next().is_none(), "keys_confirmed should be empty");
    assert!(values_confirmed.next().is_none(), "values_confirmed should be empty");
}

pub fn check_iterators_match(map: impl for<'a> Map<'a, usize, String>) {
    ensure_map_is_empty(&map);

    const NUM_ITEMS: usize = 10;
    const NUM_TOTAL_ITEMS: usize = 20;

    for i in 0..NUM_ITEMS {
        println!("i: {}", i);

        // Insert an item into the map.
        map.insert(i, i.to_string()).unwrap();
    }

    check_confirmed_iterators_match(&map, NUM_ITEMS);

    /* test atomic insertions */

    {
        // Start an atomic write batch.
        map.start_atomic();

        for i in NUM_ITEMS..NUM_TOTAL_ITEMS {
            println!("i: {}", i);

            // Insert an item into the map.
            map.insert(i, i.to_string()).unwrap();

            check_pending_iterator(&map, i - NUM_ITEMS + 1);

            check_confirmed_iterators_match(&map, NUM_ITEMS);
        }

        // Finish the current atomic write batch.
        map.finish_atomic().unwrap();
    }

    check_confirmed_iterators_match(&map, NUM_TOTAL_ITEMS);
}
