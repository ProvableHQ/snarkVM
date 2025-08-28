/*
start_height: 19
randomness: 45791624
cases: []
*/

program child.aleo;

function dummy:

constructor:
    assert.eq edition 0u16;

/////////////////////////////////////////////////

import child.aleo;

program parent.aleo;

mapping foo:
    key as field.public;
    value as child.aleo/S.public;

function dummy:

constructor:
    assert.eq edition 0u16;

