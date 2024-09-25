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

#[derive(Debug, PartialEq, Eq)]
pub enum LegendreSymbol {
    Zero = 0,
    QuadraticResidue = 1,
    QuadraticNonResidue = -1,
}

impl LegendreSymbol {
    pub fn is_zero(&self) -> bool {
        *self == LegendreSymbol::Zero
    }

    pub fn is_qnr(&self) -> bool {
        *self == LegendreSymbol::QuadraticNonResidue
    }

    pub fn is_qr(&self) -> bool {
        *self == LegendreSymbol::QuadraticResidue
    }
}
