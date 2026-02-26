/*
start_height: 19
randomness: 84572913
cases:
  - program: committee_test.aleo
    function: main
    inputs: [true, 7u8]
*/

import credits.aleo;

program committee_test.aleo;

// Local mirror of the external type (optional but allowed)
struct committee_state:
    is_open as boolean;
    commission as u8;

// Closure to check individual fields of committee_state
closure check_fields:
    input r0 as credits.aleo/committee_state;
    input r1 as committee_state;

    // compare individual fields
    assert.eq r0.is_open r1.is_open;
    assert.eq r0.commission r1.commission;

// Main function demonstrating use of the external struct
function main:
    input r0 as boolean.private;
    input r1 as u8.private;

    // Construct external struct using cast
    cast r0 r1 into r2 as credits.aleo/committee_state;

    // Construct local struct
    cast r0 r1 into r3 as committee_state;

    // Compare individual fields
    assert.eq r2.is_open r3.is_open;
    assert.eq r2.commission r3.commission;

    // Also test using the closure
    call check_fields r2 r3;

constructor:
    assert.eq edition 0u16;

