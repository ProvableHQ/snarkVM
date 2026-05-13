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

use crate::{Opcode, Operand, RegistersCircuit, RegistersTrait, StackTrait, register_types_equivalent};
use console::{
    network::prelude::*,
    program::{Register, RegisterType},
};
use snarkvm_synthesizer_error::*;

/// Asserts two operands are equal to each other.
pub type AssertEq<N> = AssertInstruction<N, { Variant::AssertEq as u8 }>;
/// Asserts two operands are **not** equal to each other.
pub type AssertNeq<N> = AssertInstruction<N, { Variant::AssertNeq as u8 }>;
/// Asserts two operands are equal, attaching a plaintext reason that surfaces on failure.
pub type AssertEqWithReason<N> = AssertInstruction<N, { Variant::AssertEqWithReason as u8 }>;
/// Asserts two operands are **not** equal, attaching a plaintext reason that surfaces on failure.
pub type AssertNeqWithReason<N> = AssertInstruction<N, { Variant::AssertNeqWithReason as u8 }>;

#[allow(clippy::enum_variant_names)]
enum Variant {
    AssertEq,
    AssertNeq,
    AssertEqWithReason,
    AssertNeqWithReason,
}

/// Asserts an operation on two operands. The `WithReason` variants carry a third plaintext
/// operand whose value is surfaced in the failure message.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AssertInstruction<N: Network, const VARIANT: u8> {
    /// The operands. Length is 2 for the bare variants and 3 for the with-reason variants
    /// (the third operand is the reason plaintext).
    operands: Vec<Operand<N>>,
}

impl<N: Network, const VARIANT: u8> AssertInstruction<N, VARIANT> {
    /// Returns the expected number of operands for this variant.
    const fn arity() -> usize {
        match VARIANT {
            0 | 1 => 2,
            2 | 3 => 3,
            _ => panic!("Invalid 'assert' instruction VARIANT"),
        }
    }

    /// Returns true if this variant checks for inequality (assert.neq family).
    const fn is_neq() -> bool {
        matches!(VARIANT, 1 | 3)
    }

    /// Returns true if this variant carries a reason operand.
    const fn has_reason() -> bool {
        matches!(VARIANT, 2 | 3)
    }

    /// Initializes a new `assert` instruction.
    #[inline]
    pub fn new(operands: Vec<Operand<N>>) -> Result<Self> {
        ensure!(operands.len() == Self::arity(), "Assert instruction expects {} operands", Self::arity());
        Ok(Self { operands })
    }

    /// Returns the opcode. The bare and with-reason variants get distinct internal opcode
    /// strings (required by the wire format, which dispatches by opcode-string lookup).
    /// The surface keyword used in Aleo source is shared and managed by the parser/display.
    #[inline]
    pub const fn opcode() -> Opcode {
        match VARIANT {
            0 => Opcode::Assert("assert.eq"),
            1 => Opcode::Assert("assert.neq"),
            2 => Opcode::Assert("assert.eq.with_reason"),
            3 => Opcode::Assert("assert.neq.with_reason"),
            _ => panic!("Invalid 'assert' instruction opcode"),
        }
    }

    /// Returns the surface keyword used in Aleo source (`assert.eq` or `assert.neq`),
    /// which is identical for the bare and with-reason variants of the same comparison.
    const fn surface_keyword() -> &'static str {
        match VARIANT {
            0 | 2 => "assert.eq",
            1 | 3 => "assert.neq",
            _ => panic!("Invalid 'assert' instruction VARIANT"),
        }
    }

    /// Returns the operands in the operation.
    #[inline]
    pub fn operands(&self) -> &[Operand<N>] {
        debug_assert!(self.operands.len() == Self::arity(), "Assert operations have a fixed arity per variant");
        &self.operands
    }

    /// Returns the destination register.
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

