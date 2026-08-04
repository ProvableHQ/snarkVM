/*
randomness: 12345
cases:
  - program: gas_dos.aleo
    function: make_trash_empty
    inputs: []
  - program: gas_dos.aleo
    function: make_trash
    inputs: [0u8, 1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8, 8u8, 9u8, 10u8, 11u8, 12u8, 13u8, 14u8, 15u8]
  - program: gas_dos.aleo
    function: make_trash_private
    inputs: [0u8, 1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8, 8u8, 9u8, 10u8, 11u8, 12u8, 13u8, 14u8, 15u8]
  - program: gas_dos.aleo
    function: make_trash_u64
    inputs: [0u64, 1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, 8u64, 9u64, 10u64, 11u64, 12u64, 13u64, 14u64, 15u64]
  - program: gas_dos.aleo
    function: make_trash_call_inner
    inputs: []
  - program: gas_dos.aleo
    function: make_trash_call_inner_private
    inputs: []
*/


program child.aleo;

function inner_trash_single:
    input r0 as u8.public;
    output r0 as u8.public;

function inner_trash:
    input r0 as u8.public;
    input r1 as u8.public;
    input r2 as u8.public;
    input r3 as u8.public;
    input r4 as u8.public;
    input r5 as u8.public;
    input r6 as u8.public;
    input r7 as u8.public;
    input r8 as u8.public;
    input r9 as u8.public;
    input r10 as u8.public;
    input r11 as u8.public;
    input r12 as u8.public;
    input r13 as u8.public;
    input r14 as u8.public;
    input r15 as u8.public;

    output r0 as u8.public;
    output r1 as u8.public;
    output r2 as u8.public;
    output r3 as u8.public;
    output r4 as u8.public;
    output r5 as u8.public;
    output r6 as u8.public;
    output r7 as u8.public;
    output r8 as u8.public;
    output r9 as u8.public;
    output r10 as u8.public;
    output r11 as u8.public;
    output r12 as u8.public;
    output r13 as u8.public;
    output r14 as u8.public;
    output r15 as u8.public;

function inner_trash_private:
    input r0 as u8.private;
    input r1 as u8.private;
    input r2 as u8.private;
    input r3 as u8.private;
    input r4 as u8.private;
    input r5 as u8.private;
    input r6 as u8.private;
    input r7 as u8.private;
    input r8 as u8.private;
    input r9 as u8.private;
    input r10 as u8.private;
    input r11 as u8.private;
    input r12 as u8.private;
    input r13 as u8.private;
    input r14 as u8.private;
    input r15 as u8.private;

    output r0 as u8.private;
    output r1 as u8.private;
    output r2 as u8.private;
    output r3 as u8.private;
    output r4 as u8.private;
    output r5 as u8.private;
    output r6 as u8.private;
    output r7 as u8.private;
    output r8 as u8.private;
    output r9 as u8.private;
    output r10 as u8.private;
    output r11 as u8.private;
    output r12 as u8.private;
    output r13 as u8.private;
    output r14 as u8.private;
    output r15 as u8.private;
    
constructor:
    assert.eq edition 0u16;


/////////////////////////////////////////////////

import child.aleo;

program gas_dos.aleo;

function make_trash_empty:

function make_trash:
    input r0 as u8.public;
    input r1 as u8.public;
    input r2 as u8.public;
    input r3 as u8.public;
    input r4 as u8.public;
    input r5 as u8.public;
    input r6 as u8.public;
    input r7 as u8.public;
    input r8 as u8.public;
    input r9 as u8.public;
    input r10 as u8.public;
    input r11 as u8.public;
    input r12 as u8.public;
    input r13 as u8.public;
    input r14 as u8.public;
    input r15 as u8.public;

    output r0 as u8.public;
    output r1 as u8.public;
    output r2 as u8.public;
    output r3 as u8.public;
    output r4 as u8.public;
    output r5 as u8.public;
    output r6 as u8.public;
    output r7 as u8.public;
    output r8 as u8.public;
    output r9 as u8.public;
    output r10 as u8.public;
    output r11 as u8.public;
    output r12 as u8.public;
    output r13 as u8.public;
    output r14 as u8.public;
    output r15 as u8.public;

