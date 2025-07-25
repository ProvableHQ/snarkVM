/*
start_height: 0
cases:
  - program: unused_position.aleo
    function: foo
    inputs: []
*/

program unused_position.aleo;

function foo:
    async foo into r0;
    output r0 as unused_position.aleo/foo.future;

finalize foo:
    position end;
