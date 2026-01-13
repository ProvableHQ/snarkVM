/*
start_height: 19 
randomness: 73488219
cases:
  - program: committee_array_test.aleo
    function: create_arrays
    inputs: [true, 5u8]
  - program: committee_array_test.aleo
    function: create_mixed_arrays
    inputs: [true, 5u8]
*/

import credits.aleo;

program committee_array_test.aleo;

struct committee_state:
    is_open as boolean;
    commission as u8;

// Check two arrays of committee_state for element-wise equality
closure check_array:
    input r0 as [credits.aleo/committee_state; 2u32];
    input r1 as [credits.aleo/committee_state; 2u32];

    assert.eq r0[0u32] r1[0u32];
    assert.eq r0[1u32] r1[1u32];

// Constructs two identical arrays of external structs and checks them
function create_arrays:
    input r0 as boolean.private;
    input r1 as u8.private;

    // Build two identical committee_state structs
    cast r0 r1 into r2 as credits.aleo/committee_state;
    cast r0 r1 into r3 as credits.aleo/committee_state;

    // Construct arrays (length uses u32)
    cast r2 r3 into r4 as [credits.aleo/committee_state; 2u32];
    cast r2 r3 into r5 as [credits.aleo/committee_state; 2u32];

    // Compare the array elements explicitly
    call check_array r4 r5;

// Closure to compare external vs local arrays
closure check_mixed_array:
    input r0 as [credits.aleo/committee_state; 2u32];
    input r1 as [committee_state; 2u32];

    // Special rule: identical external vs local structs can be eq'd
    assert.eq r0[0u32] r1[0u32];
    assert.eq r0[1u32] r1[1u32];

// Function that creates mixed arrays
function create_mixed_arrays:
    input r0 as boolean.private;
    input r1 as u8.private;

    // External structs
    cast r0 r1 into r2 as credits.aleo/committee_state;
    cast r0 r1 into r3 as credits.aleo/committee_state;

    // Local structs
    cast r0 r1 into r4 as committee_state;
    cast r0 r1 into r5 as committee_state;

    // Build arrays
    cast r2 r3 into r6 as [credits.aleo/committee_state; 2u32];
    cast r4 r5 into r7 as [committee_state; 2u32];

    // Compare them using the special rule
    call check_mixed_array r6 r7;

constructor:
    assert.eq edition 0u16;

