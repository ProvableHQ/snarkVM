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

mod utilities;
use utilities::*;

use console::network::prelude::*;
use snarkvm_synthesizer::program::Program;

use rayon::prelude::*;

#[test]
fn test_program_parse() {
    // Load the tests.
    let tests = load_tests::<_, FileParseTest>("./tests/parser/program", "./expectations/parser/program");
    // Run each test and compare it against its corresponding expectation.
    tests.par_iter().for_each(|test| {
        // Run the parser on the test string.
        let test_string = test.test_string();
        let output = match Program::<CurrentNetwork>::from_str(test_string) {
            Ok(_) => "Program was successfully parsed.".to_string(),
            Err(e) => e.to_string(),
        };
        // Check against the expected output.
        test.check(&output).unwrap();
        // Save the output.
        test.save(&output).unwrap();
    });
}
