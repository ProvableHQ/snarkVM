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

use colored::Colorize;

use std::{any::Any, backtrace::Backtrace, borrow::Borrow, cell::Cell, panic};

thread_local! {
/// The message backtrace of the last panic on this thread (if any).
///
/// We store this information here instead of directly processing it in a panic hook, because panic hooks are global whereas this can be processed on a per-thread basis.
/// For example, one thread may execute a program where panics should *not* cause the entire process to terminate, while in another thread there is a panic due to a bug.
static PANIC_INFO: Cell<Option<(String, Backtrace)>> = const { Cell::new(None) };
}

/// Generates an `io::Error` from the given string.
#[inline]
pub fn io_error<S: ToString>(err: S) -> std::io::Error {
    std::io::Error::other(err.to_string())
}

/// Generates an `io::Error` from the given `anyhow::Error`.
///
/// This will flatten the existing error chain so that it fits in a single-line string.
#[inline]
pub fn into_io_error<E: Into<anyhow::Error>>(err: E) -> std::io::Error {
    let err: anyhow::Error = err.into();
    std::io::Error::other(flatten_error(&err))
}

/// Converts an `anyhow::Error` into a single-line string.
///
/// This follows the existing convention in the codebase that joins errors using em dashes.
/// For example, an error "Invalid transaction" with a cause "Proof failed" would be logged
/// as "Invalid transaction — Proof failed".
#[inline]
pub fn flatten_error<E: Borrow<anyhow::Error>>(error: E) -> String {
    let error = error.borrow();
    let chain = error.chain().skip(1).map(|next| next.to_string()).collect::<Vec<String>>().join(" — ");
    format!("{error}{}", format!(" — {chain}").dimmed())
}

/// Displays an `anyhow::Error`'s main error and its error chain to stderr.
///
/// This can be used to show a "pretty" error to the end user.
#[track_caller]
#[inline]
pub fn display_error<E: Borrow<anyhow::Error>>(error: E) {
    let error = error.borrow();
    eprintln!("⚠️ {error}");
    error.chain().skip(1).for_each(|cause| eprintln!("     ↳ {cause}"));
}

/// Ensures that two values are equal, otherwise bails with a formatted error message.
///
/// # Arguments
/// * `actual` - The actual value
/// * `expected` - The expected value  
/// * `message` - A description of what was being checked
#[macro_export]
macro_rules! ensure_equals {
    ($actual:expr, $expected:expr, $message:expr) => {
        if $actual != $expected {
            anyhow::bail!("{}: Was {} but expected {}.", $message, $actual, $expected);
        }
    };
}

/// A trait to provide a nicer way to unwarp `anyhow::Result`.
pub trait PrettyUnwrap {
    type Inner;

    /// Behaves like [`std::result::Result::unwrap`] but will print the entire anyhow chain to stderr.
    fn pretty_unwrap(self) -> Self::Inner;

    /// Behaves like [`std::result::Result::expect`] but will print the entire anyhow chain to stderr.
    fn pretty_expect<S: ToString>(self, context: S) -> Self::Inner;
}

/// Set the global panic hook for process. Should be called exactly once.
pub fn set_panic_hook() {
    std::panic::set_hook(Box::new(|err| {
        let msg = err.to_string();
        let trace = Backtrace::force_capture();
        PANIC_INFO.with(move |info| info.set(Some((msg, trace))));
    }));
}

/// Helper for `PrettyUnwrap`:
/// Creates a panic with the `anyhow::Error` nicely formatted.
#[track_caller]
#[inline]
fn pretty_panic(error: &anyhow::Error) -> ! {
    let mut string = format!("⚠️ {error}");
    error.chain().skip(1).for_each(|cause| string.push_str(&format!("\n     ↳ {cause}")));
    let caller = std::panic::Location::caller();

    tracing::error!("[{}:{}] {string}", caller.file(), caller.line());
    panic!("{string}");
}

/// Implement the trait for `anyhow::Result`.
impl<T> PrettyUnwrap for anyhow::Result<T> {
    type Inner = T;

    #[track_caller]
    #[inline]
    fn pretty_unwrap(self) -> Self::Inner {
        match self {
            Ok(result) => result,
            Err(error) => {
                pretty_panic(&error);
            }
        }
    }

    #[track_caller]
    fn pretty_expect<S: ToString>(self, context: S) -> Self::Inner {
        match self {
            Ok(result) => result,
            Err(error) => {
                pretty_panic(&error.context(context.to_string()));
            }
        }
    }
}

