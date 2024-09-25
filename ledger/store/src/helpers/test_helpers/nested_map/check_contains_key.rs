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
use crate::helpers::NestedMap;

const NUM_ITEMS: usize = 10;
const NUM_TOTAL_ITEMS: usize = 20;

pub fn check_contains_key(map: impl for<'a> NestedMap<'a, usize, usize, String>) {
    ensure_map_is_empty(&map);

    const MAP: usize = 0;

    for i in 0..NUM_ITEMS {
        println!("i: {}", i);

        assert!(!map.contains_key_confirmed(&MAP, &i).unwrap());
        assert!(!map.contains_key_speculative(&MAP, &i).unwrap());

        // Insert an item into the map.
        map.insert(MAP, i, i.to_string()).unwrap();

        assert!(map.contains_key_confirmed(&MAP, &i).unwrap());
        assert!(map.contains_key_speculative(&MAP, &i).unwrap());
    }

    /* test atomic insertions */

    {
        // Start an atomic write batch.
        map.start_atomic();

        for i in NUM_ITEMS..NUM_TOTAL_ITEMS {
            println!("i: {}", i);

            assert!(!map.contains_key_confirmed(&MAP, &i).unwrap());
            assert!(!map.contains_key_speculative(&MAP, &i).unwrap());

            // Insert an item into the map.
            map.insert(MAP, i, i.to_string()).unwrap();

            assert!(!map.contains_key_confirmed(&MAP, &i).unwrap());
            assert!(map.contains_key_speculative(&MAP, &i).unwrap());
        }

        // Finish the current atomic write batch.
        map.finish_atomic().unwrap();
    }

    for i in 0..NUM_TOTAL_ITEMS {
        assert!(map.contains_key_confirmed(&MAP, &i).unwrap());
        assert!(map.contains_key_speculative(&MAP, &i).unwrap());

        // Remove the item from the map.
        map.remove_key(&MAP, &i).unwrap();

        assert!(!map.contains_key_confirmed(&MAP, &i).unwrap());
        assert!(!map.contains_key_speculative(&MAP, &i).unwrap());
    }

    ensure_map_is_empty(&map);
}
