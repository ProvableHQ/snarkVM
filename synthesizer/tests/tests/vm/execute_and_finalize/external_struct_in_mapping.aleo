/*
start_height: 19
randomness: 45791624
cases:
  - program: parent.aleo
    function: store_t
    inputs: []
  - program: parent.aleo
    function: store_s
    inputs: []
  - program: parent.aleo
    function: store_s_as_t
    inputs: []
  - program: parent.aleo
    function: baz_keys
    inputs: []
  - program: parent.aleo
    function: compare
    inputs: []
*/

program child.aleo;

struct S:
    x as field;

function make_s:
    cast 12field into r0 as S;
    output r0 as S.public;

constructor:
    assert.eq edition 0u16;

/////////////////////////////////////////////////

import child.aleo;

program parent.aleo;

struct S:
    x as field;

mapping foo:
    key as field.public;
    value as S.public;

mapping bar:
    key as field.public;
    value as child.aleo/S.public;

mapping baz:
    key as S.public;
    value as field.public;

closure make_t:
    input r0 as field;
    cast r0 into r1 as S;
    output r1 as S;

function store_t:
    call make_t 12field into r0;
    async store_t r0 into r1;
    output r1 as parent.aleo/store_t.future;
finalize store_t:
    input r0 as S.public;
    set r0 into foo[0field];

function store_s:
    call child.aleo/make_s into r0;
    async store_s r0 into r1;
    output r1 as parent.aleo/store_s.future;
finalize store_s:
    input r0 as child.aleo/S.public;
    set r0 into bar[1field];

function store_s_as_t:
    call child.aleo/make_s into r0;
    async store_s_as_t r0 into r1;
    output r1 as parent.aleo/store_s_as_t.future;
finalize store_s_as_t:
    input r0 as child.aleo/S.public;
    cast r0.x into r1 as S;
    set r1 into foo[2field];

function baz_keys:
    call child.aleo/make_s into r0;
    async baz_keys r0 into r1;
    output r1 as parent.aleo/baz_keys.future;
finalize baz_keys:
    input r0 as child.aleo/S.public;
    set 1field into baz[r0];

function compare:
    async compare into r0;
    output r0 as parent.aleo/compare.future;
finalize compare:
    get foo[0field] into r0;
    get bar[1field] into r1;
    assert.eq r0.x r1.x;


constructor:
    assert.eq edition 0u16;

