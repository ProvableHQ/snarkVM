/*
randomness: 490582958
cases:
  - program: count_usages.aleo
    function: add_and_subtract
    inputs: [1i64, 2i64]
*/

program basic_math.aleo;

mapping uses:
    key as address.public;
    value as i64.public;

function add_and_count:
    input r0 as i64.private;
    input r1 as i64.private;
    add r0 r1 into r2;
    async add_and_count self.caller into r3;
    output r2 as i64.private;
    output r3 as basic_math.aleo/add_and_count.future;

finalize add_and_count:
    input r0 as address.public;
    get.or_use uses[r0] 0i64 into r1;
    add r1 1i64 into r2;
    set r2 into uses[r0];

function sub_and_count:
    input r0 as i64.private;
    input r1 as i64.private;
    sub r0 r1 into r2;
    async sub_and_count self.caller into r3;
    output r2 as i64.private;
    output r3 as basic_math.aleo/sub_and_count.future;

finalize sub_and_count:
    input r0 as address.public;
    get.or_use uses[r0] 0i64 into r1;
    add r1 1i64 into r2;
    set r2 into uses[r0];
    
constructor:
    assert.eq edition 0u16;

/////////////////////////////////////////////////

import basic_math.aleo;

program count_usages.aleo;

function add_and_subtract:
    input r0 as i64.private;
    input r1 as i64.private;
    call basic_math.aleo/add_and_count r0 r1 into r2 r3;
    call basic_math.aleo/sub_and_count r2 r1 into r4 r5;
    assert.eq r0 r4;
    assert.eq r3[0u32] r5[0u32];
    async add_and_subtract r3 r5 into r6;
    output r0 as i64.private;
    output r6 as count_usages.aleo/add_and_subtract.future;

finalize add_and_subtract:
    input r0 as basic_math.aleo/add_and_count.future;
    input r1 as basic_math.aleo/sub_and_count.future;
    assert.eq r0[0u32] r1[0u32];
    await r0;
    await r1;
    
constructor:
    assert.eq edition 0u16;
