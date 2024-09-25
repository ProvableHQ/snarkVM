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

fn check_get_unique_maps(map: &impl for<'a> NestedMap<'a, usize, usize, String>) {
    ensure_map_is_empty(map);

    for i in 0..NUM_ITEMS {
        println!("insert({i})");

        assert_eq!(map.get_map_confirmed(&i).unwrap(), Vec::new());
        assert_eq!(map.get_map_speculative(&i).unwrap(), Vec::new());

        // Insert an item into the map.
        map.insert(i, i, i.to_string()).unwrap();

        assert_eq!(map.get_map_confirmed(&i).unwrap(), vec![(i, i.to_string())]);
        assert_eq!(map.get_map_speculative(&i).unwrap(), vec![(i, i.to_string())]);
    }

    /* test atomic insertions */

    {
        // Start an atomic write batch.
        map.start_atomic();

        for i in NUM_ITEMS..NUM_TOTAL_ITEMS {
            println!("insert({i}) [atomic]");

            assert_eq!(map.get_map_confirmed(&i).unwrap(), Vec::new());
            assert_eq!(map.get_map_speculative(&i).unwrap(), Vec::new());

            // Insert an item into the map.
            map.insert(i, i, i.to_string()).unwrap();

            assert_eq!(map.get_map_confirmed(&i).unwrap(), Vec::new());
            assert_eq!(map.get_map_speculative(&i).unwrap(), vec![(i, i.to_string())]);
        }

        // Finish the current atomic write batch.
        map.finish_atomic().unwrap();
    }

    for i in 0..NUM_TOTAL_ITEMS {
        println!("remove_map({i})");

        map.remove_map(&i).unwrap();
    }

    ensure_map_is_empty(map);
}

fn check_get_same_map(map: &impl for<'a> NestedMap<'a, usize, usize, String>) {
    ensure_map_is_empty(map);

    const MAP: usize = 0;

    let mut confirmed = Vec::new();
    let mut speculative = Vec::new();

    for i in 0..NUM_ITEMS {
        println!("i: {}", i);

        assert_eq!(map.get_map_confirmed(&MAP).unwrap(), confirmed);
        assert_eq!(map.get_map_speculative(&MAP).unwrap(), speculative);

        // Insert an item into the map.
        map.insert(MAP, i, i.to_string()).unwrap();
        confirmed.push((i, i.to_string()));
        speculative.push((i, i.to_string()));

        assert_eq!(map.get_map_confirmed(&MAP).unwrap(), confirmed);
        assert_eq!(map.get_map_speculative(&MAP).unwrap(), speculative);
    }

    /* test atomic insertions */

    {
        // Start an atomic write batch.
        map.start_atomic();

        for i in NUM_ITEMS..NUM_TOTAL_ITEMS {
            println!("i: {}", i);

            assert_eq!(map.get_map_confirmed(&MAP).unwrap(), confirmed);
            assert_eq!(map.get_map_speculative(&MAP).unwrap(), speculative);

            // Insert an item into the map.
            map.insert(MAP, i, i.to_string()).unwrap();
            speculative.push((i, i.to_string()));

            assert_eq!(map.get_map_confirmed(&MAP).unwrap(), confirmed);
            assert_eq!(map.get_map_speculative(&MAP).unwrap(), speculative);
        }

        // Finish the current atomic write batch.
        map.finish_atomic().unwrap();

        assert_eq!(map.get_map_confirmed(&MAP).unwrap(), speculative);
        assert_eq!(map.get_map_speculative(&MAP).unwrap(), speculative);
    }

    map.remove_map(&MAP).unwrap();

    ensure_map_is_empty(map);
}

pub fn check_get_map(map: impl for<'a> NestedMap<'a, usize, usize, String>) {
    println!("Checking get unique maps");
    check_get_unique_maps(&map);

    println!("Checking get same map");
    check_get_same_map(&map);
}
