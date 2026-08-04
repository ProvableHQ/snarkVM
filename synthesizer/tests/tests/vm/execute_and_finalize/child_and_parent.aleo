/*
randomness: 45791624
cases:
  - program: child.aleo
    function: foo
    inputs: []
  - program: parent.aleo
    function: foo
    inputs: []
*/

program child.aleo;

function foo:
    output self.caller as address.public;
    output self.signer as address.public;
    
constructor:
    assert.eq edition 0u16;

/////////////////////////////////////////////////

import child.aleo;

program parent.aleo;

function foo:
    call child.aleo/foo into r0 r1;
    output r0 as address.public;
    output r1 as address.public;
    output self.caller as address.public;
    output self.signer as address.public;
    
constructor:
    assert.eq edition 0u16;

