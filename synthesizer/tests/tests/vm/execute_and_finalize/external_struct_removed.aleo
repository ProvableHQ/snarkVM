/*
randomness: 45791624
cases:
  - program: child.aleo
    function: foo
    inputs: []
*/

program parent0.aleo;

struct S:
    x as field;

function foo:
    cast 12field into r0 as S;
    output r0 as S.public;

/////////////////////////////////////////////////

import parent0.aleo;

program parent1.aleo;

function foo:
    call parent0.aleo/foo into r0;
    output r0 as parent0.aleo/S.public;

/////////////////////////////////////////////////

import parent0.aleo;
import parent1.aleo;

program child.aleo;

function foo:
    call parent1.aleo/foo into r0;
