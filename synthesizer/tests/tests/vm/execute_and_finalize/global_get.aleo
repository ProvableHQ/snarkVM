/*
randomness: 89247928
cases:
  - program: first.aleo
    function: a
    inputs: [0u16]
  - program: first.aleo
    function: a
    inputs: [1u16]
  - program: second.aleo
    function: a
    inputs: [0u16]
  - program: second.aleo
    function: a
    inputs: [1u16]
*/

program first.aleo;

function a:
    input r0 as u16.public;
    async a r0 into r1;
    output r1 as first.aleo/a.future;
finalize a:
    input r0 as u16.public;
    metadata.get edition into r1;
    assert.eq r0 r1;

/////////////////////////////////////////////////

program second.aleo;

function a:
    input r0 as u16.public;
    async a r0 into r1;
    output r1 as second.aleo/a.future;
finalize a:
    input r0 as u16.public;
    metadata.get edition into r1;
    assert.eq r0 r1;
