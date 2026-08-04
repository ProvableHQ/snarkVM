/*
cases: []
*/

program registry.aleo;

mapping users:
    key as address.public;
    value as boolean.public;

function register:
    async register self.caller into r0;
    output r0 as registry.aleo/register.future;

finalize register:
    input r0 as address.public;
    set true into users[r0];
    
constructor:
    assert.eq edition 0u16;


/////////////////////////////////////////////////

program relay.aleo;

function send:
    input r0 as address.public;
    async send r0 into r1;
    output r1 as relay.aleo/send.future;

finalize send:
    input r0 as address.public;
    get registry.aleo/users[r0] into r1;
    assert.eq r1 true;
    
constructor:
    assert.eq edition 0u16;