function make_trash_private:
    input r0 as u8.private;
    input r1 as u8.private;
    input r2 as u8.private;
    input r3 as u8.private;
    input r4 as u8.private;
    input r5 as u8.private;
    input r6 as u8.private;
    input r7 as u8.private;
    input r8 as u8.private;
    input r9 as u8.private;
    input r10 as u8.private;
    input r11 as u8.private;
    input r12 as u8.private;
    input r13 as u8.private;
    input r14 as u8.private;
    input r15 as u8.private;

    output r0 as u8.private;
    output r1 as u8.private;
    output r2 as u8.private;
    output r3 as u8.private;
    output r4 as u8.private;
    output r5 as u8.private;
    output r6 as u8.private;
    output r7 as u8.private;
    output r8 as u8.private;
    output r9 as u8.private;
    output r10 as u8.private;
    output r11 as u8.private;
    output r12 as u8.private;
    output r13 as u8.private;
    output r14 as u8.private;
    output r15 as u8.private;


function make_trash_u64:
    input r0 as u64.public;
    input r1 as u64.public;
    input r2 as u64.public;
    input r3 as u64.public;
    input r4 as u64.public;
    input r5 as u64.public;
    input r6 as u64.public;
    input r7 as u64.public;
    input r8 as u64.public;
    input r9 as u64.public;
    input r10 as u64.public;
    input r11 as u64.public;
    input r12 as u64.public;
    input r13 as u64.public;
    input r14 as u64.public;
    input r15 as u64.public;

    output r0 as u64.public;
    output r1 as u64.public;
    output r2 as u64.public;
    output r3 as u64.public;
    output r4 as u64.public;
    output r5 as u64.public;
    output r6 as u64.public;
    output r7 as u64.public;
    output r8 as u64.public;
    output r9 as u64.public;
    output r10 as u64.public;
    output r11 as u64.public;
    output r12 as u64.public;
    output r13 as u64.public;
    output r14 as u64.public;
    output r15 as u64.public;

function make_trash_call_inner:
    call child.aleo/inner_trash 0u8 1u8 2u8 3u8 4u8 5u8 6u8 7u8 8u8 9u8 10u8 11u8 12u8 13u8 14u8 15u8 into r0 r1 r2 r3 r4 r5 r6 r7 r8 r9 r10 r11 r12 r13 r14 r15;
    call child.aleo/inner_trash 0u8 1u8 2u8 3u8 4u8 5u8 6u8 7u8 8u8 9u8 10u8 11u8 12u8 13u8 14u8 15u8 into r16 r17 r18 r19 r20 r21 r22 r23 r24 r25 r26 r27 r28 r29 r30 r31;
    call child.aleo/inner_trash 0u8 1u8 2u8 3u8 4u8 5u8 6u8 7u8 8u8 9u8 10u8 11u8 12u8 13u8 14u8 15u8 into r32 r33 r34 r35 r36 r37 r38 r39 r40 r41 r42 r43 r44 r45 r46 r47;
    call child.aleo/inner_trash 0u8 1u8 2u8 3u8 4u8 5u8 6u8 7u8 8u8 9u8 10u8 11u8 12u8 13u8 14u8 15u8 into r48 r49 r50 r51 r52 r53 r54 r55 r56 r57 r58 r59 r60 r61 r62 r63;

function make_trash_call_inner_private:
    call child.aleo/inner_trash_private 0u8 1u8 2u8 3u8 4u8 5u8 6u8 7u8 8u8 9u8 10u8 11u8 12u8 13u8 14u8 15u8 into r0 r1 r2 r3 r4 r5 r6 r7 r8 r9 r10 r11 r12 r13 r14 r15;
    call child.aleo/inner_trash_private 0u8 1u8 2u8 3u8 4u8 5u8 6u8 7u8 8u8 9u8 10u8 11u8 12u8 13u8 14u8 15u8 into r16 r17 r18 r19 r20 r21 r22 r23 r24 r25 r26 r27 r28 r29 r30 r31;
    call child.aleo/inner_trash_private 0u8 1u8 2u8 3u8 4u8 5u8 6u8 7u8 8u8 9u8 10u8 11u8 12u8 13u8 14u8 15u8 into r32 r33 r34 r35 r36 r37 r38 r39 r40 r41 r42 r43 r44 r45 r46 r47;
    call child.aleo/inner_trash_private 0u8 1u8 2u8 3u8 4u8 5u8 6u8 7u8 8u8 9u8 10u8 11u8 12u8 13u8 14u8 15u8 into r48 r49 r50 r51 r52 r53 r54 r55 r56 r57 r58 r59 r60 r61 r62 r63;
    
constructor:
    assert.eq edition 0u16;
