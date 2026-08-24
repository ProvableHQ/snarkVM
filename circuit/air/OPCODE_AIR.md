# Per-opcode R1CS to AIR proof of concept

## Is global R1CS access really a problem?

Yes, if the compiler starts from only a global R1CS instance.

R1CS variables live in one unordered witness vector. Any constraint can use any
variable. A narrow row-oriented AIR cannot evaluate those linear combinations
without adding one of:

- witness-memory accesses plus a permutation argument;
- a wide matrix/selector table;
- or repeated scans of the global witness.

`R1csAir` avoids memory by putting the entire witness in one very wide row. That
is mechanically sound, but not a competitive AIR for a large function.
`R1csGateAir` uses narrow `(A, B, C)` rows, but does not prove that those values
are the linear combinations from the R1CS witness.

## What changes at the opcode boundary?

An opcode has bounded arity and a bounded local computation. For a fixed opcode,
literal type, and input mode:

- its local R1CS shape is reusable across invocations;
- one invocation can occupy one AIR row containing its local witness;
- the opcode's R1CS equations become local row constraints;
- only input/output equality remains to compose opcode rows.

This turns arbitrary global witness access into a wiring problem.

snarkVM makes that wiring easier because function registers are SSA-like:
destinations are monotonically allocated and cannot be overwritten. A production
compiler can therefore link a producer event `(register_id, value)` to every
consumer event without implementing mutable RAM semantics.

The remaining wiring is still real, but compiler-generated variables provide an
additional implementation strategy:

- fixed adjacent dataflow can use ordinary `local`/`next` transition constraints;
- non-adjacent SSA values can be copied through compiler-generated carry
  variables on each intervening boundary;
- long or highly overlapping live ranges may be cheaper through a
  copy/permutation bus;
- separate opcode AIRs need a cross-AIR interaction argument.

Thus a memory/permutation argument is not mandatory. A compiler can route every
wire through adjacent rows. The tradeoff is that the number of copy cells is
proportional to the sum of register live-range lengths. Interval-coloring carry
lanes can reduce width to the maximum number of simultaneously live values.

The last option follows Plonky3's current architecture: AIRs emit messages via an
[`InteractionBuilder`](https://github.com/Plonky3/Plonky3/blob/main/lookup/src/builder.rs),
and lookup/permutation machinery balances matching messages. Its
[`AirBuilder`](https://github.com/Plonky3/Plonky3/blob/main/air/src/builder.rs)
provides current/next windows and filtered transition constraints.

## Proof of concept

`OpcodeR1csAir` compiles several isolated `Assignment`s of one opcode shape. The
test inserts an extra assignment that acts as a compiler-generated copy row:

```text
isolated mul invocation 0: private [lhs=2, rhs=3, out=6]
generated carry row:       private [lhs=6, rhs=1, out=6]
isolated mul invocation 1: private [lhs=6, rhs=4, out=24]
```

Each assignment has the same local R1CS shape:

```text
lhs * rhs = out
```

The compiled trace has one complete local witness per row:

```text
one | lhs | rhs | out
  1 |   2 |   3 |   6
  1 |   6 |   1 |   6   <- compiler-generated variables carry the value
  1 |   6 |   4 |  24
```

The AIR applies `lhs * rhs - out = 0` on every row. An explicit
`TransitionLink` additionally applies:

```text
local.out - next.lhs = 0
```

The test changes the carry row to the independently valid copy `7 * 1 = 7`.
Every row still satisfies the opcode's local R1CS, but the linked AIR rejects
because the routed value changes from `6` to `7`.

```rust
let link = TransitionLink::new(
    OpcodeColumn::Private(2), // current output
    OpcodeColumn::Private(0), // next left input
);

let (air, trace) = OpcodeR1csAir::from_assignments(
    &[first_mul_assignment, second_mul_assignment],
    &[link],
)?;
debug_constraints(&air, &trace)?;
```

See `test_opcode_air_links_consecutive_multiplications` in
[`src/r1cs.rs`](src/r1cs.rs).

## What this proves—and what it does not

The proof of concept establishes that per-opcode synthesis can replace global
witness columns with a bounded-width, ordered trace when the dataflow is fixed.
Non-adjacent dataflow can be made adjacent by inserting copy variables or rows.

It does not yet compile a complete Aleo function:

1. `Instruction::execute` currently appends constraints to one global
   environment; it does not emit isolated opcode assignments.
2. Different literal types and modes can produce different R1CS shapes and must
   be separate opcode variants.
3. Carrying every non-adjacent value through intermediate boundaries may be too
   expensive; a register bus or copy permutation is the scalable alternative.
4. Cryptographic kernels such as Poseidon should generally use native AIRs
   rather than mechanically translated R1CS.

The production direction is therefore:

1. synthesize/cache one local R1CS template per opcode shape;
2. generate one local witness row per invocation;
3. group rows by opcode shape (Plonky3-style chips/tables);
4. initially route SSA values through interval-colored carry lanes;
5. if copy overhead dominates, emit producer and consumer messages
   `(register_id, value)` on a shared register bus and prove that the bus
   multiset balances with a permutation/lookup argument.

Per-opcode compilation substantially improves the problem, especially with
snarkVM's immutable registers. It does not make cross-opcode consistency
disappear; it converts that consistency into either explicit copy routing or the
standard AIR interaction problem.
