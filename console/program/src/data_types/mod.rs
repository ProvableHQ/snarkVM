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

mod array_type;
pub use array_type::ArrayType;

mod finalize_type;
pub use finalize_type::FinalizeType;

mod literal_type;
pub use literal_type::LiteralType;

mod plaintext_type;
pub use plaintext_type::PlaintextType;

mod record_type;
pub use record_type::{EntryType, RecordType};

mod register_type;
pub use register_type::RegisterType;

mod struct_type;
pub use struct_type::StructType;

mod value_type;
pub use value_type::{ValueType, Variant};
