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

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> FromBytes
    for ProgramCoreV1<N, Instruction, Command>
{
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the program ID.
        let id = ProgramID::read_le(&mut reader)?;

        // Initialize the program.
        let mut program = ProgramCoreV1::new(id).map_err(|e| error(e.to_string()))?;

        // Read the number of program imports.
        let imports_len = u8::read_le(&mut reader)?;
        // Read the program imports.
        for _ in 0..imports_len {
            program.add_import(Import::read_le(&mut reader)?).map_err(|e| error(e.to_string()))?;
        }

        // Read the number of components.
        let components_len = u16::read_le(&mut reader)?;
        for _ in 0..components_len {
            // Read the variant.
            let variant = u8::read_le(&mut reader)?;
            // Match the variant.
            match variant {
                // Read the mapping.
                0 => program.add_mapping(Mapping::read_le(&mut reader)?).map_err(|e| error(e.to_string()))?,
                // Read the struct.
                1 => program.add_struct(StructType::read_le(&mut reader)?).map_err(|e| error(e.to_string()))?,
                // Read the record.
                2 => program.add_record(RecordType::read_le(&mut reader)?).map_err(|e| error(e.to_string()))?,
                // Read the closure.
                3 => program.add_closure(ClosureCore::read_le(&mut reader)?).map_err(|e| error(e.to_string()))?,
                // Read the function.
                4 => program.add_function(FunctionCore::read_le(&mut reader)?).map_err(|e| error(e.to_string()))?,
                // Invalid variant.
                _ => return Err(error(format!("Failed to parse program. Invalid component variant '{variant}'"))),
            }
        }

        Ok(program)
    }
}

impl<N: Network, Instruction: InstructionTrait<N>, Command: CommandTrait<N>> ToBytes
    for ProgramCoreV1<N, Instruction, Command>
{
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the program ID.
        self.id.write_le(&mut writer)?;

        // Write the number of program imports.
        u8::try_from(self.imports.len()).map_err(|e| error(e.to_string()))?.write_le(&mut writer)?;
        // Write the program imports.
        for import in self.imports.values() {
            import.write_le(&mut writer)?;
        }

        // Write the number of components.
        u16::try_from(self.identifiers.len()).map_err(|e| error(e.to_string()))?.write_le(&mut writer)?;
        // Write the components.
        for (identifier, definition) in self.identifiers.iter() {
            match definition {
                ProgramDefinition::Mapping => match self.mappings.get(identifier) {
                    Some(mapping) => {
                        // Write the variant.
                        0u8.write_le(&mut writer)?;
                        // Write the mapping.
                        mapping.write_le(&mut writer)?;
                    }
                    None => return Err(error(format!("Mapping '{identifier}' is not defined"))),
                },
                ProgramDefinition::Struct => match self.structs.get(identifier) {
                    Some(struct_) => {
                        // Write the variant.
                        1u8.write_le(&mut writer)?;
                        // Write the struct.
                        struct_.write_le(&mut writer)?;
                    }
                    None => return Err(error(format!("Struct '{identifier}' is not defined."))),
                },
                ProgramDefinition::Record => match self.records.get(identifier) {
                    Some(record) => {
                        // Write the variant.
                        2u8.write_le(&mut writer)?;
                        // Write the record.
                        record.write_le(&mut writer)?;
                    }
                    None => return Err(error(format!("Record '{identifier}' is not defined."))),
                },
                ProgramDefinition::Closure => match self.closures.get(identifier) {
                    Some(closure) => {
                        // Write the variant.
                        3u8.write_le(&mut writer)?;
                        // Write the closure.
                        closure.write_le(&mut writer)?;
                    }
                    None => return Err(error(format!("Closure '{identifier}' is not defined."))),
                },
                ProgramDefinition::Function => match self.functions.get(identifier) {
                    Some(function) => {
                        // Write the variant.
                        4u8.write_le(&mut writer)?;
                        // Write the function.
                        function.write_le(&mut writer)?;
                    }
                    None => return Err(error(format!("Function '{identifier}' is not defined."))),
                },
            }
        }

        Ok(())
    }
}
