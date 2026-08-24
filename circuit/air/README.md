# snarkvm-circuit-air

Plonky3-style AIR types for snarkVM.

This crate is a **parallel** arithmetization surface. The existing circuit gadgets remain an R1CS EDSL (`Environment::enforce` is still `A * B = C`). AIR is not implemented by swapping `Environment`.

- `Air` / `AirBuilder` / `Trace` — uniform row-constraint API (local / next / `assert_zero` / `when`)
- `R1csAir` / `R1csGateAir` — two different lowerings of an `Assignment` after gadget synthesis ([how they differ](R1CS_AIRS.md))
- `OpcodeR1csAir` — one bounded local witness per opcode row with explicit transition wiring ([proof of concept](OPCODE_AIR.md))
- `PoseidonAir` — native round-based Poseidon permutation AIR

Existing R1CS constraint counts, `Assignment` layout, and Varuna proving are unchanged.
