/*
randomness: 5890364059
cases:
  - program: public_wallet.aleo
    function: init
    inputs: [aleo1sry3pke49ykrf0aeshf889tr98r4c86p5f4ms766795ssdwfdyqq9jdg0j, 10u64]
*/


program token.aleo;

// On-chain storage of an `account` map, with `owner` as the key,
// and `amount` as the value.
mapping account:
    // The token owner.
    key as address.public;
    // The token amount.
    value as u64.public;

// The function `mint_public` issues the specified token amount
// for the token receiver publicly on the network.
function mint_public:
    // Input the token receiver.
    input r0 as address.public;
    // Input the token amount.
    input r1 as u64.public;
    // Mint the tokens publicly.
    async mint_public r0 r1 into r2;
    output r2 as token.aleo/mint_public.future;

// The finalize scope of `mint_public` increments the
// `account` of the token receiver by the specified amount.
finalize mint_public:
    // Input the token receiver.
    input r0 as address.public;
    // Input the token amount.
    input r1 as u64.public;

    // Get `account[r0]` into `r2`, defaulting to 0u64 if the entry does not exist.
    get.or_use account[r0] 0u64 into r2;
    // Add `r1` to `r2`. If the operation overflows, `mint_public` is reverted.
    add r2 r1 into r3;
    // Set `r3` into `account[r0]`.
    set r3 into account[r0];
    
constructor:
    assert.eq edition 0u16;

/////////////////////////////////////////////////

import token.aleo;

program public_wallet.aleo;

function init:
    input r0 as address.public;
    input r1 as u64.public;
    call token.aleo/mint_public r0 r1 into r2;
    async init r2 into r3;
    output r3 as public_wallet.aleo/init.future;
finalize init:
    input r0 as token.aleo/mint_public.future;
    await r0;
    
constructor:
    assert.eq edition 0u16;
