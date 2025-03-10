/*
randomness: 489234981
cases:
  - program: check_metadata_v2.aleo
    function: test_checksum
    inputs: []
  - program: check_metadata_v2.aleo
    function: test_metadata
    inputs: []
  - program: check_metadata_v2.aleo
    function: test_checksum_invalid
    inputs: []
  - program: check_metadata_v2.aleo
    function: test_metadata_does_not_exist
    inputs: []
*/


program$2 child.aleo;

function foo:

$metadata program_owner: aleo1rhgdu77hgyqd3xjj8ucu3jj9r2krwz6mnzyd80gncr5fxcwlh5rsvzp9px;
$metadata edition: 0u16;
$metadata upgradable: true;

/////////////////////////////////////////////////

import credits.aleo;
import child.aleo;

program$2 check_metadata_v2.aleo;

function test_checksum:
    async test_checksum into r0;
    output r0 as check_metadata_v2.aleo/test_checksum.future;

finalize test_checksum:
    metadata.get _checksum into r0 as u128;
    metadata.get credits.aleo/_checksum into r1 as u128;
    metadata.get child.aleo/_checksum into r2 as u128;

function test_metadata:
    async test_metadata into r0;
    output r0 as check_metadata_v2.aleo/test_metadata.future;

finalize test_metadata:
    metadata.get program_owner into r0 as address;
    metadata.get edition into r1 as u16;
    metadata.get upgradable into r2 as boolean;
    metadata.get child.aleo/program_owner into r3 as address;
    metadata.get child.aleo/edition into r4 as u16;
    metadata.get child.aleo/upgradable into r5 as boolean;
    assert.eq r0 r3;
    assert.eq r1 r4;
    assert.eq r2 r5;

function test_checksum_invalid:
    async test_checksum_invalid into r0;
    output r0 as check_metadata_v2.aleo/test_checksum_invalid.future;

finalize test_checksum_invalid:
    metadata.get _checksum into r0 as i128;

function test_metadata_does_not_exist:
    async test_metadata_does_not_exist into r0;
    output r0 as check_metadata_v2.aleo/test_metadata_does_not_exist.future;

finalize test_metadata_does_not_exist:
    metadata.get foo into r0 as i128;

$metadata program_owner: aleo1rhgdu77hgyqd3xjj8ucu3jj9r2krwz6mnzyd80gncr5fxcwlh5rsvzp9px;
$metadata edition: 0u16;
$metadata upgradable: true;

