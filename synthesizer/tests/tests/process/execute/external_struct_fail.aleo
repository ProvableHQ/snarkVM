/*
randomness: 45791624
cases: []
*/

program external_struct_fail_parent.aleo;

struct S:
    x as field;

function make_s:
    cast 12field into r0 as S;
    output r0 as S.public;

/////////////////////////////////////////////////

import external_struct_fail_parent.aleo;

program external_struct_fail_child.aleo;

struct T:
    x as field;

function make_s:
    // This should fail as the struct doesn't exist.
    cast 2field into r0 as external_struct_fail_parent.aleo/S2;
    output r0 as external_struct_fail_parent.aleo/S2.public;
