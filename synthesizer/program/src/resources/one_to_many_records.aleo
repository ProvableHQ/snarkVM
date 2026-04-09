program batch_one_to_many_records.aleo;

record test:
    owner as address.private;
    amount as u64.private;

function mint:
    input r0 as u64.private;
    cast self.caller r0 into r1 as test.record;
    output r1 as test.record;

function one_to_many_records:
    input r0 as u64.private;
    input r1 as address.private;
    input r2 as u64.private;
    cast r1 r2 into r3 as test.record;
    cast r1 r2 into r4 as test.record;
    cast r1 r2 into r5 as test.record;
    cast r1 r2 into r6 as test.record;
    cast r1 r2 into r7 as test.record;
    cast r1 r2 into r8 as test.record;
    cast r1 r2 into r9 as test.record;
    cast r1 r2 into r10 as test.record;
    cast r1 r2 into r11 as test.record;
    cast r1 r2 into r12 as test.record;
    cast r1 r2 into r13 as test.record;
    cast r1 r2 into r14 as test.record;
    cast r1 r2 into r15 as test.record;
    cast r1 r2 into r16 as test.record;
    cast r1 r2 into r17 as test.record;
    cast r1 r2 into r18 as test.record;
    output r3 as test.record;
    output r4 as test.record;
    output r5 as test.record;
    output r6 as test.record;
    output r7 as test.record;
    output r8 as test.record;
    output r9 as test.record;
    output r10 as test.record;
    output r11 as test.record;
    output r12 as test.record;
    output r13 as test.record;
    output r14 as test.record;
    output r15 as test.record;
    output r16 as test.record;
    output r17 as test.record;
    output r18 as test.record;