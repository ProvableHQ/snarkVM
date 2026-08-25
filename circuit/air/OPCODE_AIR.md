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
    &[first_mul_assignment, carry_assignment, second_mul_assignment],
    &[link],
)?;
debug_constraints(&air, &trace)?;
```

See `test_opcode_air_links_consecutive_multiplications` in
[`src/r1cs.rs`](src/r1cs.rs).

## Architectural comparison with Plonky3

The two systems can use the same high-level composition model, but they start at
different abstraction levels.

snarkVM gadgets currently execute against `Environment` and append equations to
one heterogeneous R1CS. Shared R1CS variables compose gadgets implicitly.
`OpcodeR1csAir` recovers AIR structure by isolating a reusable R1CS shape for
each opcode variant, assigning one invocation per row, and making dataflow
explicit with carry columns or transition links.

Plonky3 AIRs are normally written directly as trace layouts and polynomial
constraints. A specialized chip can choose whether rounds occupy rows or
columns, which intermediates are witnessed, and how polynomial degree is
reduced. Separate chips compose through shared columns, transition constraints,
or lookup/permutation interactions.

Therefore, different snarkVM opcode AIRs can be composed Plonky3-style. The
compiler must additionally:

1. choose and populate an AIR table for each opcode shape;
2. bind public inputs and outputs;
3. route SSA values through adjacent carry lanes or a cross-table bus; and
4. constrain table multiplicities and control flow.

Plonky3's bare `Poseidon2Air` does not itself expose public inputs or emit
cross-table interactions. A wrapper AIR must bind or bus-connect its input and
output columns when composing it with the rest of a program.

Compiler-generated variables between opcode invocations are sufficient for
correctness. Their cost is proportional to live-range length, so a bus becomes
preferable when many values have long or overlapping live ranges.

## Poseidon AIR size benchmark

This benchmark measures structural AIR size, not proving time or proof bytes.
It counts committed main-trace field elements, symbolic constraint polynomials,
and maximum polynomial degree for one permutation row or segment.

The snarkVM case is a full-rate `Poseidon2<Circuit>::hash` of two private field
elements. In snarkVM, `Poseidon2` means classic Poseidon with rate 2; it does not
mean the Poseidon2 permutation design. Its sponge has a three-word state, eight
full rounds, 31 partial rounds, and an `x^17` S-box. Constant-only work is folded
away, leaving one nonconstant permutation with two variable rate words and a
fixed capacity word. The assignment is lowered as one `OpcodeR1csAir` row. It is
therefore a hash-block measurement, not an arbitrary three-word raw permutation.

Measured with:

```text
cargo test -p snarkvm-synthesizer-snark test_poseidon_hash_air_size_report -- --nocapture
```

The result is:

- `OpcodeR1csAir`: 273 columns, one row, 273 committed cells, 270 R1CS
  multiplication constraints, 271 AIR constraints including `one = 1`, and
  maximum degree 2.
- Native round-oriented `PoseidonAir`: three main columns, four preprocessed
  columns, 40 rows, 120 committed cells, 160 fixed cells, three symbolic
  transition constraints, and maximum symbolic degree 19.

For Plonky3, the comparison uses its standard BabyBear Poseidon2 AIR at commit
[`5dc50e5`](https://github.com/Plonky3/Plonky3/tree/5dc50e51436a811c62443e336f766015fecc9217):
a 16-word state, eight full rounds, 13 partial rounds, and an `x^7` S-box with
one auxiliary register. Plonky3 places one complete permutation in each row.
Its [`Poseidon2Cols`](https://github.com/Plonky3/Plonky3/blob/5dc50e51436a811c62443e336f766015fecc9217/poseidon2-air/src/columns.rs)
layout gives:

```text
16 inputs
+ 8 full rounds * (16 S-box registers + 16 post-state cells)
+ 13 partial rounds * (1 S-box register + 1 post-S-box cell)
= 298 columns
```

Symbolically evaluating the pinned
[`Poseidon2Air`](https://github.com/Plonky3/Plonky3/blob/5dc50e51436a811c62443e336f766015fecc9217/poseidon2-air/src/air.rs)
gives 282 constraints and maximum degree 3. One permutation therefore occupies
298 committed cells. Its vectorized eight-permutation AIR changes width and
height but retains 298 cells per permutation.

The absolute row sizes are close: Plonky3 uses 9.2% more committed cells and
4.1% more constraints per permutation. That is not an apples-to-apples
efficiency result: Plonky3's permutation has 16 state words, while snarkVM's has
three. Normalized per state word, the opcode lowering uses 91 cells and 90.3 AIR
constraints; Plonky3 uses 18.6 cells and 17.6 constraints. The specialized
Plonky3 layout is therefore about 4.9 times smaller in cells and 5.1 times
smaller in constraints per state word.

The native snarkVM AIR demonstrates the other side of the width/degree
tradeoff. It has only 120 committed cells, but directly embeds `x^17` and uses a
preprocessed full-round selector. Its raw S-box expression has degree 17; the
complete symbolic transition expression has degree 19 after the full-round and
transition selectors are included. A production native AIR should witness
intermediate powers to reduce degree and add segment selectors or a
one-permutation-per-row layout for batching.

These ratios must not be interpreted as proof-size or prover-time ratios.
snarkVM uses a roughly 253-bit field and classic Poseidon parameters; the
Plonky3 instance uses the 31-bit BabyBear field and Poseidon2 parameters. A fair
performance benchmark requires the same field, state width, permutation,
security target, commitment scheme, and batch size.

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
