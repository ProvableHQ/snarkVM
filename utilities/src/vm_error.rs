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

pub use std::error::Error;

/// This macro provides a VM runtime environment which will safely halt
/// without producing logs that look like unexpected behavior.
/// In debug mode, it prints to stderr using the format: "VM safely halted at {location}: {halt message}".
#[macro_export]
macro_rules! try_vm_runtime {
    ($e:expr) => {{
        use std::panic;

        // Store the previous hook (if any).
        let previous_hook = panic::take_hook();

        // Set a custom hook before calling catch_unwind to
        // indicate that the panic was expected and handled.
        panic::set_hook(Box::new(|err| {
            #[cfg(debug_assertions)]
            {
                // Remove all words up to "panicked".
                let trimmed = err
                    .to_string()
                    .split_ascii_whitespace()
                    .skip_while(|&word| word != "panicked")
                    .collect::<Vec<&str>>()
                    .join(" ");

                // Have the message start with "VM safely halted".
                let msg = trimmed.replacen("panicked", "VM safely halted", 1);
                eprintln!("{msg}");
            }
        }));

        // Perform the operation that may panic.
        let result = panic::catch_unwind(panic::AssertUnwindSafe($e));

        // Restore the standard panic hook.
        panic::set_hook(previous_hook);

        // Return the result, allowing regular error-handling.
        result
    }};
}
