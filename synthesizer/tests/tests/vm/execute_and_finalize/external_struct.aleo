/*
start_height: 19
randomness: 45791624
cases:
  - program: external_struct_test.aleo
    function: main
    inputs: ["0u8"]
*/

program external_struct.aleo;

struct S:
    data as u8;

function make_struct:
    input r0 as u8.private;
    cast r0 into r1 as S;
    output r1 as S.private;

constructor:
    assert.eq edition 0u16;


/////////////////////////////////////////////////

import external_struct.aleo;
program external_struct_test.aleo;

struct S:
    data as u8;

closure check_structs:
    input r0 as S;
    input r1 as S;
    assert.eq r0 r1;

function main:
    input r0 as u8.private;
    call external_struct.aleo/make_struct r0 into r1;
    call external_struct.aleo/make_struct r0 into r2;
    cast r0 into r3 as S;
    assert.eq r1 r2;
    assert.eq r1 r3;
    call check_structs r1 r2;
    call check_structs r1 r3;

constructor:
    assert.eq edition 0u16;