impl<N: Network, const VARIANT: u8> AssertInstruction<N, VARIANT> {
    /// Evaluates the instruction.
    pub fn evaluate(
        &self,
        stack: &impl StackTrait<N>,
        registers: &mut impl RegistersTrait<N>,
    ) -> Result<(), EvalError> {
        if self.operands.len() != Self::arity() {
            return Err(anyhow!(
                "Instruction '{}' expects {} operands, found {} operands",
                Self::opcode(),
                Self::arity(),
                self.operands.len()
            )
            .into());
        }

        let input_a = registers.load(stack, &self.operands[0])?;
        let input_b = registers.load(stack, &self.operands[1])?;

        let failed = match Self::is_neq() {
            false => input_a != input_b,
            true => input_a == input_b,
        };

        if failed {
            let lhs = format!("{input_a}");
            let rhs = format!("{input_b}");

            if Self::has_reason() {
                // Resolve the reason plaintext at runtime and surface it in the speculation
                // error message via the AssertError variant. The reason is not persisted to
                // chain state — only `RejectedReason::Finalize.command` records *where* the
                // failure happened. Consumers wanting the resolved value recover it via
                // local replay.
                let reason = registers.load_plaintext(stack, &self.operands[2])?;
                let reason = format!("{reason}");
                return Err(match Self::is_neq() {
                    false => AssertError::EqWithReason { lhs, rhs, reason }.into(),
                    true => AssertError::NeqWithReason { lhs, rhs, reason }.into(),
                });
            }

            return Err(match Self::is_neq() {
                false => AssertError::Eq { lhs, rhs }.into(),
                true => AssertError::Neq { lhs, rhs }.into(),
            });
        }
        Ok(())
    }

    /// Executes the instruction.
    pub fn execute<A: circuit::Aleo<Network = N>>(
        &self,
        stack: &impl StackTrait<N>,
        registers: &mut impl RegistersCircuit<N, A>,
    ) -> Result<(), ExecError> {
        if self.operands.len() != Self::arity() {
            return Err(anyhow!(
                "Instruction '{}' expects {} operands, found {} operands",
                Self::opcode(),
                Self::arity(),
                self.operands.len()
            )
            .into());
        }

        let input_a = registers.load_circuit(stack, &self.operands[0])?;
        let input_b = registers.load_circuit(stack, &self.operands[1])?;

        // For with-reason variants, the reason operand is intentionally ignored in circuit
        // context; circuit-side printing on failure lands in phase 4.
        match Self::is_neq() {
            false => A::assert(input_a.is_equal(&input_b))?,
            true => A::assert(input_a.is_not_equal(&input_b))?,
        }
        Ok(())
    }

    /// Finalizes the instruction.
    #[inline]
    pub fn finalize(
        &self,
        stack: &impl StackTrait<N>,
        registers: &mut impl RegistersTrait<N>,
    ) -> Result<(), FinalizeError> {
        self.evaluate(stack, registers)?;
        Ok(())
    }

    /// Returns the output type from the given program and input types.
    pub fn output_types(
        &self,
        stack: &impl StackTrait<N>,
        input_types: &[RegisterType<N>],
    ) -> Result<Vec<RegisterType<N>>> {
        if input_types.len() != Self::arity() {
            bail!(
                "Instruction '{}' expects {} inputs, found {} inputs",
                Self::opcode(),
                Self::arity(),
                input_types.len()
            )
        }
        // The first two operands must have equivalent types (they're being compared).
        if !register_types_equivalent(stack, &input_types[0], stack, &input_types[1])? {
            bail!(
                "Instruction '{}' expects inputs of equivalent types. Found inputs of type '{}' and '{}'",
                Self::opcode(),
                input_types[0],
                input_types[1]
            )
        }
        if self.operands.len() != Self::arity() {
            bail!(
                "Instruction '{}' expects {} operands, found {} operands",
                Self::opcode(),
                Self::arity(),
                self.operands.len()
            )
        }
        // The third operand (reason) is only constrained to exist; its type is not constrained
        // further (any plaintext is fine).
        Ok(vec![])
    }
}

