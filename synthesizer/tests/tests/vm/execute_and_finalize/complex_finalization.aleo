/*
randomness: 490582958
cases:
  - program: four_program.aleo
    function: a
    inputs: []
*/

// This test checks that the finalization order is correct.
// The functions are invoked in the following order:
// "four::a"
//   --> "two::b"
//        --> "zero::c"
//        --> "one::d"
//   --> "three::e"
//        --> "two::b"
//             --> "zero::c"
//             --> "one::d"
//        --> "one::d"
//        --> "zero::c"
// The future (call graph) produced by the top-level finalize should reflect this structure.

program zero_program.aleo;

mapping counts:
    key as address.public;
    value as u64.public;

function c:
    async c self.signer into r0;
    output r0 as zero_program.aleo/c.future;
finalize c:
    input r0 as address.public;
    get.or_use counts[r0] 0u64 into r1;
    add r1 1u64 into r2;
    set r2 into counts[r0];
    
constructor:
    assert.eq edition 0u16;

/////////////////////////////////////////////////

program one_program.aleo;

mapping counts:
    key as address.public;
    value as u64.public;

function d:
    async d self.signer into r0;
    output r0 as one_program.aleo/d.future;
finalize d:
    input r0 as address.public;
    get.or_use counts[r0] 0u64 into r1;
    add r1 1u64 into r2;
    set r2 into counts[r0];
    
constructor:
    assert.eq edition 0u16;

/////////////////////////////////////////////////

import zero_program.aleo;
import one_program.aleo;

program two_program.aleo;

mapping counts:
    key as address.public;
    value as u64.public;

function b:
    call zero_program.aleo/c into r0;
    call one_program.aleo/d into r1;
    async b r0 r1 self.signer into r2;
    output r2 as two_program.aleo/b.future;
finalize b:
    input r0 as zero_program.aleo/c.future;
    input r1 as one_program.aleo/d.future;
    input r2 as address.public;
    await r0;
    await r1;
    get.or_use counts[r2] 0u64 into r3;
    add r3 1u64 into r4;
    set r4 into counts[r2];
    
constructor:
    assert.eq edition 0u16;

/////////////////////////////////////////////////

import zero_program.aleo;
import one_program.aleo;
import two_program.aleo;

program three_program.aleo;

mapping counts:
    key as address.public;
    value as u64.public;

function e:
    call two_program.aleo/b into r0;
    call one_program.aleo/d into r1;
    call zero_program.aleo/c into r2;
    async e r0 r1 r2 self.signer into r3;
    output r3 as three_program.aleo/e.future;
finalize e:
    input r0 as two_program.aleo/b.future;
    input r1 as one_program.aleo/d.future;
    input r2 as zero_program.aleo/c.future;
    input r3 as address.public;
    await r0;
    await r1;
    await r2;
    get.or_use counts[r3] 0u64 into r4;
    add r4 1u64 into r5;
    set r5 into counts[r3];
    
constructor:
    assert.eq edition 0u16;

/////////////////////////////////////////////////

import zero_program.aleo;
import one_program.aleo;
import two_program.aleo;
import three_program.aleo;

program four_program.aleo;

mapping counts:
    key as address.public;
    value as u64.public;

function a:
    call two_program.aleo/b into r0;
    call three_program.aleo/e into r1;
    async a r0 r1 self.signer into r2;
    output r2 as four_program.aleo/a.future;
finalize a:
    input r0 as two_program.aleo/b.future;
    input r1 as three_program.aleo/e.future;
    input r2 as address.public;
    // Assert that all of the inputs to the finalize blocks of `one_program.aleo/d` and `zero_program.aleo/c`.
    assert.eq r0[0u32][0u32] r0[1u32][0u32];
    assert.eq r0[1u32][0u32] r1[0u32][0u32][0u32];
    assert.eq r1[0u32][0u32][0u32] r1[0u32][1u32][0u32];
    assert.eq r1[0u32][1u32][0u32] r1[1u32][0u32];
    assert.eq r1[1u32][0u32] r1[2u32][0u32];
    // Assert that all of the futures of `one_program.aleo/d` are the same.
    assert.eq r0[1u32] r1[0u32][1u32];
    assert.eq r1[0u32][1u32] r1[1u32];
    // Assert that all of the futures of `zero_program.aleo/c` are the same.
    assert.eq r0[0u32] r1[0u32][0u32];
    assert.eq r1[0u32][0u32] r1[2u32];
    await r0;
    await r1;
    get.or_use counts[r2] 0u64 into r3;
    add r3 1u64 into r4;
    set r4 into counts[r2];
    
constructor:
    assert.eq edition 0u16;

