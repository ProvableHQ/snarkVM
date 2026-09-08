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

/// Computes a SHA-256 digest of the given byte slice and returns it as a lowercase hex string.
/// Used to verify the integrity of downloaded or loaded parameter files.
#[inline(always)]
pub(crate) fn checksum(bytes: impl AsRef<[u8]>) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(bytes))
}

/// Constructs a `ParameterError::ChecksumMismatch` error from an expected and a computed checksum.
/// Used to report integrity failures when a loaded or downloaded file does not match its metadata.
/// Generic over `T` so it can be returned from any `Result<T, ParameterError>`-returning function.
#[cfg(any(feature = "filesystem", feature = "wasm"))]
#[inline(always)]
pub(crate) fn checksum_error<T>(expected: String, candidate: String) -> Result<T, crate::errors::ParameterError> {
    Err(crate::errors::ParameterError::ChecksumMismatch(expected, candidate))
}

/// Removes a parameter file from disk (on targets where the filesystem is available)
/// when it is found to be corrupt or mismatched.
/// Prints a message on success so the user knows to retry; logs a warning on failure.
#[inline(always)]
pub(crate) fn remove_file(filepath: impl AsRef<std::path::Path>) {
    cfg_if::cfg_if! {
        if #[cfg(feature="wasm")] {
            // No-op on wasm targets where filesystem access is unavailable.
            let _ = filepath;
        } else {
            let filepath = filepath.as_ref();
            if filepath.exists() {
                match std::fs::remove_file(filepath) {
                    Ok(()) => println!("Removed {:?}. Please retry the command.", filepath),
                    Err(err) => eprintln!("Failed to remove {:?}: {err}", filepath),
                }
            }
        }
    }
}

/// Validates a locally loaded byte buffer against an expected size and checksum, then returns it.
/// On a size mismatch the cached file is removed. On a checksum mismatch a `ChecksumMismatch`
/// error is returned without removing the file.
/// Used inside `impl_local!` after reading a compile-time embedded parameter file.
#[inline(always)]
pub(crate) fn load_bytes_local(
    filepath: &str,
    buffer: &[u8],
    expected_size: usize,
    expected_checksum: &str,
) -> Result<Vec<u8>, crate::errors::ParameterError> {
    if expected_size != buffer.len() {
        remove_file(filepath);
        return Err(crate::errors::ParameterError::SizeMismatch(expected_size, buffer.len()));
    }
    let candidate_checksum = checksum(buffer);
    if expected_checksum != candidate_checksum {
        return Err(crate::errors::ParameterError::ChecksumMismatch(expected_checksum.to_string(), candidate_checksum));
    }
    Ok(buffer.to_vec())
}

