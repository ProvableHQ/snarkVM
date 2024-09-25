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

#[derive(Debug, Error)]
pub enum FieldError {
    #[error("{}: {}", _0, _1)]
    Crate(&'static str, String),

    #[error("Invalid field element")]
    InvalidFieldElement,

    #[error("Attempting to parse an invalid string into a field element")]
    InvalidString,

    #[error("{}", _0)]
    Message(String),

    #[error("Attempting to parse an empty string into a field element")]
    ParsingEmptyString,

    #[error("Attempting to parse a non-digit character into a field element")]
    ParsingNonDigitCharacter,
}

impl From<std::io::Error> for FieldError {
    fn from(error: std::io::Error) -> Self {
        FieldError::Crate("std::io", format!("{error:?}"))
    }
}

impl From<FieldError> for std::io::Error {
    fn from(error: FieldError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, format!("{error}"))
    }
}
