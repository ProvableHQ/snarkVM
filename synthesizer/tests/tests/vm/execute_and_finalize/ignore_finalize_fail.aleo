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
    input r0 as field.private;
    input r1 as field.private;
    call child.aleo/foo into r2;
    add r0 r1 into r3;
    output r3 as field.private;
    
constructor:
    assert.eq edition 0u16;
