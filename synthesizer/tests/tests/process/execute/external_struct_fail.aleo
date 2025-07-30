/*
randomness: 45791624
cases: []
*/

program parent.aleo;

struct S:
    x as field;

function make_s:
    cast 12field into r0 as S;
    output r0 as S.public;

/////////////////////////////////////////////////

import parent.aleo;

program child.aleo;

struct T:
    x as field;

function make_s:
    // This should fail as the struct doesn't exist.
    cast 2field into r0 as parent.aleo/S2;
    output r0 as parent.aleo/S2.public;