impl<N: Network, const VARIANT: u8> Parser for AssertInstruction<N, VARIANT> {
    /// Parses a string into an operation. The surface keyword in Aleo source is shared
    /// between the bare and with-reason variants; the optional ` with <reason>` tail
    /// disambiguates and is required for with-reason variants.
    fn parse(string: &str) -> ParserResult<Self> {
        let (string, _) = tag(Self::surface_keyword())(string)?;
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        let (string, first) = Operand::parse(string)?;
        let (string, _) = Sanitizer::parse_whitespaces(string)?;
        let (string, second) = Operand::parse(string)?;

        if Self::has_reason() {
            // Parse `with <reason>` tail.
            let (string, _) = Sanitizer::parse_whitespaces(string)?;
            let (string, _) = tag("with")(string)?;
            let (string, _) = Sanitizer::parse_whitespaces(string)?;
            let (string, reason) = Operand::parse(string)?;
            return Ok((string, Self { operands: vec![first, second, reason] }));
        }

        Ok((string, Self { operands: vec![first, second] }))
    }
}

impl<N: Network, const VARIANT: u8> FromStr for AssertInstruction<N, VARIANT> {
    type Err = Error;

    /// Parses a string into an operation.
    fn from_str(string: &str) -> Result<Self> {
        match Self::parse(string) {
            Ok((remainder, object)) => {
                // Ensure the remainder is empty.
                ensure!(remainder.is_empty(), "Failed to parse string. Found invalid character in: \"{remainder}\"");
                // Return the object.
                Ok(object)
            }
            Err(error) => bail!("Failed to parse string. {error}"),
        }
    }
}

impl<N: Network, const VARIANT: u8> Debug for AssertInstruction<N, VARIANT> {
    /// Prints the operation as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network, const VARIANT: u8> Display for AssertInstruction<N, VARIANT> {
    /// Prints the operation to a string using the surface keyword (`assert.eq` / `assert.neq`),
    /// regardless of whether the variant carries an internal `.with_reason` opcode tag.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if self.operands.len() != Self::arity() {
            return Err(fmt::Error);
        }
        write!(f, "{}", Self::surface_keyword())?;
        write!(f, " {} {}", self.operands[0], self.operands[1])?;
        if Self::has_reason() {
            write!(f, " with {}", self.operands[2])?;
        }
        Ok(())
    }
}

impl<N: Network, const VARIANT: u8> FromBytes for AssertInstruction<N, VARIANT> {
    /// Reads the operation from a buffer.
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let mut operands = Vec::with_capacity(Self::arity());
        for _ in 0..Self::arity() {
            operands.push(Operand::read_le(&mut reader)?);
        }
        Ok(Self { operands })
    }
}

