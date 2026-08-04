/*
randomness: 45791624
cases: []
*/

program child.aleo;

mapping count:
    key as address.public;
    value as field.public;

function foo:
    async foo self.caller into r0;
    output r0 as child.aleo/foo.future;

finalize foo:
    input r0 as address.public;
    get.or_use count[r0] 0field into r1;
    add r1 1field into r2;
    set r2 into count[r0];

constructor:
    assert.eq edition 0u16;

/////////////////////////////////////////////////

import child.aleo;

program parent.aleo;

function foo:
    call child.aleo/foo into r0;
    call child.aleo/foo into r1;
    async foo r0 into r2;
    async foo r1 into r3;
    output r2 as parent.aleo/foo.future;

finalize foo:
    input r0 as child.aleo/foo.future;
    await r0;

constructor:
    assert.eq edition 0u16;
