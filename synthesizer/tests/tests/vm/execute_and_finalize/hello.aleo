/*
randomness: 12345
cases:
  - program: hello.aleo
    function: hello
    inputs: [0u32, 1u32]
  - program: hello.aleo
    function: hello
    inputs: [1u32, 2u32]
  - program: hello.aleo
    function: goodbye
    inputs: [0u32, 1u32]
  - program: hello.aleo
    function: goodbye
    inputs: [1u32, 0u32]
*/

program hello.aleo;

function hello:
    input r0 as u32.public;
    input r1 as u32.private;
    add r0 r1 into r2;
    output r2 as u32.private;

function goodbye:
    input r0 as u32.public;
    input r1 as u32.private;
    add r0 r1 into r2;
    async goodbye r1 r2 into r3;
    output r2 as u32.public;
    output r3 as hello.aleo/goodbye.future;

finalize goodbye:
    input r0 as u32.public;
    input r1 as u32.public;
    add r0 r1 into r2;
    assert.neq r1 r2;
    
constructor:
    assert.eq edition 0u16;
