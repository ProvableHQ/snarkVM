/*
start_height: 0
cases:
  - program: timelock.aleo
    function: lock
    inputs: []
  - program: timelock.aleo
    function: lock
    inputs: []
*/

program timelock.aleo;

function lock:
    async lock into r0;
    output r0 as timelock.aleo/lock.future;

finalize lock:
    gt block.height 4u32 into r0; // It takes 4 blocks to set up the test.
    assert.eq r0 true;
