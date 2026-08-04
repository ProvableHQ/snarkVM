/*
randomness: 95043801
cases:
    - program: branch_with_future.aleo
      function: bar
      inputs: []
    - program: branch_with_future.aleo
      function: baz
      inputs: [true, true]
    - program: branch_with_future.aleo
      function: baz
      inputs: [true, false]
    - program: branch_with_future.aleo
      function: baz
      inputs: [false, true]
    - program: branch_with_future.aleo
      function: baz
      inputs: [false, false]
    - program: branch_with_future.aleo
      function: qux
      inputs: []
*/

program child.aleo;

function foo:
    input r0 as boolean.public;
    input r1 as boolean.public;
    async foo r0 r1 into r2;
    output r2 as child.aleo/foo.future;

finalize foo:
    input r0 as boolean.public;
    input r1 as boolean.public;
    assert.eq r0 r1;
    
constructor:
    assert.eq edition 0u16;


/////////////////////////////////////////////////

import child.aleo;

program branch_with_future.aleo;

function bar:
    call child.aleo/foo true false into r0;
    async bar r0 into r1;
    output r1 as branch_with_future.aleo/bar.future;

finalize bar:
    input r0 as child.aleo/foo.future;
    branch.eq true true to exit;
    await r0;
    position exit;

function baz:
    input r0 as boolean.public;
    input r1 as boolean.public;
    call child.aleo/foo true true into r2;
    call child.aleo/foo true true into r3;
    async baz r0 r1 r2 r3 into r4;
    output r4 as branch_with_future.aleo/baz.future;

finalize baz:
    input r0 as boolean.public;
    input r1 as boolean.public;
    input r2 as child.aleo/foo.future;
    input r3 as child.aleo/foo.future;
    branch.eq r0 false to skip1;
    await r2;
    branch.eq true true to next;
    position skip1;
    await r3;
    position next;
    branch.eq r1 false to skip2;
    await r3;
    branch.eq true true to end;
    position skip2;
    await r2;
    position end;

function qux:
    call child.aleo/foo true true into r0;
    async qux r0 into r1;
    output r1 as branch_with_future.aleo/qux.future;

finalize qux:
    input r0 as child.aleo/foo.future;
    await r0;
    await r0;
    
constructor:
    assert.eq edition 0u16;
