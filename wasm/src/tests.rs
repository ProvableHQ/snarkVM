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

use snarkvm_console::{
    account::{Address, PrivateKey, ViewKey},
    network::MainnetV0,
};
use snarkvm_utilities::TestRng;

use core::str::FromStr;
use wasm_bindgen_test::*;

const ITERATIONS: usize = 1000;

#[wasm_bindgen_test]
fn test_account() {
    const ALEO_PRIVATE_KEY: &str = "APrivateKey1zkp8cC4jgHEBnbtu3xxs1Ndja2EMizcvTRDq5Nikdkukg1p";
    const ALEO_VIEW_KEY: &str = "AViewKey1n1n3ZbnVEtXVe3La2xWkUvY3EY7XaCG6RZJJ3tbvrrrD";
    const ALEO_ADDRESS: &str = "aleo1wvgwnqvy46qq0zemj0k6sfp3zv0mp77rw97khvwuhac05yuwscxqmfyhwf";

    let private_key = PrivateKey::<MainnetV0>::from_str(ALEO_PRIVATE_KEY).unwrap();
    assert_eq!(ALEO_PRIVATE_KEY, private_key.to_string());

    let view_key = ViewKey::try_from(&private_key).unwrap();
    assert_eq!(ALEO_VIEW_KEY, view_key.to_string());

    let address = Address::try_from(&view_key).unwrap();
    assert_eq!(ALEO_ADDRESS, address.to_string());
}

#[wasm_bindgen_test]
fn test_account_sign() {
    let mut rng = TestRng::default();

    for _ in 0..ITERATIONS {
        // Sample a new private key and address.
        let private_key = PrivateKey::<MainnetV0>::new(&mut rng).unwrap();
        let address = Address::try_from(&private_key).unwrap();

        // Sign a message with the account private key.
        let result = private_key.sign_bytes("hello world!".as_bytes(), &mut rng);
        assert!(result.is_ok(), "Failed to generate a signature");

        // Verify the signed message.
        let signature = result.unwrap();
        let result = signature.verify_bytes(&address, "hello world!".as_bytes());
        assert!(result, "Failed to execute signature verification");
    }
}
