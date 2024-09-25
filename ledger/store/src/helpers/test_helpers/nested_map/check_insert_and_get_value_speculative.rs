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

pub fn check_insert_and_get_value_speculative(map: impl for<'a> NestedMap<'a, usize, usize, String>) {
    // Sanity check.
    assert!(map.iter_confirmed().next().is_none());

    /* test atomic insertions */

    // Start an atomic write batch.
    map.start_atomic();

    // Insert an item into the map.
    map.insert(0, 0, "0".to_string()).unwrap();

    // Check that the item is not yet in the map.
    assert!(map.get_value_confirmed(&0, &0).unwrap().is_none());
    assert_eq!(map.len_map_confirmed(&0).unwrap(), 0);
    assert!(map.is_empty_map_confirmed(&0).unwrap());
    // Check that the item is in the batch.
    assert_eq!(map.get_value_pending(&0, &0), Some(Some("0".to_string())));
    // Check that the item can be speculatively retrieved.
    assert_eq!(map.get_value_speculative(&0, &0).unwrap(), Some(Cow::Owned("0".to_string())));

    // Queue (since a batch is in progress) NUM_ITEMS insertions.
    for i in 1..10 {
        // Update the item in the map.
        map.insert(0, 0, i.to_string()).unwrap();

        // Check that the item is not yet in the map.
        assert!(map.get_value_confirmed(&0, &0).unwrap().is_none());
        // Check that the updated item is in the batch.
        assert_eq!(map.get_value_pending(&0, &0), Some(Some(i.to_string())));
        // Check that the updated item can be speculatively retrieved.
        assert_eq!(map.get_value_speculative(&0, &0).unwrap(), Some(Cow::Owned(i.to_string())));
    }

    // The map should still contain no items.
    assert!(map.iter_confirmed().next().is_none());
    assert_eq!(map.len_map_confirmed(&0).unwrap(), 0);
    assert!(map.is_empty_map_confirmed(&0).unwrap());

    // Finish the current atomic write batch.
    map.finish_atomic().unwrap();

    // Check that the item is present in the map now.
    assert_eq!(map.get_value_confirmed(&0, &0).unwrap(), Some(Cow::Owned("9".to_string())));
    assert_eq!(map.len_map_confirmed(&0).unwrap(), 1);
    assert!(!map.is_empty_map_confirmed(&0).unwrap());
    // Check that the item is not in the batch.
    assert_eq!(map.get_value_pending(&0, &0), None);
    // Check that the item can be speculatively retrieved.
    assert_eq!(map.get_value_speculative(&0, &0).unwrap(), Some(Cow::Owned("9".to_string())));
}