/// `try_vm_runtime` executes the given closure in an environment which will safely halt
/// without producing logs that look like unexpected behavior.
/// In debug mode, it prints to stderr using the format: "VM safely halted at {location}: {halt message}".
///
/// Note: For this to work as expected, panics must be set to `unwind` during compilation (default), and the closure cannot invoke any async code that may potentially execute in a different OS thread.
#[track_caller]
#[inline]
pub fn try_vm_runtime<R, F: FnMut() -> R>(f: F) -> Result<R, Box<dyn Any + Send>> {
    // Perform the operation that may panic.
    let result = std::panic::catch_unwind(panic::AssertUnwindSafe(f));

    if result.is_err() {
        // Get the stored panic and backtrace from the thread-local variable.
        let (msg, _) = PANIC_INFO.with(|info| info.take()).expect("No panic information stored?");

        #[cfg(debug_assertions)]
        {
            // Remove all words up to "panicked".
            // And prepend with "VM Safely halted"
            let msg = msg
                .to_string()
                .split_ascii_whitespace()
                .skip_while(|&word| word != "panicked")
                .collect::<Vec<&str>>()
                .join(" ")
                .replacen("panicked", "VM safely halted", 1);

            eprintln!("{msg}");
        }
        #[cfg(not(debug_assertions))]
        {
            // Discard message
            let _ = msg;
        }
    }

    // Return the result, allowing regular error-handling.
    result
}

/// `catch_unwind` calls the given closure `f` and, if `f` panics, returns the panic message and backtrace.
#[inline]
pub fn catch_unwind<R, F: FnMut() -> R>(f: F) -> Result<R, (String, Backtrace)> {
    // Perform the operation that may panic.
    std::panic::catch_unwind(panic::AssertUnwindSafe(f)).map_err(|_| {
        // Get the stored panic and backtrace from the thread-local variable.
        PANIC_INFO.with(|info| info.take()).expect("No panic information stored?")
    })
}

#[cfg(test)]
mod tests {
    use super::{PrettyUnwrap, catch_unwind, flatten_error, pretty_panic, set_panic_hook, try_vm_runtime};

    use anyhow::{Context, Result, anyhow, bail};
    use colored::Colorize;

    const ERRORS: [&str; 3] = ["Third error", "Second error", "First error"];

    #[test]
    fn test_flatten_error() {
        // First error should be printed regularly, the other two dimmed.
        let expected = format!("{}{}", ERRORS[0], format!(" — {} — {}", ERRORS[1], ERRORS[2]).dimmed());

        let my_error = anyhow!(ERRORS[2]).context(ERRORS[1]).context(ERRORS[0]);
        let result = flatten_error(&my_error);

        assert_eq!(result, expected);
    }

    #[test]
    fn chained_error_panic_format() {
        let expected = format!("⚠️ {}\n     ↳ {}\n     ↳ {}", ERRORS[0], ERRORS[1], ERRORS[2]);

        let result = std::panic::catch_unwind(|| {
            let my_error = anyhow!(ERRORS[2]).context(ERRORS[1]).context(ERRORS[0]);
            pretty_panic(&my_error);
        })
        .unwrap_err();

        assert_eq!(*result.downcast::<String>().expect("Error was not a string"), expected);
    }

    #[test]
    fn chained_pretty_unwrap_format() {
        let expected = format!("⚠️ {}\n     ↳ {}\n     ↳ {}", ERRORS[0], ERRORS[1], ERRORS[2]);

        // Also test `pretty_unwrap` and chaining errors across functions.
        let result = std::panic::catch_unwind(|| {
            fn level2() -> Result<()> {
                bail!(ERRORS[2]);
            }

            fn level1() -> Result<()> {
                level2().with_context(|| ERRORS[1])?;
                Ok(())
            }

            fn level0() -> Result<()> {
                level1().with_context(|| ERRORS[0])?;
                Ok(())
            }

            level0().pretty_unwrap();
        })
        .unwrap_err();

        assert_eq!(*result.downcast::<String>().expect("Error was not a string"), expected);
    }

    // Ensure catch_unwind stores the panic message as expected.
    #[test]
    fn test_catch_unwind() {
        set_panic_hook();
        let result = catch_unwind(move || {
            panic!("This is my message");
        });
        // Remove hook so test asserts work normally again.
        let _ = std::panic::take_hook();

        let (msg, bt) = result.expect_err("No panic caught");
        assert!(msg.ends_with("This is my message"));

        // This function should be in the panics backtrace
        assert!(bt.to_string().contains("test_catch_unwind"));
    }

    /// Ensure catch_unwind does not break `try_vm_runtime`.
    #[test]
    fn test_nested_with_try_vm_runtime() {
        set_panic_hook();

        let result = std::panic::catch_unwind(|| {
            // try_vm_runtime uses catch_unwind internally
            let vm_result = try_vm_runtime(|| {
                panic!("VM operation failed!");
            });

            assert!(vm_result.is_err(), "try_vm_runtime should catch VM panic");

            // We can handle the VM error gracefully
            "handled_vm_error"
        });

        assert!(result.is_ok(), "Should handle VM error gracefully");
        assert_eq!(result.unwrap(), "handled_vm_error");
    }
}
