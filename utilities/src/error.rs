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

#[cfg(feature = "std")]
pub use std::error::Error;

#[cfg(not(feature = "std"))]
pub trait Error: core::fmt::Debug + core::fmt::Display {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[cfg(not(feature = "std"))]
impl<'a, E: Error + 'a> From<E> for crate::Box<dyn Error + 'a> {
    fn from(err: E) -> crate::Box<dyn Error + 'a> {
        crate::Box::new(err)
    }
}

#[cfg(not(feature = "std"))]
impl<T: Error> Error for crate::Box<T> {}

#[cfg(not(feature = "std"))]
impl Error for crate::String {}

#[cfg(not(feature = "std"))]
impl Error for crate::io::Error {}

/// This macro provides a VM runtime environment which will safely halt
/// without producing logs that look like unexpected behavior.
/// In debug mode, it prints to stderr using the format: "VM safely halted at <location>: <halt message>".
#[macro_export]
macro_rules! try_vm_runtime {
    ($e:expr) => {{
        use std::panic;

        // Set a custom hook before calling catch_unwind to
        // indicate that the panic was expected and handled.
        panic::set_hook(Box::new(|e| {
            let msg = e.to_string();
            let msg = msg.split_ascii_whitespace().skip_while(|&word| word != "panicked").collect::<Vec<&str>>();
            let mut msg = msg.join(" ");
            msg = msg.replacen("panicked", "VM safely halted", 1);
            #[cfg(debug_assertions)]
            eprintln!("{msg}");
        }));

        // Perform the operation that may panic.
        let result = panic::catch_unwind(panic::AssertUnwindSafe($e));

        // Restore the standard panic hook.
        let _ = panic::take_hook();

        // Return the result, allowing regular error-handling.
        result
    }};
}
