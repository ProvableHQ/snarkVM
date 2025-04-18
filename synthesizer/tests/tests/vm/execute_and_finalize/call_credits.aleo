/*
randomness: 45791624
cases:
    - program: call_credits.aleo
      function: pub_to_priv
      inputs: [1u64]
*/

import credits.aleo;

program call_credits.aleo;

function pub_to_priv:
    input r0 as u64.private;
    call credits.aleo/transfer_public_to_private self.signer r0 into r1 r2;
    async pub_to_priv r2 into r3;
    output self.signer as address.public;
    output r3 as call_credits.aleo/pub_to_priv.future;
finalize pub_to_priv:
    input r0 as credits.aleo/transfer_public_to_private.future;
    await r0;
