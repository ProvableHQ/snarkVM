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

use crate::helpers::Map;

use std::borrow::Cow;

pub fn check_remove_and_get_speculative(map: impl for<'a> Map<'a, usize, String>) {
    // Sanity check.
    assert!(map.iter_confirmed().next().is_none());

    // Insert an item into the map.
    map.insert(0, "0".to_string()).unwrap();

    // Check that the item is present in the map .
    assert_eq!(map.get_confirmed(&0).unwrap(), Some(Cow::Owned("0".to_string())));
    // Check that the item is not in the batch.
    assert_eq!(map.get_pending(&0), None);
    // Check that the item can be speculatively retrieved.
    assert_eq!(map.get_speculative(&0).unwrap(), Some(Cow::Owned("0".to_string())));

    /* test atomic removals */

    // Start an atomic write batch.
    map.start_atomic();

    // Remove the item from the map.
    map.remove(&0).unwrap();

    // Check that the item still exists in the map.
    assert_eq!(map.get_confirmed(&0).unwrap(), Some(Cow::Owned("0".to_string())));
    // Check that the item is removed in the batch.
    assert_eq!(map.get_pending(&0), Some(None));
    // Check that the item is removed when speculatively retrieved.
    assert_eq!(map.get_speculative(&0).unwrap(), None);

    // Try removing the item again.
    map.remove(&0).unwrap();

    // Check that the item still exists in the map.
    assert_eq!(map.get_confirmed(&0).unwrap(), Some(Cow::Owned("0".to_string())));
    // Check that the item is removed in the batch.
    assert_eq!(map.get_pending(&0), Some(None));
    // Check that the item is removed when speculatively retrieved.
    assert_eq!(map.get_speculative(&0).unwrap(), None);

    // Finish the current atomic write batch.
    map.finish_atomic().unwrap();

    // Check that the item is not present in the map now.
    assert!(map.get_confirmed(&0).unwrap().is_none());
    // Check that the item is not in the batch.
    assert_eq!(map.get_pending(&0), None);
    // Check that the item is removed when speculatively retrieved.
    assert_eq!(map.get_speculative(&0).unwrap(), None);

    // Check that the map is empty now.
    assert!(map.iter_confirmed().next().is_none());
}
