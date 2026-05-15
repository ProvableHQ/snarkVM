/*
randomness: 1337
keys:
  - APrivateKey1zkpFbGDx4znwxo1zrxfUscfGn1Vy3My3ia5gRHx3XwaLtCR
  - APrivateKey1zkpJhviKDvvm7yu7SZuhSudVR7zjCRG2HznuAHwuGYc1xqN
cases:
  - program: mint_and_split.aleo
    function: mint
    inputs: [aleo1j2hfs6yru47h2nvsjdefwtw6nwaj0y4zcl02juyy29txm7nt6y9qln7uhp, 100u64]
  - program: mint_and_split.aleo
    function: split
    inputs: [ "{ owner: aleo1j2hfs6yru47h2nvsjdefwtw6nwaj0y4zcl02juyy29txm7nt6y9qln7uhp.private, microcredits: 100u64.private, _nonce: 0group.public }", 50u64]
    private_key: APrivateKey1zkpFbGDx4znwxo1zrxfUscfGn1Vy3My3ia5gRHx3XwaLtCR
  - program: mint_and_split.aleo
    function: split
    inputs: [ "{ owner: aleo1j2hfs6yru47h2nvsjdefwtw6nwaj0y4zcl02juyy29txm7nt6y9qln7uhp.private, microcredits: 100u64.private, _nonce: 0group.public }", 50u64]
    private_key: APrivateKey1zkpJhviKDvvm7yu7SZuhSudVR7zjCRG2HznuAHwuGYc1xqN
  - program: mint_and_split.aleo
    function: split
    inputs: [ "{ owner: aleo1j2hfs6yru47h2nvsjdefwtw6nwaj0y4zcl02juyy29txm7nt6y9qln7uhp.private, microcredits: 100u64.private, _nonce: 4745749250363947745807119124444606639539806979883854068889616498365610280135group.public, _version: 1u8.public }", 50u64]
    private_key: APrivateKey1zkpFbGDx4znwxo1zrxfUscfGn1Vy3My3ia5gRHx3XwaLtCR
*/

program mint_and_split.aleo;

record credits:
    owner as address.private;
    microcredits as u64.private;

function mint:
    input r0 as address.public;
    input r1 as u64.public;
    cast r0 r1 into r2 as credits.record;
    output r2 as credits.record;

function split:
    input r0 as credits.record;
    input r1 as u64.private;
    sub r0.microcredits r1 into r2;
    cast r0.owner r1 into r3 as credits.record;
    cast r0.owner r2 into r4 as credits.record;
    output r3 as credits.record;
    output r4 as credits.record;
    
constructor:
    assert.eq edition 0u16;
