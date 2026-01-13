program child.aleo;

function child:
    call parent.aleo/parent into r0;
    output r0 as parent.aleo/S.public;

function child2:
    input r0 as parent.aleo/S.public;
    output r0 as parent.aleo/S.public;
