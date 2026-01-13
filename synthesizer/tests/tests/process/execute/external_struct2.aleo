/*
randomness: 45791624
cases:
  - program: external_struct2_child.aleo
    function: main
    inputs: [1u8]
*/

program external_struct2_parent.aleo;

struct S:
    data as u8;

function make_struct:
    input r0 as u8.private;
    cast r0 into r1 as S;
    output r1 as S.private;

/////////////////////////////////////////////////


import external_struct2_parent.aleo;

program external_struct2_child.aleo;

struct S:
    data as u8;

closure check_structs:
    input r0 as external_struct2_parent.aleo/S;
    input r1 as S;
    is.eq r0 r1 into r2;
    is.neq r0 r1 into r3;
    assert.eq r0 r1;
    assert.eq r2 true;
    assert.eq r3 false;

function main:
    input r0 as u8.private;
    call external_struct2_parent.aleo/make_struct r0 into r1;
    call external_struct2_parent.aleo/make_struct r0 into r2;
    cast r0 into r3 as S;
    assert.eq r1 r2;
    assert.eq r1 r3;
    call check_structs r1 r2;
    call check_structs r1 r3;
