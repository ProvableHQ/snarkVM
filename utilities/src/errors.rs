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

/// Generates an `io::Error` from the given string.
pub fn io_error<S: ToString>(err: S) -> std::io::Error {
    std::io::Error::other(err.to_string())
}

/// Generates an `io::Error` from the given `anyhow::Error`.
///
/// This will flatten the existing error chain so that it fits in a single-line string.
pub fn into_io_error<E: Into<anyhow::Error>>(err: E) -> std::io::Error {
    let err: anyhow::Error = err.into();
    std::io::Error::other(flatten_anyhow_error(&err))
}

/// Helper function for `log_error` and `log_warning`.
#[inline]
fn flatten_anyhow_error(error: &anyhow::Error) -> String {
    let mut output = error.to_string();
    for next in error.chain().skip(1) {
        output = format!("{output} — {next}");
    }
    output
}

/// Logs `anyhow::Error`'s its error chain using the `ERROR` log level.
///
/// This follows the existing convention in the codebase that joins errors using em dashes.
/// For example, an error "Invalid transaction" with a cause "Proof failed" would be logged
/// as "Invalid transaction — Proof failed".
pub fn log_error(error: &anyhow::Error) {
    tracing::error!("{}", flatten_anyhow_error(error));
}

/// Logs `anyhow::Error`'s its error chain using the `WARN` log level.
///
/// This follows the existing convention in the codebase that joins errors using em dashes.
/// For example, an error "Invalid transaction" with a cause "Proof failed" would be logged
/// as "Invalid transaction — Proof failed".
pub fn log_warning(error: &anyhow::Error) {
    tracing::warn!("{}", flatten_anyhow_error(error));
}

/// Displays an `anyhow::Error`'s main error and its error chain to stderr.
///
/// This can be used to show a "pretty" error to the end user.
pub fn display_error(error: &anyhow::Error) {
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

    /// Behaves like [`std::Result::unwrap`] but will print the entire anyhow chain to stderr.
    fn pretty_unwrap(self) -> Self::Inner;
}

/// Helper for `PrettyUnwrap`, which creates a panic with the `anyhow::Error` nicely formatted and also logs the panic.
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
    fn pretty_unwrap(self) -> Self::Inner {
        match self {
            Ok(result) => result,
            Err(error) => {
                pretty_panic(&error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PrettyUnwrap, flatten_anyhow_error, pretty_panic};

    use anyhow::{Context, Result, anyhow, bail};

    const ERRORS: [&str; 3] = ["Third error", "Second error", "First error"];

    #[test]
    fn flatten_error() {
        let expected = format!("{} — {} — {}", ERRORS[0], ERRORS[1], ERRORS[2]);

        let my_error = anyhow!(ERRORS[2]).context(ERRORS[1]).context(ERRORS[0]);
        let result = flatten_anyhow_error(&my_error);

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

    /// Ensure catch_unwind does not break `try_vm_runtime`.
    #[test]
    fn test_nested_with_try_vm_runtime() {
        use crate::try_vm_runtime;

        let result = std::panic::catch_unwind(|| {
            // try_vm_runtime uses catch_unwind internally
            let vm_result = try_vm_runtime!(|| {
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
