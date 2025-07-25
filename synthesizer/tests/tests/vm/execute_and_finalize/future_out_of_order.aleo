/*
randomness: 45791624
cases:
  - program: parent.aleo
    function: foo
    inputs: []
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

function boo:
    async boo self.caller into r0;
    output r0 as child.aleo/boo.future;

finalize boo:
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
    call child.aleo/foo into r2;
    call child.aleo/boo into r3;
    call child.aleo/boo into r4;
    call child.aleo/boo into r5;
    async foo r5 r4 r1 r0 r2 r3 into r6;
    output r6 as parent.aleo/foo.future;

finalize foo:
    input r0 as child.aleo/boo.future;
    input r1 as child.aleo/boo.future;
    input r2 as child.aleo/foo.future;
    input r3 as child.aleo/foo.future;
    input r4 as child.aleo/foo.future;
    input r5 as child.aleo/boo.future;
    await r4;
    await r1;
    await r3;
    await r2;
    await r0;
    await r5;
    
constructor:
    assert.eq edition 0u16;

