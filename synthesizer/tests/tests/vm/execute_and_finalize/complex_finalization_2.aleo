/*
randomness: 402893173
cases:
  - program: outer.aleo
    function: call_mid
    inputs: [0field]
*/

program nofin.aleo;

function do:
    input r0 as field.public;
    mul r0 100field into r1;
    output r1 as field.public;

/////////////////////////////////////////////////

program inner.aleo;

mapping rand_store:
    key as u8.public;
    value as u128.public;

function save_inner_rand:
    input r0 as field.public;
    async save_inner_rand r0 into r1;
    output r1 as inner.aleo/save_inner_rand.future;

finalize save_inner_rand:
    input r0 as field.public;
    rand.chacha r0 into r1 as u128;
    set r1 into rand_store[0u8];

/////////////////////////////////////////////////

import inner.aleo;
import nofin.aleo;

program mid.aleo;

mapping rand_store:
    key as u8.public;
    value as u128.public;

function save_mid_rand:
    input r0 as field.public;
    call nofin.aleo/do r0 into r1;
    call inner.aleo/save_inner_rand r1 into r2;
    call nofin.aleo/do r1 into r3;
    async save_mid_rand r2 into r4;
    output r3 as field.public;
    output r4 as mid.aleo/save_mid_rand.future;

finalize save_mid_rand:
    input r0 as inner.aleo/save_inner_rand.future;
    await r0;
    rand.chacha into r1 as u128;
    set r1 into rand_store[0u8];


/////////////////////////////////////////////////

import inner.aleo;
import nofin.aleo;
import mid.aleo;

program outer.aleo;

function call_mid:
    input r0 as field.public;
    add r0 1field into r1;
    call nofin.aleo/do r1 into r2;
    call mid.aleo/save_mid_rand r0 into r3 r4;
    call nofin.aleo/do r2 into r5;
    call mid.aleo/save_mid_rand r3 into r6 r7;
    call nofin.aleo/do r5 into r8;
    async call_mid r4 r7 into r9;
    output r8 as field.public;
    output r9 as outer.aleo/call_mid.future;

finalize call_mid:
    input r0 as mid.aleo/save_mid_rand.future;
    input r1 as mid.aleo/save_mid_rand.future;
    await r0;
    await r1;

