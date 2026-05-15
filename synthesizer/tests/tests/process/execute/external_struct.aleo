/*
randomness: 45791624
cases:
  - program: child.aleo
    function: call_make_array
    inputs: []
  - program: child.aleo
    function: make_s
    inputs: []
  - program: child.aleo
    function: make_t
    inputs: []
  - program: child.aleo
    function: call_make_outer
    inputs: []
  - program: child.aleo
    function: pass_s
    inputs: []
  - program: child.aleo
    function: pass_t
    inputs: []
  - program: child.aleo
    function: cast_s
    inputs: []
  - program: child2.aleo
    function: call_make_outer2
    inputs: []
  - program: child2.aleo
    function: call_make_outer2_access
    inputs: []
*/

program external_struct_parent.aleo;

struct S:
    x as field;

struct Outer:
    x as field;
    s as S;

function make_s:
    cast 12field into r0 as S;
    output r0 as S.public;

function make_outer:
    input r0 as field.public;
    cast r0 into r1 as S;
    cast r0 r1 into r2 as Outer;
    output r2 as Outer.public;

function make_array:
    input r0 as field.public;
    cast r0 into r1 as S;
    cast r1 r1 into r2 as [S; 2u32];
    output r2 as [S; 2u32].public;

function accept_s:
    input r0 as S.public;
    output r0.x as field.public;

/////////////////////////////////////////////////

import external_struct_parent.aleo;

program child.aleo;

struct S:
    x as field;

struct Outer:
    x as field;
    s as S;

struct Outer2:
    s as external_struct_parent.aleo/S;

function make_s:
    call external_struct_parent.aleo/make_s into r0;
    output r0 as external_struct_parent.aleo/S.public;

function make_t:
    call external_struct_parent.aleo/make_s into r0;
    output r0 as S.public;

function call_make_outer:
    call external_struct_parent.aleo/make_outer 3field into r0;
    output r0 as Outer.public;

function call_make_array:
    call external_struct_parent.aleo/make_array 4field into r0;
    output r0 as [S; 2u32].public;

function pass_s:
    cast 1field into r0 as external_struct_parent.aleo/S;
    call external_struct_parent.aleo/accept_s r0 into r1;
    output r1 as field.public;

function pass_t:
    cast 2field into r0 as S;
    call external_struct_parent.aleo/accept_s r0 into r1;
    output r1 as field.public;

function make_outer2:
    cast 100field into r0 as external_struct_parent.aleo/S;
    cast r0 into r1 as Outer2;
    output r1 as Outer2.public;

function cast_s:
    cast 101field into r0 as external_struct_parent.aleo/S;
    output r0 as external_struct_parent.aleo/S.public;

/////////////////////////////////////////////////

import child.aleo;
import external_struct_parent.aleo;

program child2.aleo;

function call_make_outer2:
    call child.aleo/make_outer2 into r0;
    output r0 as child.aleo/Outer2.public;

function call_make_outer2_access:
    call child.aleo/make_outer2 into r0;
    output r0.s as external_struct_parent.aleo/S.public;
