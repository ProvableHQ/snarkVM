/*
start_height: 19
randomness: 45791624
cases:
  - program: external_record_child.aleo
    function: main
    inputs: ["aleo1qr2ha4pfs5l28aze88yn6fhleeythklkczrule2v838uwj65n5gqxt9djx", 42u8]
*/

program external_struct_parent.aleo;

struct S:
    data as u8;

function make_struct:
    input r0 as u8.private;
    cast r0 into r1 as S;
    output r1 as S.private;

constructor:
    assert.eq edition 0u16;


/////////////////////////////////////////////////

import external_struct_parent.aleo;

program external_record_child.aleo;

record R:
    owner as address.private;
    s_data as external_struct_parent.aleo/S.private;  // <-- record with external struct

closure check_records:
    input r0 as R.record;
    input r1 as R.record;
    // compare fields individually
    assert.eq r0.owner r1.owner;
    assert.eq r0.s_data.data r1.s_data.data;

function main:
    input r0 as address.public;
    input r1 as u8.private;

    // create external structs
    call external_struct_parent.aleo/make_struct r1 into r2;
    call external_struct_parent.aleo/make_struct r1 into r3;

    // cast into records
    cast r0 r2 into r4 as R.record;
    cast r0 r3 into r5 as R.record;

    // compare fields individually
    assert.eq r4.owner r5.owner;
    assert.eq r4.s_data r5.s_data;

    // optional: use closure to check again
    call check_records r4 r5;

constructor:
    assert.eq edition 0u16;
