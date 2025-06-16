// Copyright (c) 2019-2025 Provable Inc.
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

#[allow(dead_code)]
const ITERATIONS: usize = 1000;

wasm_bindgen_test_configure!(run_in_browser);

use js_sys::Date;
use web_sys::console;

#[allow(dead_code)]
#[wasm_bindgen_test]
fn test_account() {
    console::log_1(&"Testing account...".into());

    const ALEO_PRIVATE_KEY: &str = "APrivateKey1zkp8cC4jgHEBnbtu3xxs1Ndja2EMizcvTRDq5Nikdkukg1p";
    const ALEO_VIEW_KEY: &str = "AViewKey1n1n3ZbnVEtXVe3La2xWkUvY3EY7XaCG6RZJJ3tbvrrrD";
    const ALEO_ADDRESS: &str = "aleo1wvgwnqvy46qq0zemj0k6sfp3zv0mp77rw97khvwuhac05yuwscxqmfyhwf";

    console::log_1(&format!("Aleo Private Key: {ALEO_PRIVATE_KEY}").into());

    let private_key = PrivateKey::<MainnetV0>::from_str(ALEO_PRIVATE_KEY).unwrap();
    assert_eq!(ALEO_PRIVATE_KEY, private_key.to_string());

    let view_key = ViewKey::try_from(&private_key).unwrap();
    assert_eq!(ALEO_VIEW_KEY, view_key.to_string());

    let address = Address::try_from(&view_key).unwrap();
    assert_eq!(ALEO_ADDRESS, address.to_string());
}

#[allow(dead_code)]
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

#[allow(dead_code)]
#[wasm_bindgen_test]
fn test_authorization_signing() {
    let mut rng = TestRng::default();

    // Sample a new private key and address.
    let private_key = PrivateKey::<MainnetV0>::new(&mut rng).unwrap();
    let address = Address::try_from(&private_key).unwrap();

    let start = Date::now();
    let process = snarkvm_synthesizer::Process::<MainnetV0>::load_web().unwrap();
    let elapsed = Date::now() - start;
    console::log_1(&format!("Loaded process in: {elapsed}ms").into());

    let start = Date::now();
    use snarkvm_circuit_network::Aleo;
    snarkvm_circuit_network::AleoV0::initialize_global_constants();
    let elapsed = Date::now() - start;
    console::log_1(&format!("Initialized global constants in: {elapsed}ms").into());

    let mut total_auth_time: f64 = 0.0;
    const ITERATIONS: u32 = 10;
    for _ in 0..ITERATIONS {
        // Create an authorization for "transfer_public"
        let start = Date::now();
        let inputs = [
            snarkvm_console::program::Value::<MainnetV0>::from_str(&address.to_string()).unwrap(),
            snarkvm_console::program::Value::<MainnetV0>::from_str("100u64").unwrap(),
        ]
        .into_iter();
        process
            .authorize::<snarkvm_circuit_network::AleoV0, _>(
                &private_key,
                "credits.aleo",
                "transfer_public",
                inputs,
                &mut rng,
            )
            .unwrap();
        let auth_time = Date::now() - start;
        console::log_1(&format!("Created authorization in: {auth_time}ms").into());

        total_auth_time += auth_time;
    }

    let average_auth_time = total_auth_time as u128 / ITERATIONS as u128;
    console::log_1(&format!("Average authorization time: {}ms", average_auth_time).into());

    assert!(false);
}
