/*
randomness: 402893173
cases:
  - program: mid.aleo
    function: save_mid_rand_2
    inputs: [0field]
  - program: outer.aleo
    function: call_mid_3
    inputs: [0field]
  - program: outer.aleo
    function: call_mid
    inputs: [0field]
  - program: outer.aleo
    function: call_mid_2
    inputs: [0field]
  - program: outer.aleo
    function: dummy
    inputs: []
  - program: outer.aleo
    function: dummy
    inputs: []
  - program: outer.aleo
    function: dummy
    inputs: []
  - program: outer.aleo
    function: call_mid
    inputs: [0field]
  - program: outer.aleo
    function: call_mid_2
    inputs: [0field]
*/

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

function dummy:

/////////////////////////////////////////////////

import inner.aleo;

program mid.aleo;

mapping rand_store:
    key as u8.public;
    value as u128.public;


function save_mid_rand:
    input r0 as field.public;
    call inner.aleo/save_inner_rand r0 into r1;
    async save_mid_rand r1 into r2;
    output r2 as mid.aleo/save_mid_rand.future;

finalize save_mid_rand:
    input r0 as inner.aleo/save_inner_rand.future;
    await r0;
    rand.chacha into r1 as u128;
    set r1 into rand_store[0u8];

// A call to `save_mid_rand_2` should be accepted because the non-async call is not a complex one.
function save_mid_rand_2:
    input r0 as field.public;
    call inner.aleo/dummy;
    call inner.aleo/save_inner_rand r0 into r1;
    async save_mid_rand_2 r1 into r2;
    output r2 as mid.aleo/save_mid_rand_2.future;

finalize save_mid_rand_2:
    input r0 as inner.aleo/save_inner_rand.future;
    await r0;
    rand.chacha into r1 as u128;
    set r1 into rand_store[0u8];

function dummy:


/////////////////////////////////////////////////

import inner.aleo;
import mid.aleo;

program outer.aleo;

// A call to `call_mid` will be rejected if called before the `CONSENSUS_V3_HEIGHT` because the complex non-async call
// is before the async ones, triggering a known failure.
// If it is called after the `CONSENSUS_V3_HEIGHT`, then it should be accepted.
function call_mid:
    input r0 as field.public;
    call mid.aleo/dummy;
    call mid.aleo/save_mid_rand r0 into r1;
    call mid.aleo/dummy;
    async call_mid r1 into r2;
    output r2 as outer.aleo/call_mid.future;

finalize call_mid:
    input r0 as mid.aleo/save_mid_rand.future;
    await r0;

// A call to `call_mid_2` will be rejected if called before the `CONSENSUS_V3_HEIGHT` because the complex non-async call
// is before the async ones, triggering a known failure.
// If it is called after the `CONSENSUS_V3_HEIGHT`, then it should be accepted.
function call_mid_2:
    input r0 as field.public;
    call mid.aleo/save_mid_rand r0 into r1;
    call mid.aleo/dummy;
    call mid.aleo/save_mid_rand r0 into r2;
    async call_mid_2 r1 r2 into r3;
    output r3 as outer.aleo/call_mid_2.future;

finalize call_mid_2:
    input r0 as mid.aleo/save_mid_rand.future;
    input r1 as mid.aleo/save_mid_rand.future;
    await r1;
    await r0;

// A call to `call_mid_3` should be accepted because the non-async call is after the async ones.
function call_mid_3:
    input r0 as field.public;
    call mid.aleo/save_mid_rand r0 into r1;
    call mid.aleo/save_mid_rand r0 into r2;
    call mid.aleo/dummy;
    async call_mid_3 r1 r2 into r3;
    output r3 as outer.aleo/call_mid_3.future;

finalize call_mid_3:
    input r0 as mid.aleo/save_mid_rand.future;
    input r1 as mid.aleo/save_mid_rand.future;
    await r1;
    await r0;

function dummy:

