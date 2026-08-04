/*
randomness: 45791624
cases: []
*/

program child.aleo;

function foo:
    input r0 as field.private;
    input r1 as field.private;
    async foo self.caller into r2;
    add r0 r1 into r3;
    output r3 as field.private;
    
constructor:
    assert.eq edition 0u16;




