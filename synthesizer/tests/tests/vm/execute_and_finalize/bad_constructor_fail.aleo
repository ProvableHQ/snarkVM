/*
randomness: 45791624
cases:
  - program: bad_constructor.aleo
    function: foo
    inputs: ["1field", "2field"]
*/

program bad_constructor.aleo;

constructor:
    assert.eq true false;

function foo:
    input r0 as field.private;
    input r1 as field.private;
    add r0 r1 into r2;
    output r2 as field.private;



