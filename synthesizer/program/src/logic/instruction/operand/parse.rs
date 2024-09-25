// Copyright 2024 Aleo Network Foundation
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

use super::*;

impl<N: Network> Parser for Operand<N> {
    /// Parses a string into a operand.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // Parse to determine the operand (order matters).
        alt((
            // Parse special operands before literals, registers, and program IDs.
            // This ensures correctness in the case where a special operand is a prefix of, or could be parsed as, a literal, register, or program ID.
            map(tag("group::GEN"), |_| Self::Literal(Literal::Group(Group::generator()))),
            map(tag("self.signer"), |_| Self::Signer),
            map(tag("self.caller"), |_| Self::Caller),
            map(tag("block.height"), |_| Self::BlockHeight),
            map(tag("network.id"), |_| Self::NetworkID),
            // Note that `Operand::ProgramID`s must be parsed before `Operand::Literal`s, since a program ID can be implicitly parsed as a literal address.
            // This ensures that the string representation of a program uses the `Operand::ProgramID` variant.
            map(ProgramID::parse, |program_id| Self::ProgramID(program_id)),
            map(Literal::parse, |literal| Self::Literal(literal)),
            map(Register::parse, |register| Self::Register(register)),
        ))(string)
    }
}

impl<N: Network> FromStr for Operand<N> {
    type Err = Error;

    /// Parses a string into an operand.
    #[inline]
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

impl<N: Network> Debug for Operand<N> {
    /// Prints the operand as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<N: Network> Display for Operand<N> {
    /// Prints the operand as a string.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            // Prints the literal, i.e. 10field.private
            Self::Literal(literal) => Display::fmt(literal, f),
            // Prints the register, i.e. r0 or r0.owner
            Self::Register(register) => Display::fmt(register, f),
            // Prints the program ID, i.e. howard.aleo
            Self::ProgramID(program_id) => Display::fmt(program_id, f),
            // Prints the identifier for the signer, i.e. self.signer
            Self::Signer => write!(f, "self.signer"),
            // Prints the identifier for the caller, i.e. self.caller
            Self::Caller => write!(f, "self.caller"),
            // Prints the identifier for the block height, i.e. block.height
            Self::BlockHeight => write!(f, "block.height"),
            // Prints the identifier for the network ID, i.e. network.id
            Self::NetworkID => write!(f, "network.id"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::MainnetV0;

    type CurrentNetwork = MainnetV0;

    #[test]
    fn test_operand_parse() -> Result<()> {
        let operand = Operand::<CurrentNetwork>::parse("1field").unwrap().1;
        assert_eq!(Operand::Literal(Literal::from_str("1field")?), operand);

        let operand = Operand::<CurrentNetwork>::parse("r0").unwrap().1;
        assert_eq!(Operand::Register(Register::from_str("r0")?), operand);

        let operand = Operand::<CurrentNetwork>::parse("r0.owner").unwrap().1;
        assert_eq!(Operand::Register(Register::from_str("r0.owner")?), operand);

        let operand = Operand::<CurrentNetwork>::parse("howard.aleo").unwrap().1;
        assert_eq!(Operand::ProgramID(ProgramID::from_str("howard.aleo")?), operand);

        let operand = Operand::<CurrentNetwork>::parse("self.signer").unwrap().1;
        assert_eq!(Operand::Signer, operand);

        let operand = Operand::<CurrentNetwork>::parse("self.caller").unwrap().1;
        assert_eq!(Operand::Caller, operand);

        let operand = Operand::<CurrentNetwork>::parse("block.height").unwrap().1;
        assert_eq!(Operand::BlockHeight, operand);

        let operand = Operand::<CurrentNetwork>::parse("network.id").unwrap().1;
        assert_eq!(Operand::NetworkID, operand);

        let operand = Operand::<CurrentNetwork>::parse("group::GEN").unwrap().1;
        assert_eq!(Operand::Literal(Literal::Group(Group::generator())), operand);

        // Sanity check a failure case.
        let (remainder, operand) = Operand::<CurrentNetwork>::parse("1field.private").unwrap();
        assert_eq!(Operand::Literal(Literal::from_str("1field")?), operand);
        assert_eq!(".private", remainder);

        Ok(())
    }

    #[test]
    fn test_operand_display() {
        let operand = Operand::<CurrentNetwork>::parse("1field").unwrap().1;
        assert_eq!(format!("{operand}"), "1field");

        let operand = Operand::<CurrentNetwork>::parse("r0").unwrap().1;
        assert_eq!(format!("{operand}"), "r0");

        let operand = Operand::<CurrentNetwork>::parse("r0.owner").unwrap().1;
        assert_eq!(format!("{operand}"), "r0.owner");

        let operand = Operand::<CurrentNetwork>::parse("howard.aleo").unwrap().1;
        assert_eq!(format!("{operand}"), "howard.aleo");

        let operand = Operand::<CurrentNetwork>::parse("self.signer").unwrap().1;
        assert_eq!(format!("{operand}"), "self.signer");

        let operand = Operand::<CurrentNetwork>::parse("self.caller").unwrap().1;
        assert_eq!(format!("{operand}"), "self.caller");

        let operand = Operand::<CurrentNetwork>::parse("group::GEN").unwrap().1;
        assert_eq!(
            format!("{operand}"),
            "1540945439182663264862696551825005342995406165131907382295858612069623286213group"
        );
    }

    #[test]
    fn test_operand_from_str_fails() -> Result<()> {
        assert!(Operand::<CurrentNetwork>::from_str("1field.private").is_err());
        Ok(())
    }
}
