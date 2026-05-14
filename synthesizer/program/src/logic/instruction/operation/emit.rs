// Copyright (c) 2019-2026 Provable Inc.
// This file is part of the snarkVM library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:

// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{Opcode, Operand, RegistersCircuit, RegistersTrait, StackTrait};
use console::{
    network::prelude::*,
    program::{Register, RegisterType},
};
use snarkvm_synthesizer_error::*;

// Per-thread capture buffer for emitted plaintext strings. Populated only under
// `#[cfg(test)]` or with `--features test`, so production builds carry no overhead
// and never accumulate state. Tests call `drain_recent_emits()` after the
// prover-side execution they want to inspect to read back the captured data.
#[cfg(any(test, feature = "test"))]
std::thread_local! {
    static EMIT_LOG: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Drains the current thread's captured emit output strings. Returns an empty Vec
/// in production builds (without the `test` feature) since capture is disabled there.
#[cfg(any(test, feature = "test"))]
pub fn drain_recent_emits() -> Vec<String> {
    EMIT_LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

#[cfg(any(test, feature = "test"))]
fn capture_emit(s: &str) {
    EMIT_LOG.with(|log| log.borrow_mut().push(s.to_string()));
}

#[cfg(not(any(test, feature = "test")))]
fn capture_emit(_s: &str) {}

/// A debug-emit instruction for use inside transition function bodies (circuit context).
///
/// `emit r0;` — prints the resolved plaintext to stderr at speculation/execution time,
/// adds zero constraints to the circuit, and never reaches the verifier. Prover-side
/// only, always-on (no feature gate) — the dev who wrote `emit` is asking for output.
///
/// Note: in finalize bodies, the `emit` surface keyword parses to `Command::Emit` (which
/// produces a structured `FinalizeOperation::EmitEvent`), not this instruction. The
/// parser tries `Command::Emit` before falling through to `Instruction`, so the two paths
/// don't collide in practice.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EmitLog<N: Network> {
    /// The operand whose plaintext value is printed.
    operands: [Operand<N>; 1],
}

impl<N: Network> EmitLog<N> {
    /// Returns the opcode.
    #[inline]
    pub const fn opcode() -> Opcode {
        Opcode::Emit("emit")
    }

    /// Returns the operands in the operation.
    #[inline]
    pub fn operands(&self) -> &[Operand<N>] {
        &self.operands
    }

    /// Returns the operand whose value is printed.
    #[inline]
    pub const fn value(&self) -> &Operand<N> {
        &self.operands[0]
    }

    /// Returns the destination registers (none).
    #[inline]
    pub fn destinations(&self) -> Vec<Register<N>> {
        vec![]
    }

    /// Returns whether this instruction refers to an external struct.
    #[inline]
    pub fn contains_external_struct(&self) -> bool {
        false
    }
}

impl<N: Network> EmitLog<N> {
    /// Evaluates the instruction: resolves the plaintext, prints to stderr, and (under
    /// `--features test` / `#[cfg(test)]`) records the formatted value into the per-thread
    /// capture buffer for test assertions. The reject-future / reject-record behavior is
    /// delegated to `load_plaintext`.
    pub fn evaluate(
        &self,
        stack: &impl StackTrait<N>,
        registers: &mut impl RegistersTrait<N>,
    ) -> Result<(), EvalError> {
        let plaintext = registers.load_plaintext(stack, &self.operands[0])?;
        let formatted = plaintext.to_string();
        eprintln!("{formatted}");
        capture_emit(&formatted);
        Ok(())
    }

    /// Executes the instruction in circuit context. Adds zero constraints — the verifier
    /// never sees this instruction's effect. The operand is loaded only to validate it
    /// resolves to a plaintext; no print / capture happens here, since `vm.execute` runs
    /// the evaluate path first which already handles the developer-facing surface. This
    /// avoids the double-print users would otherwise see (once from evaluate, once from
    /// execute) for a single source-level `emit`.
    pub fn execute<A: circuit::Aleo<Network = N>>(
        &self,
        stack: &impl StackTrait<N>,
        registers: &mut impl RegistersCircuit<N, A>,
    ) -> Result<(), ExecError> {
        let _ = registers.load_plaintext_circuit(stack, &self.operands[0])?;
        Ok(())
    }

    /// Finalizes the instruction. In practice, `emit` in a finalize body is parsed as
    /// `Command::Emit` (the structured-event path), so this method is unreachable via
    /// parsing. Kept as a no-print fallback for programmatic constructions.
    #[inline]
    pub fn finalize(
        &self,
        stack: &impl StackTrait<N>,
        registers: &mut impl RegistersTrait<N>,
    ) -> Result<(), FinalizeError> {
        self.evaluate(stack, registers)?;
        Ok(())
    }

    /// Returns no output types — `emit` produces no register destinations. The operand
    /// must resolve to a plaintext type (validated at runtime by `load_plaintext`).
    pub fn output_types(
        &self,
        _stack: &impl StackTrait<N>,
        input_types: &[RegisterType<N>],
    ) -> Result<Vec<RegisterType<N>>> {
        if input_types.len() != 1 {
            bail!("Instruction '{}' expects 1 input, found {}", Self::opcode(), input_types.len())
        }
        Ok(vec![])
    }
}

impl<N: Network> Parser for EmitLog<N> {
    /// Parses a string into an operation.
    fn parse(string: &str) -> ParserResult<Self> {
        let (string, _) = tag(*Self::opcode())(string)?;
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        let (string, value) = Operand::parse(string)?;
        Ok((string, Self { operands: [value] }))
    }
}

impl<N: Network> FromStr for EmitLog<N> {
    type Err = Error;

    fn from_str(string: &str) -> Result<Self> {
        match Self::parse(string) {
            Ok((remainder, object)) => {
                ensure!(remainder.is_empty(), "Failed to parse string. Found invalid character in: \"{remainder}\"");
                Ok(object)
            }
            Err(error) => bail!("Failed to parse string. {error}"),
        }
    }
}

impl<N: Network> Debug for EmitLog<N> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for EmitLog<N> {
    /// Prints the operation using the surface keyword `emit`.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{} {}", Self::opcode(), self.operands[0])
    }
}

impl<N: Network> FromBytes for EmitLog<N> {
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let value = Operand::read_le(&mut reader)?;
        Ok(Self { operands: [value] })
    }
}

impl<N: Network> ToBytes for EmitLog<N> {
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.operands[0].write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_parse() {
        let (string, emit) = EmitLog::<CurrentNetwork>::parse("emit r0").unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
        assert_eq!(emit.operands.len(), 1);
        assert_eq!(emit.value(), &Operand::Register(Register::Locator(0)));
    }

    #[test]
    fn test_display() {
        let emit = EmitLog::<CurrentNetwork>::from_str("emit r3").unwrap();
        assert_eq!(emit.to_string(), "emit r3");
    }

    #[test]
    fn test_bytes() {
        let emit = EmitLog::<CurrentNetwork>::from_str("emit r2").unwrap();
        let bytes = emit.to_bytes_le().unwrap();
        let decoded = EmitLog::<CurrentNetwork>::read_le(&bytes[..]).unwrap();
        assert_eq!(emit, decoded);
    }

    #[test]
    fn test_with_literal_operand() {
        let emit = EmitLog::<CurrentNetwork>::from_str("emit 42u64").unwrap();
        assert_eq!(emit.to_string(), "emit 42u64");
        let bytes = emit.to_bytes_le().unwrap();
        let decoded = EmitLog::<CurrentNetwork>::read_le(&bytes[..]).unwrap();
        assert_eq!(emit, decoded);
    }
}
