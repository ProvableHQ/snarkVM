/*
randomness: 9086185409
keys:
  - APrivateKey1zkpABon5degxuW8JnBniSXgN1C4eAGKfDH8qRPZe1geHpWp
cases:
  - program: relay.aleo
    function: send
    inputs: [aleo1f6eg623knp66cwx0926w3plgdgzcmfpgyrzgnjz90mucgs3z7s9qls4upm, 0u8]
  - program: relay.aleo
    function: send_without_check
    inputs: [aleo1f6eg623knp66cwx0926w3plgdgzcmfpgyrzgnjz90mucgs3z7s9qls4upm, 0u8]
  - program: relay.aleo
    function: check_has_registered
    inputs: [aleo1f6eg623knp66cwx0926w3plgdgzcmfpgyrzgnjz90mucgs3z7s9qls4upm]
  - program: registry.aleo
    function: register
    inputs: []
    private_key: APrivateKey1zkpABon5degxuW8JnBniSXgN1C4eAGKfDH8qRPZe1geHpWp
  - program: relay.aleo
    function: check_has_registered
    inputs: [aleo1f6eg623knp66cwx0926w3plgdgzcmfpgyrzgnjz90mucgs3z7s9qls4upm]
  - program: relay.aleo
    function: send
    inputs: [aleo1f6eg623knp66cwx0926w3plgdgzcmfpgyrzgnjz90mucgs3z7s9qls4upm, 1u8]
  - program: registry.aleo
    function: unregister
    inputs: []
    private_key: APrivateKey1zkpABon5degxuW8JnBniSXgN1C4eAGKfDH8qRPZe1geHpWp
  - program: relay.aleo
    function: send
    inputs: [aleo1f6eg623knp66cwx0926w3plgdgzcmfpgyrzgnjz90mucgs3z7s9qls4upm, 2u8]
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

function unregister:
    async unregister self.caller into r0;
    output r0 as registry.aleo/unregister.future;

finalize unregister:
    input r0 as address.public;
    set false into users[r0];
    
constructor:
    assert.eq edition 0u16;

/////////////////////////////////////////////////

import registry.aleo;

program relay.aleo;

record message:
    owner as address.private;
    data as u8.private;

function check_has_registered:
    input r0 as address.public;
    async check_has_registered r0 into r1;
    output r1 as relay.aleo/check_has_registered.future;

finalize check_has_registered:
    input r0 as address.public;
    contains registry.aleo/users[r0] into r1;
    assert.eq r1 true;

function send:
    input r0 as address.public;
    input r1 as u8.public;
    cast r0 r1 into r2 as message.record;
    async send r0 into r3;
    output r2 as message.record;
    output r3 as relay.aleo/send.future;

finalize send:
    input r0 as address.public;
    get registry.aleo/users[r0] into r1;
    assert.eq r1 true;

function send_without_check:
    input r0 as address.public;
    input r1 as u8.public;
    cast r0 r1 into r2 as message.record;
    async send_without_check r0 into r3;
    output r2 as message.record;
    output r3 as relay.aleo/send_without_check.future;

finalize send_without_check:
    input r0 as address.public;
    get.or_use registry.aleo/users[r0] true into r1;
    assert.eq r1 true;

constructor:
    assert.eq edition 0u16;