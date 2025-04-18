/*
randomness: 9086185409
keys:
  - APrivateKey1zkp8CZNn3yeCseEtxuVPbDCwSyhGW6yZKUYKfgXmcpoGPWH
cases:
    - program: program_0.aleo
      function: main
      inputs: []
    - program: program_1.aleo
      function: main
      inputs: []
    - program: program_2.aleo
      function: main
      inputs: []


*/

program program_0.aleo;

function main:
    output self.caller as address.public;
    output self.signer as address.public;

/////////////////////////////////////////////////

import program_0.aleo;

program program_1.aleo;

function main:
    call program_0.aleo/main into r0 r1;
    output r0 as address.public;
    output r1 as address.public;
    output self.caller as address.public;
    output self.signer as address.public;

/////////////////////////////////////////////////

import program_1.aleo;

program program_2.aleo;

function main:
    call program_1.aleo/main into r0 r1 r2 r3;
    output r0 as address.public;
    output r1 as address.public;
    output r2 as address.public;
    output r3 as address.public;
    output self.caller as address.public;
    output self.signer as address.public;





