/*
start_height: 0
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
    set true into foo[r0];

