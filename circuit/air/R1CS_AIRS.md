# `R1csAir` vs `R1csGateAir`

Both types lower an R1CS `Assignment` into this crate's `Air` API. They answer different questions.

| | `R1csAir` | `R1csGateAir` |
|---|---|---|
| Columns | Every public and private variable | Always 3: `(A, B, C)` |
| Rows | Always 1 | One per R1CS constraint |
| What `eval` checks | Each constraint as a polynomial **in the witness** | Only `A * B − C = 0` on that row |
| Encodes LC structure? | Yes (`A(w)`, `B(w)`, `C(w)` baked into `eval`) | No (LC values are precomputed into the trace) |
| Soundness of the R1CS instance | Complete | Incomplete on its own |

`R1csAir` is a **circuit-specific** AIR: `eval` lists this instance's constraints. `R1csGateAir` is a **uniform** AIR: the same three-column gate on every row, which is closer to a STARK-style packing.

Use `R1csAir` when the AIR itself should be a verifier of the assignment. Use `R1csGateAir` when you only need the multiplicative gates as a table (and you already trust how `A`, `B`, `C` were filled).

## Shared starting point

Gadgets still emit R1CS. After `eject_assignment_and_reset()`, you have public wires, private wires, and constraints `A · B = C` over linear combinations of those wires.

The rest of this note uses the crate's test circuit: two private field elements and their product.

```rust
let a = Field::new(Mode::Private, 3);
let b = Field::new(Mode::Private, 5);
let _product = a * b; // enforces a * b == product
```

That assignment is:

- public: `[1]` (the environment `one` at column 0)
- private: `[3, 5, 15]` (`a`, `b`, product)
- one constraint: `a * b = product`

## Example 1: `R1csAir` (witness columns, one row)

The trace is the witness laid out as a single row:

```text
columns:  one | a | b | product
row 0:      1 | 3 | 5 | 15
```

`eval` reconstructs each linear combination from those columns, then asserts the R1CS relation:

```text
local[0] == 1                         // public one
local[1] * local[2] - local[3] == 0   // 3 * 5 - 15 == 0
```

If you change the product cell to `16`, this AIR fails: `3 * 5 − 16 ≠ 0`. The constraint polynomial still names the same columns, so the AIR is checking **this circuit**, not an arbitrary multiplication.

Width grows with the number of variables. Height stays 1. Many R1CS constraints become many polynomials in `eval`, all applied to that one row.

## Example 2: `R1csGateAir` (gate table, one row per constraint)

The trace stores the **evaluated** linear combinations, not the witness:

```text
columns:  A | B | C
row 0:    3 | 5 | 15
```

`eval` is the same for every row and every circuit:

```text
local[0] * local[1] - local[2] == 0   // A * B - C == 0
```

That accepts `(3, 5, 15)`. It would also accept `(7, 2, 14)`, which is a valid multiplication but **not** the original assignment. Nothing in this AIR ties `A`, `B`, `C` back to `a`, `b`, and `product`.

With two R1CS constraints the shape difference is obvious. Suppose the circuit also enforces `a * 4 = t`:

```text
R1csAir (1 row, 5 columns)
  one | a | b | product | t
    1 | 3 | 5 |      15 | 12
  eval: a*b == product  and  a*4 == t

R1csGateAir (2 rows, 3 columns)
  A | B | C
  3 | 5 | 15     // first constraint
  3 | 4 | 12     // second constraint
  eval: A*B == C on every row
```

## Which one to call

```rust
let assignment = /* Circuit::eject_assignment_and_reset() */;

let (air, trace) = R1csAir::from_assignment(&assignment);
debug_constraints(&air, &trace)?;          // checks the instance

let (gate, gate_trace) = R1csGateAir::from_assignment(&assignment);
debug_constraints(&gate, &gate_trace)?;    // checks only A*B=C per row
```

Neither replaces Varuna. They are lowerings of the same `Assignment` after gadget synthesis.