impl<N: Network, const VARIANT: u8> ToBytes for AssertInstruction<N, VARIANT> {
    /// Writes the operation to a buffer.
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        if self.operands.len() != Self::arity() {
            return Err(error(format!(
                "The number of operands must be {}, found {}",
                Self::arity(),
                self.operands.len()
            )));
        }
        self.operands.iter().try_for_each(|operand| operand.write_le(&mut writer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_parse_bare() {
        let (string, assert) = AssertEq::<CurrentNetwork>::parse("assert.eq r0 r1").unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
        assert_eq!(assert.operands.len(), 2, "The number of operands is incorrect");
        assert_eq!(assert.operands[0], Operand::Register(Register::Locator(0)), "The first operand is incorrect");
        assert_eq!(assert.operands[1], Operand::Register(Register::Locator(1)), "The second operand is incorrect");

        let (string, assert) = AssertNeq::<CurrentNetwork>::parse("assert.neq r0 r1").unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
        assert_eq!(assert.operands.len(), 2, "The number of operands is incorrect");
        assert_eq!(assert.operands[0], Operand::Register(Register::Locator(0)), "The first operand is incorrect");
        assert_eq!(assert.operands[1], Operand::Register(Register::Locator(1)), "The second operand is incorrect");
    }

    #[test]
    fn test_parse_with_reason() {
        let (string, assert) = AssertEqWithReason::<CurrentNetwork>::parse("assert.eq r0 r1 with r2").unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
        assert_eq!(assert.operands.len(), 3, "The number of operands is incorrect");
        assert_eq!(assert.operands[0], Operand::Register(Register::Locator(0)));
        assert_eq!(assert.operands[1], Operand::Register(Register::Locator(1)));
        assert_eq!(assert.operands[2], Operand::Register(Register::Locator(2)));

        let (string, assert) = AssertNeqWithReason::<CurrentNetwork>::parse("assert.neq r3 r4 with r5").unwrap();
        assert!(string.is_empty(), "Parser did not consume all of the string: '{string}'");
        assert_eq!(assert.operands.len(), 3);
        assert_eq!(assert.operands[2], Operand::Register(Register::Locator(5)));
    }

    #[test]
    fn test_display_with_reason() {
        let assert = AssertEqWithReason::<CurrentNetwork>::from_str("assert.eq r0 r1 with r2").unwrap();
        assert_eq!(assert.to_string(), "assert.eq r0 r1 with r2");

        let assert = AssertNeqWithReason::<CurrentNetwork>::from_str("assert.neq r3 r4 with r5").unwrap();
        assert_eq!(assert.to_string(), "assert.neq r3 r4 with r5");
    }

    #[test]
    fn test_bytes_with_reason() {
        let eq = AssertEqWithReason::<CurrentNetwork>::from_str("assert.eq r0 r1 with r2").unwrap();
        let bytes = eq.to_bytes_le().unwrap();
        let decoded = AssertEqWithReason::<CurrentNetwork>::read_le(&bytes[..]).unwrap();
        assert_eq!(eq, decoded);

        let neq = AssertNeqWithReason::<CurrentNetwork>::from_str("assert.neq r3 r4 with r5").unwrap();
        let bytes = neq.to_bytes_le().unwrap();
        let decoded = AssertNeqWithReason::<CurrentNetwork>::read_le(&bytes[..]).unwrap();
        assert_eq!(neq, decoded);
    }

    /// Regression test for the opcode-collision bug: the bare and with-reason variants
    /// share the surface keyword (`assert.eq` / `assert.neq`) but must have distinct
    /// internal opcode strings so that `Instruction`-level bytes serialization dispatches
    /// to the correct read path. If the with-reason variants ever share an opcode string
    /// with their bare counterparts, this round-trip will deserialize back to the wrong
    /// arity and the display will drop the `with` tail.
    #[test]
    fn test_instruction_bytes_roundtrip() {
        use crate::Instruction;
        let cases = ["assert.eq r0 r1;", "assert.neq r0 r1;", "assert.eq r0 r1 with r2;", "assert.neq r0 r1 with r2;"];
        for source in cases {
            let expected = Instruction::<CurrentNetwork>::from_str(source).unwrap();
            let bytes = expected.to_bytes_le().unwrap();
            let decoded = Instruction::<CurrentNetwork>::read_le(&bytes[..]).unwrap();
            assert_eq!(
                format!("{decoded}"),
                format!("{expected}"),
                "Instruction-level bytes roundtrip dropped/altered operands for '{source}'"
            );
        }
    }

    /// The reason operand may be either a register or a literal — both are valid
    /// `Operand` shapes. Verify the literal form parses and round-trips through bytes,
    /// since Leo's `require(cond, MyReason { ... })` could lower to either shape.
    #[test]
    fn test_with_reason_literal_operand() {
        let eq = AssertEqWithReason::<CurrentNetwork>::from_str("assert.eq r0 r1 with 99u64").unwrap();
        assert_eq!(eq.to_string(), "assert.eq r0 r1 with 99u64");
        let bytes = eq.to_bytes_le().unwrap();
        let decoded = AssertEqWithReason::<CurrentNetwork>::read_le(&bytes[..]).unwrap();
        assert_eq!(eq, decoded);
    }
}
