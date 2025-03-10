/*
randomness: 3457689
cases:
  - program: check_metadata_v1.aleo
    function: test_checksum
    inputs: []
  - program: check_metadata_v1.aleo
    function: test_checksum_invalid
    inputs: []
  - program: check_metadata_v1.aleo
    function: test_metadata_does_not_exist
    inputs: []
*/

import credits.aleo;

program check_metadata_v1.aleo;

function test_checksum:
    async test_checksum into r0;
    output r0 as check_metadata_v1.aleo/test_checksum.future;

finalize test_checksum:
    metadata.get _checksum into r0 as u128;
    metadata.get credits.aleo/_checksum into r1 as u128;

function test_checksum_invalid:
    async test_checksum_invalid into r0;
    output r0 as check_metadata_v1.aleo/test_checksum_invalid.future;

finalize test_checksum_invalid:
    metadata.get _checksum into r0 as i128;

function test_metadata_does_not_exist:
    async test_metadata_does_not_exist into r0;
    output r0 as check_metadata_v1.aleo/test_metadata_does_not_exist.future;

finalize test_metadata_does_not_exist:
    metadata.get foo into r0 as i128;


