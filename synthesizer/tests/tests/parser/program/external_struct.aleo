program child.aleo;

mapping refers_to_external_struct:
    key as parent.aleo/SomeStruct.public;
    value as parent.aleo/AnotherStruct.public;

struct my_struct:
    member as parent.aleo/S;
    member2 as parent.aleo/S2;

function child:
    call parent.aleo/parent into r0;
    output r0 as parent.aleo/S.public;

function child2:
    input r0 as parent.aleo/S.public;
    output r0 as parent.aleo/S.public;
