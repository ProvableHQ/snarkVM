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

use std::fmt::Debug;

#[derive(Debug, Error)]
pub enum ParameterError {
    #[error("expected checksum of {}, found checksum of {}", _0, _1)]
    ChecksumMismatch(String, String),

    #[error("{}: {}", _0, _1)]
    Crate(&'static str, String),

    #[error("{}", _0)]
    Message(String),

    #[error("Remote fetch is disabled, enable compiler flag for feature")]
    RemoteFetchDisabled,

    #[error("Expected size of {}, found size of {}", _0, _1)]
    SizeMismatch(usize, usize),

    #[error("{}", _0)]
    Wasm(String),
}

#[cfg(not(feature = "wasm"))]
impl From<curl::Error> for ParameterError {
    fn from(error: curl::Error) -> Self {
        ParameterError::Crate("curl::error", format!("{error:?}"))
    }
}

impl From<std::io::Error> for ParameterError {
    fn from(error: std::io::Error) -> Self {
        ParameterError::Crate("std::io", format!("{error:?}"))
    }
}

impl From<std::path::StripPrefixError> for ParameterError {
    fn from(error: std::path::StripPrefixError) -> Self {
        ParameterError::Crate("std::path", format!("{error:?}"))
    }
}

impl From<ParameterError> for std::io::Error {
    fn from(error: ParameterError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, format!("{error:?}"))
    }
}
