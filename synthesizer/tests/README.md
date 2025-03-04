# Using the Test Framework

This directory contains a set of utilities and integration tests for various components of `synthesizer` crate. The test framework is based loosely off ideas from [Characterization Testing](https://en.wikipedia.org/wiki/Characterization_test) (you have have heard these referred to as "golden tests"). At a high level, the framework runs a set of integration tests and compares their output to their corresponding expected output. If the output does not match, the framework returns an error.

Don't expect a change in RNG state to necessarily lead to a change in outputs, because certain cryptographic objects like field elements may require a different amount of iterated sampling, offsetting initial changes in RNG state. 

Some important files/folders are:
- `expectations` | The expectation files for each of the test categories.
- `tests` | The test files.
- `utilities` | Utilities for parsing test files and checking test outputs.
- `test_instruction_parse.rs` | A test runner that runs `Instruction::parse` on each line of input in `./tests/parser/instruction`. It attempts to parse each input as an `Instruction` and reports either:
    -  `Parsing was successful.`
    -  Or the errors produced by the parser.

- `test_program_parse.rs` |  A test runner that runs `Program::parse` on each file in `./tests/parser/program`. It attempts to parse each input as an `Program` and reports either:
    -  `Parsing was successful.`
    -  Or the errors produced by the parser.
-  `test_process_execute.rs` | A test runner that runs `Process::execute` on each file in `./tests/program` and checks the output against the corresponding execution file. Note that this test does not verify the execution.
-  `test_vm_execute_and_finalize.rs` | A test runner that loads a test program, initializes a VM, runs `VM::execute`, `VM::speculate`, and `VM::add_next_block` on each test case.

## Anatomy of a Test

The tests in `./tests/program` are structured in a very specific way. Let's take a look!

```
/*
randomness: 1337
cases:
  - function: mint
    inputs: [aleo1j2hfs6yru47h2nvsjdefwtw6nwaj0y4zcl02juyy29txm7nt6y9qln7uhp, 100u64]
  - function: split
    inputs: [ "{ owner: aleo1j2hfs6yru47h2nvsjdefwtw6nwaj0y4zcl02juyy29txm7nt6y9qln7uhp.private, microcredits: 100u64.private, _nonce: 0group.public }", 50u64]
    private_key: APrivateKey1zkpFbGDx4znwxo1zrxfUscfGn1Vy3My3ia5gRHx3XwaLtCR
  - function: split
    inputs: [ "{ owner: aleo1j2hfs6yru47h2nvsjdefwtw6nwaj0y4zcl02juyy29txm7nt6y9qln7uhp.private, microcredits: 100u64.private, _nonce: 0group.public }", 50u64]
    private_key: APrivateKey1zkpJhviKDvvm7yu7SZuhSudVR7zjCRG2HznuAHwuGYc1xqN
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
```

The first component of a test program is the configuration. The config is defined as a YAML object in the first comment in the test file.
At the top-level of the config, a user can define:
- `randomness`. A random `u64` to seed the test.
- `cases`. The set of test cases.

Each test case contains:
- `function`. The name of the function to run.
- `inputs`. The inputs to run the function with.
- `private_key`. The private key with which to run the test. (Optional)

## Running the Tests

- To run the tests, use the following command.
```
cargo test --test '*' 
```
- To run the tests with filters, use the following command. This will only run test files that contain `query` in their name. This is recommended when running specific tests, since `vm_execute_and_finalize` can take a long time.
```
TEST_FILTER=<query> cargo test --test '*' 
```
- To regenerate the expectation files, use the following command. This can be stacked with the `TEST_FILTER` flag to regenerate expectations for a specific test.
```
REWRITE_EXPECTATIONS=1 cargo test --test '*' 
```
- To run a specific set of tests, run the following command. This can be stacked with the flags above.
```
cargo test --test '<namespace>'

// Example
cargo test --test 'test_program_parse'

```