/// Injects `store_bytes` and `remote_fetch` helper methods into the enclosing `impl` block.
///
/// - `store_bytes` writes a parameter buffer to a local path, creating any missing directories.
///   Disabled on wasm targets.
/// - `remote_fetch` downloads a parameter file from a URL into a provided buffer. On native
///   targets it uses a blocking `reqwest` client with per-host retry logic; on wasm it uses a
///   synchronous `XmlHttpRequest` with ISO-8859-5 encoding to preserve raw bytes. Disabled on
///   SGX targets.
///
/// Used internally by `impl_remote!` to provide download and caching support.
macro_rules! impl_store_and_remote_fetch {
    () => {
        #[cfg(all(feature = "filesystem", not(feature = "wasm")))]
        fn store_bytes(buffer: &[u8], file_path: &std::path::Path) -> Result<(), $crate::errors::ParameterError> {
            use snarkvm_utilities::Write;

            #[cfg(not(feature = "no_std_out"))]
            {
                use colored::*;
                let output = format!("{:>15} - Storing file in \"{}\"", "Installation", file_path.display());
                println!("{}", output.dimmed());
            }

            // Ensure the folders up to the file path all exist.
            let mut directory_path = file_path.to_path_buf();
            directory_path.pop();
            let _ = std::fs::create_dir_all(directory_path)?;

            // Attempt to write the parameter buffer to a file.
            match std::fs::File::create(file_path) {
                Ok(mut file) => file.write_all(&buffer)?,
                Err(error) => eprintln!("{}", error),
            }
            Ok(())
        }

        #[cfg(all(feature = "filesystem", not(feature = "wasm"), not(target_env = "sgx")))]
        fn remote_fetch(buffer: &mut Vec<u8>, url: &str) -> Result<(), $crate::errors::ParameterError> {
            use std::io::Read;

            #[cfg(not(feature = "no_std_out"))]
            {
                use colored::*;
                let output = format!("{:>15} - Downloading \"{url}\"", "Installation");
                println!("{}", output.dimmed());
            }

            // Retry up to 3 times on transient errors (5xx, 429, IO, timeout).
            let mut attempts = 3u32;
            loop {
                match ureq::get(url).config().max_redirects(10).build().call() {
                    Ok(mut response) => {
                        response.body_mut().as_reader().read_to_end(buffer)?;
                        break;
                    }
                    Err(ureq::Error::StatusCode(code)) if attempts > 0 && (code >= 500 || code == 429) => {
                        attempts -= 1;
                    }
                    Err(ureq::Error::Io(_) | ureq::Error::Timeout(_)) if attempts > 0 => {
                        attempts -= 1;
                    }
                    Err(err) => return Err(err.into()),
                }
            }

            #[cfg(not(feature = "no_std_out"))]
            {
                use colored::*;
                let size_in_megabytes = buffer.len() as u64 / 1_048_576;
                let output = format!("{:>15} - Download complete ({size_in_megabytes} MB)", "Installation");
                println!("{}", output.dimmed());
            }

            Ok(())
        }

        #[cfg(feature = "wasm")]
        fn remote_fetch(url: &str) -> Result<Vec<u8>, $crate::errors::ParameterError> {
            // Use the browser's XmlHttpRequest object to download the parameter file synchronously.
            //
            // This method blocks the event loop while the parameters are downloaded, and should be
            // executed in a web worker to prevent the main browser window from freezing.
            let xhr = web_sys::XmlHttpRequest::new().map_err(|_| {
                $crate::errors::ParameterError::Wasm("Download failed - XMLHttpRequest object not found".to_string())
            })?;

            // XmlHttpRequest if specified as synchronous cannot use the responseType property. It
            // cannot thus download bytes directly and enforces a text encoding. To get back the
            // original binary, a charset that does not corrupt the original bytes must be used.
            xhr.override_mime_type("octet/binary; charset=ISO-8859-5").unwrap();

            // Initialize and send the request.
            xhr.open_with_async("GET", url, false).map_err(|_| {
                $crate::errors::ParameterError::Wasm(
                    "Download failed - This browser does not support synchronous requests".to_string(),
                )
            })?;
            xhr.send()
                .map_err(|_| $crate::errors::ParameterError::Wasm("Download failed - XMLHttpRequest failed".to_string()))?;

            // Wait for the response in a blocking fashion.
            if xhr.response().is_ok() && xhr.status().unwrap() == 200 {
                // Get the text from the response.
                let rust_text = xhr
                    .response_text()
                    .map_err(|_| $crate::errors::ParameterError::Wasm("XMLHttpRequest failed".to_string()))?
                    .ok_or($crate::errors::ParameterError::Wasm(
                        "The request was successful but no parameters were received".to_string(),
                    ))?;

                // Re-encode the text back into bytes using the chosen encoding.
                use encoding::Encoding;
                encoding::all::ISO_8859_5
                    .encode(&rust_text, encoding::EncoderTrap::Strict)
                    .map_err(|_| $crate::errors::ParameterError::Wasm("Parameter decoding failed".to_string()))
            } else {
                Err($crate::errors::ParameterError::Wasm("Download failed - XMLHttpRequest failed".to_string()))
            }
        }
    };
}

