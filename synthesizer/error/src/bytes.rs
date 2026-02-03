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

use crate::{FinalizeTypesInitError, ProcessError, ProcessFinalizeError, ProgramUpgradeError, StackInitError};

use snarkvm_console_network::{FromBytes, IoResult, ToBytes, error};

use std::io::{Read, Write};

impl FromBytes for StackInitError {
    /// Reads the transaction from the buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the variant.
        let variant = u8::read_le(&mut reader)?;

        let read_err_detail = |reader: &mut R| -> IoResult<String> {
            let len = u16::read_le(&mut *reader)?;
            let mut detail = Vec::with_capacity(len as usize);
            reader.read_exact(&mut detail)?;
            String::from_utf8(detail).map_err(|_| error("Invalid error encoding"))
        };

        let err = match variant {
            0 => {
                let detail = read_err_detail(&mut reader)?;
                Self::ClosureAlreadyExists(detail)
            }
            1 => Self::CreditsReinitialization,
            2 => {
                let detail = read_err_detail(&mut reader)?;
                Self::DifferentProgramAlreadyExists(detail)
            }
            3 => {
                let err = FinalizeTypesInitError::read_le(&mut reader)?;
                Self::FinalizeTypesInit(err)
            }
            4 => {
                let detail = read_err_detail(&mut reader)?;
                Self::FunctionAlreadyExists(detail)
            }
            // 5 => {
            //     let detail1 = read_err_detail(&mut reader)?;
            //     let detail2 = read_err_detail(&mut reader)?;
            //     Self::MissingExternalImport(detail1, detail2)
            // }
            6 => {
                let detail = read_err_detail(&mut reader)?;
                Self::MissingImport(detail)
            }
            7 => {
                let err = ProcessError::read_le(&mut reader)?;
                Self::Process(err)
            }
            8 => Self::ProgramEditionOverflow,
            9 => Self::ProgramIdConversion,
            10 => Self::ProgramMalformed,
            11 => {
                let detail = read_err_detail(&mut reader)?;
                Self::ProgramMissingFunctions(detail)
            }
            12 => Self::ProgramSelfImport,
            13 => {
                let err = ProgramUpgradeError::read_le(&mut reader)?;
                Self::ProgramUpgrade(err)
            }
            _ => {
                return Err(error("Invalid RejectionReason variant"));
            }
        };

        Ok(err)
    }
}

impl ToBytes for StackInitError {
    /// Writes the transaction to the buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::ClosureAlreadyExists(detail) => {
                0u8.write_le(&mut writer)?;
                (detail.len() as u16).write_le(&mut writer)?;
                detail.as_bytes().write_le(&mut writer)
            }
            Self::CreditsReinitialization => 1u8.write_le(&mut writer),
            Self::DifferentProgramAlreadyExists(detail) => {
                2u8.write_le(&mut writer)?;
                (detail.len() as u16).write_le(&mut writer)?;
                detail.as_bytes().write_le(&mut writer)
            }
            Self::FinalizeTypesInit(err) => {
                3u8.write_le(&mut writer)?;
                err.write_le(&mut writer)
            }
            Self::FunctionAlreadyExists(detail) => {
                4u8.write_le(&mut writer)?;
                (detail.len() as u16).write_le(&mut writer)?;
                detail.as_bytes().write_le(&mut writer)
            }
            // Self::MissingExternalImport(detail1, detail2) => {
            //     5u8.write_le(&mut writer)?;
            //     (detail1.len() as u16).write_le(&mut writer)?;
            //     detail1.as_bytes().write_le(&mut writer)?;
            //     (detail2.len() as u16).write_le(&mut writer)?;
            //     detail2.as_bytes().write_le(&mut writer)
            // }
            Self::MissingImport(detail) => {
                6u8.write_le(&mut writer)?;
                (detail.len() as u16).write_le(&mut writer)?;
                detail.as_bytes().write_le(&mut writer)
            }
            Self::Process(err) => {
                7u8.write_le(&mut writer)?;
                err.write_le(&mut writer)
            }
            Self::ProgramEditionOverflow => 8u8.write_le(&mut writer),
            Self::ProgramIdConversion => 9u8.write_le(&mut writer),
            Self::ProgramMalformed => 10u8.write_le(&mut writer),
            Self::ProgramMissingFunctions(detail) => {
                11u8.write_le(&mut writer)?;
                (detail.len() as u16).write_le(&mut writer)?;
                detail.as_bytes().write_le(&mut writer)
            }
            Self::ProgramSelfImport => 12u8.write_le(&mut writer),
            Self::ProgramUpgrade(err) => {
                13u8.write_le(&mut writer)?;
                err.write_le(&mut writer)
            }
        }
    }
}

impl FromBytes for ProcessFinalizeError {
    /// Reads the transaction from the buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the variant.
        let variant = u8::read_le(&mut reader)?;

        let err = match variant {
            0 => {
                let inner = StackInitError::read_le(&mut reader)?;
                Self::StackInit(inner)
            }
            _ => {
                return Err(error("Invalid ProcessFinalizeError variant"));
            }
        };

        Ok(err)
    }
}

impl ToBytes for ProcessFinalizeError {
    /// Writes the transaction to the buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        match self {
            Self::StackInit(inner) => {
                0u8.write_le(&mut writer)?;
                inner.write_le(&mut writer)
            }
        }
    }
}

impl FromBytes for ProcessError {
    /// Reads the transaction from the buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the variant.
        let variant = u8::read_le(&mut reader)?;

        todo!()
    }
}

impl ToBytes for ProcessError {
    /// Writes the transaction to the buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        todo!()
    }
}

impl FromBytes for ProgramUpgradeError {
    /// Reads the transaction from the buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the variant.
        let variant = u8::read_le(&mut reader)?;

        todo!()
    }
}

impl ToBytes for ProgramUpgradeError {
    /// Writes the transaction to the buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        todo!()
    }
}

impl FromBytes for FinalizeTypesInitError {
    /// Reads the transaction from the buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the variant.
        let variant = u8::read_le(&mut reader)?;

        todo!()
    }
}

impl ToBytes for FinalizeTypesInitError {
    /// Writes the transaction to the buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        todo!()
    }
}
