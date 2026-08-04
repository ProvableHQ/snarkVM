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
    call external_struct_fail_parent.aleo/make_s into r0;
    // This should fail as the struct names don't match.
    output r0 as T.public;