/// Implements the full remote-load flow for a parameter file.
///
/// On native `filesystem` targets, it serves from the local cache directory if present; otherwise
/// iterates through `$remote_urls` in order, retrying on transient errors, and caches the first
/// verified download to disk.
///
/// On WASM targets, it terates through `$remote_urls` using `XmlHttpRequest`, verifying the
/// checksum after each attempt and returning on the first success.
/// In all cases the buffer is validated against `$expected_size` and `$expected_checksum` before
/// being returned.
///
/// On SGX targets: it will attempt to load the paramtes for local directory.
/// The function will return an error in cases where the `filesystem` feature is disabled or no parameter file exists locally, as remote fetch is not supported on SGX.
///
/// Used inside `impl_remote!`.
macro_rules! impl_load_bytes_logic_remote {
    ($remote_urls: expr, $local_dir: expr, $filename: expr, $metadata: expr, $expected_checksum: expr, $expected_size: expr) => {
        cfg_if::cfg_if! {
            if #[cfg(feature = "wasm")] {
                // Try each URL in order, falling back to the next if one fails.
                let remote_urls: &[&str] = &$remote_urls;
                let mut buffer = vec![];
                let mut last_error: Option<$crate::errors::ParameterError> = None;

                for base_url in remote_urls.iter() {
                    let url = format!("{base_url}/{}", $filename);

                    match Self::remote_fetch(&url) {
                        Ok(fetched_buffer) => {
                            // Ensure the checksum matches.
                            let candidate_checksum = $crate::macros::checksum(&fetched_buffer);
                            if $expected_checksum == candidate_checksum {
                                buffer = fetched_buffer;
                                last_error = None;
                                break;
                            } else {
                                last_error = Some($crate::errors::ParameterError::ChecksumMismatch(
                                    $expected_checksum.to_string(),
                                    candidate_checksum,
                                ));
                            }
                        }
                        Err(e) => {
                            last_error = Some(e);
                        }
                    }
                }

                // If all URLs failed, return the last error.
                if let Some(e) = last_error {
                    return Err(e);
                }

                // Ensure the size matches.
                if $expected_size != buffer.len() {
                    return Err($crate::errors::ParameterError::SizeMismatch($expected_size, buffer.len()));
                }

                return Ok(buffer)
            } else if #[cfg(all(feature = "filesystem", target_env="sgx"))] {
                // Compose the correct file path for the parameter file.
                let mut file_path = aleo_std::aleo_dir();
                file_path.push($local_dir);
                file_path.push($filename);

                let buffer = if file_path.exists() {
                    // Attempts to load the parameter file locally with an absolute path.
                    std::fs::read(&file_path)?
                } else {
                    // Cannot remote fetch on SGX.
                    return Err($crate::errors::ParameterError::RemoteFetchDisabled);
                };

                // Ensure the size matches.
                if $expected_size != buffer.len() {
                    $crate::macros::remove_file(&file_path);
                    return Err($crate::errors::ParameterError::SizeMismatch($expected_size, buffer.len()));
                }

                // Ensure the checksum matches.
                let candidate_checksum = $crate::macros::checksum(buffer.as_slice());
                if $expected_checksum != candidate_checksum {
                    return $crate::macros::checksum_error($expected_checksum, candidate_checksum)
                }
                return Ok(buffer);
            } else if #[cfg(feature="filesystem")] {
                // Compose the correct file path for the parameter file.
                let mut file_path = aleo_std::aleo_dir();
                file_path.push($local_dir);
                file_path.push($filename);

                let buffer = if file_path.exists() {
                    // Attempts to load the parameter file locally with an absolute path.
                    std::fs::read(&file_path)?
                } else {
                    // Downloads the missing parameters and stores it in the local directory for use.
                    #[cfg(not(feature = "no_std_out"))]
                    {
                        use colored::*;
                        let path = format!("(in \"{}\")", file_path.display());
                        eprintln!(
                            "\n⚠️  \"{}\" does not exist. Downloading and storing it {}.\n",
                            $filename, path.dimmed()
                        );
                    }

                    // -- Load remote file --
                    // Try each URL in order, falling back to the next if one fails.
                    let remote_urls: &[&str] = &$remote_urls;
                    let mut buffer = vec![];
                    let mut last_error: Option<($crate::errors::ParameterError, &str)> = None;

                    for base_url in remote_urls.iter() {
                        // Remove the previous error (if any).
                        cfg_if::cfg_if!{
                            if #[cfg(feature = "no_std_out")] {
                                last_error = None;
                            } else {
                                use colored::Colorize;
                                // If this is a retry, print the previous error as warning.
                                if let Some((err, url)) = last_error.take() {
                                    eprintln!("{:>15} - {err}", "Warning".yellow());
                                    eprintln!("{:>15} - Failed to fetch from \"{url}\". Trying next source...", "Warning".yellow());
                                 }
                            }
                        }

                        let url = format!("{base_url}/{}", $filename);
                        buffer.clear();

                        match Self::remote_fetch(&mut buffer, &url) {
                            Ok(()) => {
                                // Ensure the checksum matches.
                                let candidate_checksum = $crate::macros::checksum(&buffer);
                                if $expected_checksum == candidate_checksum {
                                    // Success - break out of the loop
                                    break;
                                } else {
                                    last_error = Some(($crate::errors::ParameterError::ChecksumMismatch(
                                        $expected_checksum.to_string(),
                                        candidate_checksum,
                                    ), base_url));
                                }
                            }
                            Err(err) => {
                                last_error = Some((err, base_url));
                            }
                        }
                    }

                    // If all URLs failed, return the last error.
                    if let Some((err, _)) = last_error {
                        return Err(err);
                    }

                    match Self::store_bytes(&buffer, &file_path) {
                        Ok(()) => buffer,
                        Err(_) => {
                            eprintln!(
                                "\n❗ Error - Failed to store \"{}\" locally. Please download this file manually and ensure it is stored in {:?}.\n",
                                $filename, file_path
                            );
                            buffer
                        }
                    }
                };

                // Ensure the size matches.
                if $expected_size != buffer.len() {
                    $crate::macros::remove_file(&file_path);
                    return Err($crate::errors::ParameterError::SizeMismatch($expected_size, buffer.len()));
                }

                // Ensure the checksum matches.
                let candidate_checksum = $crate::macros::checksum(buffer.as_slice());
                if $expected_checksum != candidate_checksum {
                    return $crate::macros::checksum_error($expected_checksum, candidate_checksum)
                }
                return Ok(buffer);
            } else {
                // We need either the `filesystem` or `wasm` feature to load parameters.
                return Err($crate::errors::ParameterError::FilesystemDisabled);
            }
        }
    }
}

