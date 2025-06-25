/*
randomness: 45791624
cases:
  - program: child.aleo
    function: foo
    inputs: []
  - program: child.aleo
    function: foo2
    inputs: []
*/

program parent.aleo;

struct S:
    x as field;

function foo:
    cast 12field into r0 as S;
    output r0 as S.public;

/////////////////////////////////////////////////

import parent.aleo;

program child.aleo;

struct T:
    x as field;

function foo:
    call parent.aleo/foo into r0;
    output r0 as parent.aleo/S.public;

function foo2:
    call parent.aleo/foo into r0;
    output r0 as T.public;
