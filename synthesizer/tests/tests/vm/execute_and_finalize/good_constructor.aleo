/*
randomness: 45791624
cases:
  - program: good_constructor.aleo
    function: check
    inputs: []
*/

import credits.aleo;

program good_constructor.aleo;

mapping data:
    key as u8.public;
    value as u8.public;

constructor:
    assert.eq edition 0u16;
    assert.eq credits.aleo/edition 0u16;
    cast 66u8 96u8 158u8 151u8 29u8 229u8 98u8 162u8 249u8 193u8 81u8 68u8 81u8 245u8 4u8 25u8 30u8 238u8 240u8 107u8 121u8 228u8 148u8 181u8 130u8 212u8 178u8 214u8 26u8 4u8 155u8 54u8 into r0 as [u8; 32u32];
    assert.eq credits.aleo/checksum r0;
    assert.neq checksum r0;
    contains data[0u8] into r1; // Check `contains` without value
    assert.eq r1 false;
    get.or_use data[0u8] 8u8 into r2; // Check `get.or_use` without value
    assert.eq r2 8u8;
    set 1u8 into data[0u8]; // Check `set`
    contains data[0u8] into r3; // Check `contains` with value
    assert.eq r3 true;
    get.or_use data[0u8] 0u8 into r4; // Check `get.or_use` without value
    assert.eq r4 1u8;
    get data[0u8] into r5; // Check `get` with value
    assert.eq r5 1u8;
    remove data[0u8]; // Check `remove`
    contains data[0u8] into r6; // Check `contains` after removal
    assert.eq r6 false;
    set 1u8 into data[0u8]; // Final set


function check:
    async check into r0;
    output r0 as good_constructor.aleo/check.future;
finalize check:
    get data[0u8] into r0;
    assert.eq r0 1u8;