/// Generates a parameter struct whose bytes are embedded at compile time via `include_bytes!`.
///
/// Two variants:
/// - `($name, $local_dir, $fname, "usrs")` — for universal reference string (`.usrs`) files.
///   Metadata is read from `$local_dir/$fname.metadata`.
/// - `($name, $local_dir, $fname, $ftype, $credits_version)` — for versioned parameter files.
///   Metadata is read from `$local_dir/$credits_version/$fname.metadata`, and the checksum/size
///   keys in the metadata are prefixed with `$ftype_`.
///
/// Both variants expose:
/// - `METADATA: &'static str` — the raw JSON metadata string.
/// - `load_bytes() -> Result<Vec<u8>, ParameterError>` — returns the embedded bytes after
///   verifying size and checksum.
///
/// A compile-time test is also generated to ensure the embedded bytes load successfully.
#[macro_export]
macro_rules! impl_local {
    ($name: ident, $local_dir: expr, $fname: tt, "usrs") => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name;

        impl $name {
            pub const METADATA: &'static str = include_str!(concat!($local_dir, $fname, ".metadata"));

            pub fn load_bytes() -> Result<Vec<u8>, $crate::errors::ParameterError> {
                let metadata: serde_json::Value = serde_json::from_str(Self::METADATA).expect("Metadata was not well-formatted");
                let expected_checksum: String = metadata["checksum"].as_str().expect("Failed to parse checksum").to_string();
                let expected_size: usize = metadata["size"].to_string().parse().expect("Failed to retrieve the file size");

                let filepath = concat!($local_dir, $fname, ".", "usrs");
                let buffer = include_bytes!(concat!($local_dir, $fname, ".", "usrs"));

                $crate::macros::load_bytes_local(filepath, buffer, expected_size, &expected_checksum)
            }
        }

        paste::item! {
            #[cfg(test)]
            #[test]
            fn [< test_ $fname _usrs >]() {
                // Print error messages if loading fails. This can be simplified once assert_matches! is stable.
                if let Err(err) = $name::load_bytes() {
                    panic!("Failed to load bytes: {err}");
                }
            }
        }
    };
    ($name: ident, $local_dir: expr, $fname: tt, $ftype: tt, $credits_version: tt) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name;

        impl $name {
            pub const METADATA: &'static str = include_str!(concat!($local_dir, $credits_version, "/", $fname, ".metadata"));

            pub fn load_bytes() -> Result<Vec<u8>, $crate::errors::ParameterError> {
                let metadata: serde_json::Value = serde_json::from_str(Self::METADATA).expect("Metadata was not well-formatted");
                let expected_checksum: String =
                    metadata[concat!($ftype, "_checksum")].as_str().expect("Failed to parse checksum").to_string();
                let expected_size: usize =
                    metadata[concat!($ftype, "_size")].to_string().parse().expect("Failed to retrieve the file size");

                let filepath = concat!($local_dir, $credits_version, "/", $fname, ".", $ftype);
                let buffer = include_bytes!(concat!($local_dir, $credits_version, "/", $fname, ".", $ftype));

                $crate::macros::load_bytes_local(filepath, buffer, expected_size, &expected_checksum)
            }
        }

        paste::item! {
            #[cfg(test)]
            #[test]
            fn [< test_ $credits_version _ $fname _ $ftype >]() {
                if let Err(err) = $name::load_bytes() {
                    panic!("Failed to load bytes: {err}");
                }
            }
        }
    };
}

