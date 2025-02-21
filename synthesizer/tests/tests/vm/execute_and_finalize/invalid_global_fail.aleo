/*
randomness: 89247928
cases: []
*/

program first.aleo;

function a:
    input r0 as u16.public;
    async a r0 into r1;
    output r1 as first.aleo/a.future;
finalize a:
    input r0 as u16.public;
    metadata.get foo into r1 as u16;
    assert.eq r0 r1;

