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

mod canonical_deserialize;

mod canonical_serialize;

use syn::*;

#[proc_macro_derive(CanonicalSerialize)]
pub fn derive_canonical_serialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    proc_macro::TokenStream::from(canonical_serialize::impl_canonical_serialize(&ast))
}

#[proc_macro_derive(CanonicalDeserialize)]
pub fn derive_canonical_deserialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    proc_macro::TokenStream::from(canonical_deserialize::impl_canonical_deserialize(&ast))
}