/// Generates a parameter struct whose bytes are fetched from a remote URL and cached locally.
///
/// Two variants:
/// - `($name, $remote_url, $local_dir, $fname, "usrs")` — for universal reference string
///   (`.usrs`) files. Metadata lives at `$local_dir/$fname.metadata`.
/// - `($name, $remote_url, $local_dir, $fname, $ftype, $credits_version)` — for versioned
///   parameter files. Metadata lives at `$local_dir/$credits_version/$fname.metadata`.
///   On wasm this variant also exposes `verify_bytes(buffer)` to validate externally supplied
///   bytes against the embedded metadata without performing a download.
///
/// Both variants expose:
/// - `METADATA: &'static str` — the raw JSON metadata string.
/// - `load_bytes() -> Result<Vec<u8>, ParameterError>` — serves from the local cache when
///   available, otherwise downloads from `$remote_url`, caches the result, and verifies size and
///   checksum before returning.
///
/// A test is also generated that calls `load_bytes()` to verify the download path.
#[macro_export]
macro_rules! impl_remote {
    ($name: ident, $remote_url: expr, $local_dir: expr, $fname: tt, "usrs") => {
        pub struct $name;

        impl $name {
            pub const METADATA: &'static str = include_str!(concat!($local_dir, $fname, ".metadata"));

            impl_store_and_remote_fetch!();

            pub fn load_bytes() -> Result<Vec<u8>, $crate::errors::ParameterError> {
                let metadata: serde_json::Value = serde_json::from_str(Self::METADATA).expect("Metadata was not well-formatted");
                let expected_checksum: String = metadata["checksum"].as_str().expect("Failed to parse checksum").to_string();
                let expected_size: usize = metadata["size"].to_string().parse().expect("Failed to retrieve the file size");

                // Construct the versioned filename.
                let filename = match expected_checksum.get(0..7) {
                    Some(sum) => format!("{}.{}.{}", $fname, "usrs", sum),
                    _ => format!("{}.{}", $fname, "usrs"),
                };
                let _ = (&expected_size, &filename);

                impl_load_bytes_logic_remote!($remote_url, $local_dir, &filename, metadata, expected_checksum, expected_size);
            }
        }
        paste::item! {
            #[cfg(test)]
            #[test]
            fn [< test_ $fname _usrs >]() {
                assert!($name::load_bytes().is_ok());
            }
        }
    };
    ($name: ident, $remote_url: expr, $local_dir: expr, $fname: tt, $ftype: tt, $credits_version: tt) => {
        pub struct $name;

        impl $name {
            pub const METADATA: &'static str = include_str!(concat!($local_dir, $credits_version, "/", $fname, ".metadata"));

            impl_store_and_remote_fetch!();

            pub fn load_bytes() -> Result<Vec<u8>, $crate::errors::ParameterError> {
                let metadata: serde_json::Value = serde_json::from_str(Self::METADATA).expect("Metadata was not well-formatted");
                let expected_checksum: String =
                    metadata[concat!($ftype, "_checksum")].as_str().expect("Failed to parse checksum").to_string();
                let expected_size: usize =
                    metadata[concat!($ftype, "_size")].to_string().parse().expect("Failed to retrieve the file size");

                // Construct the versioned filename.
                let filename = match expected_checksum.get(0..7) {
                    Some(sum) => format!("{}.{}.{}", $fname, $ftype, sum),
                    _ => format!("{}.{}", $fname, $ftype),
                };
                let _ = (&expected_size, &filename);

                impl_load_bytes_logic_remote!($remote_url, $local_dir, &filename, metadata, expected_checksum, expected_size);
            }

            #[cfg(feature = "wasm")]
            /// Verify external bytes.
            pub fn verify_bytes(buffer: &[u8]) -> Result<(), $crate::errors::ParameterError> {
                let metadata: serde_json::Value = serde_json::from_str(Self::METADATA).expect("Metadata was not well-formatted");
                let expected_checksum: String =
                    metadata[concat!($ftype, "_checksum")].as_str().expect("Failed to parse checksum").to_string();
                let expected_size: usize =
                    metadata[concat!($ftype, "_size")].to_string().parse().expect("Failed to retrieve the file size");

                // Ensure the size matches.
                if buffer.len() != expected_size {
                    return Err($crate::errors::ParameterError::SizeMismatch(expected_size, buffer.len()));
                }

                // Ensure the checksum matches.
                let candidate_checksum = $crate::macros::checksum(buffer);
                if expected_checksum != candidate_checksum {
                    return $crate::macros::checksum_error(expected_checksum, candidate_checksum);
                }
                Ok(())
            }
        }

        paste::item! {
            #[cfg(test)]
            #[test]
            fn [< test_ $credits_version _ $fname _ $ftype >]() {
                if let Err(err) = $name::load_bytes() {
                    panic!("Failed to load bytes: {err}");
                }
            }
        }
    };
}
