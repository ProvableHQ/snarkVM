program test_token.aleo;

record Token:
    owner as address.private;
    amount as u128.private;

function transfer_from_public:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as u128.public;
    async transfer_from_public r0 r1 r2 into r3;
    output r3 as test_token.aleo/transfer_from_public.future;

finalize transfer_from_public:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as u128.public;
    assert.eq true true;

function transfer_public:
    input r0 as address.public;
    input r1 as u128.public;
    async transfer_public r0 r1 into r2;
    output r2 as test_token.aleo/transfer_public.future;

finalize transfer_public:
    input r0 as address.public;
    input r1 as u128.public;
    assert.eq true true;

function transfer_public_to_private:
    input r0 as address.private;
    input r1 as u128.public;
    cast r0 r1 into r2 as Token.record;
    async transfer_public_to_private r1 into r3;
    output r2 as Token.record;
    output r3 as test_token.aleo/transfer_public_to_private.future;

finalize transfer_public_to_private:
    input r0 as u128.public;
    assert.eq true true;

function transfer_private_to_public:
    input r0 as Token.record;
    input r1 as address.private;
    input r2 as u128.private;
    cast r0.owner r0.amount into r3 as Token.record;
    cast r1 0u128 into r4 as Token.record;
    async transfer_private_to_public into r5;
    output r3 as Token.record;
    output r4 as Token.record;
    output r5 as test_token.aleo/transfer_private_to_public.future;

finalize transfer_private_to_public:
    assert.eq true true;
