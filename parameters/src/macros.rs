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

#[macro_export]
macro_rules! checksum {
    ($bytes: expr) => {{
        use sha2::Digest;
        hex::encode(&sha2::Sha256::digest($bytes))
    }};
}

#[macro_export]
macro_rules! checksum_error {
    ($expected: expr, $candidate: expr) => {
        Err($crate::errors::ParameterError::ChecksumMismatch($expected, $candidate))
    };
}

#[macro_export]
macro_rules! remove_file {
    ($filepath:expr) => {
        // Safely remove the corrupt file, if it exists.
        #[cfg(not(feature = "wasm"))]
        if std::path::PathBuf::from(&$filepath).exists() {
            match std::fs::remove_file(&$filepath) {
                Ok(()) => println!("Removed {:?}. Please retry the command.", $filepath),
                Err(err) => eprintln!("Failed to remove {:?}: {err}", $filepath),
            }
        }
    };
}

macro_rules! impl_store_and_remote_fetch {
    () => {
        #[cfg(not(feature = "wasm"))]
        fn store_bytes(buffer: &[u8], file_path: &std::path::Path) -> Result<(), $crate::errors::ParameterError> {
            use snarkvm_utilities::Write;

            #[cfg(not(feature = "no_std_out"))]
            {
                use colored::*;
                let output = format!("{:>15} - Storing file in {:?}", "Installation", file_path);
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

        #[cfg(not(feature = "wasm"))]
        fn remote_fetch(buffer: &mut Vec<u8>, url: &str) -> Result<(), $crate::errors::ParameterError> {
            let mut easy = curl::easy::Easy::new();
            easy.follow_location(true)?;
            easy.url(url)?;

            #[cfg(not(feature = "no_std_out"))]
            {
                use colored::*;

                let output = format!("{:>15} - Downloading \"{}\"", "Installation", url);
                println!("{}", output.dimmed());

                easy.progress(true)?;
                easy.progress_function(|total_download, current_download, _, _| {
                    let percent = (current_download / total_download) * 100.0;
                    let size_in_megabytes = total_download as u64 / 1_048_576;
                    let output = format!(
                        "\r{:>15} - {:.2}% complete ({:#} MB total)",
                        "Installation", percent, size_in_megabytes
                    );
                    print!("{}", output.dimmed());
                    true
                })?;
            }

            let mut transfer = easy.transfer();
            transfer.write_function(|data| {
                buffer.extend_from_slice(data);
                Ok(data.len())
            })?;
            Ok(transfer.perform()?)
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
            xhr.send().map_err(|_| {
                $crate::errors::ParameterError::Wasm("Download failed - XMLHttpRequest failed".to_string())
            })?;

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

macro_rules! impl_load_bytes_logic_local {
    ($filepath: expr, $buffer: expr, $expected_size: expr, $expected_checksum: expr) => {
        // Ensure the size matches.
        if $expected_size != $buffer.len() {
            remove_file!($filepath);
            return Err($crate::errors::ParameterError::SizeMismatch($expected_size, $buffer.len()));
        }

        // Ensure the checksum matches.
        let candidate_checksum = checksum!($buffer);
        if $expected_checksum != candidate_checksum {
            return checksum_error!($expected_checksum, candidate_checksum);
        }

        return Ok($buffer.to_vec());
    };
}

macro_rules! impl_load_bytes_logic_remote {
    ($remote_url: expr, $local_dir: expr, $filename: expr, $metadata: expr, $expected_checksum: expr, $expected_size: expr) => {
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
                let path = format!("(in {:?})", file_path);
                eprintln!(
                    "\n⚠️  \"{}\" does not exist. Downloading and storing it {}.\n",
                    $filename, path.dimmed()
                );
            }

            // Construct the URL.
            let url = format!("{}/{}", $remote_url, $filename);

            // Load remote file
            cfg_if::cfg_if! {
                if #[cfg(not(feature = "wasm"))] {
                    let mut buffer = vec![];
                    Self::remote_fetch(&mut buffer, &url)?;

                    // Ensure the checksum matches.
                    let candidate_checksum = checksum!(&buffer);
                    if $expected_checksum != candidate_checksum {
                        return checksum_error!($expected_checksum, candidate_checksum)
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
                } else if #[cfg(feature = "wasm")] {
                    let buffer = Self::remote_fetch(&url)?;

                    // Ensure the checksum matches.
                    let candidate_checksum = checksum!(&buffer);
                    if $expected_checksum != candidate_checksum {
                        return checksum_error!($expected_checksum, candidate_checksum)
                    }

                    buffer
                } else {
                    return Err($crate::errors::ParameterError::RemoteFetchDisabled);
                }
            }
        };

        // Ensure the size matches.
        if $expected_size != buffer.len() {
            remove_file!(file_path);
            return Err($crate::errors::ParameterError::SizeMismatch($expected_size, buffer.len()));
        }

        // Ensure the checksum matches.
        let candidate_checksum = checksum!(buffer.as_slice());
        if $expected_checksum != candidate_checksum {
            return checksum_error!($expected_checksum, candidate_checksum)
        }

        return Ok(buffer)
    }
}

#[macro_export]
macro_rules! impl_local {
    ($name: ident, $local_dir: expr, $fname: tt, "usrs") => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name;

        impl $name {
            pub const METADATA: &'static str = include_str!(concat!($local_dir, $fname, ".metadata"));

            pub fn load_bytes() -> Result<Vec<u8>, $crate::errors::ParameterError> {
                let metadata: serde_json::Value =
                    serde_json::from_str(Self::METADATA).expect("Metadata was not well-formatted");
                let expected_checksum: String =
                    metadata["checksum"].as_str().expect("Failed to parse checksum").to_string();
                let expected_size: usize =
                    metadata["size"].to_string().parse().expect("Failed to retrieve the file size");

                let _filepath = concat!($local_dir, $fname, ".", "usrs");
                let buffer = include_bytes!(concat!($local_dir, $fname, ".", "usrs"));

                impl_load_bytes_logic_local!(_filepath, buffer, expected_size, expected_checksum);
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
    ($name: ident, $local_dir: expr, $fname: tt, $ftype: tt) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name;

        impl $name {
            pub const METADATA: &'static str = include_str!(concat!($local_dir, $fname, ".metadata"));

            pub fn load_bytes() -> Result<Vec<u8>, $crate::errors::ParameterError> {
                let metadata: serde_json::Value =
                    serde_json::from_str(Self::METADATA).expect("Metadata was not well-formatted");
                let expected_checksum: String =
                    metadata[concat!($ftype, "_checksum")].as_str().expect("Failed to parse checksum").to_string();
                let expected_size: usize =
                    metadata[concat!($ftype, "_size")].to_string().parse().expect("Failed to retrieve the file size");

                let _filepath = concat!($local_dir, $fname, ".", $ftype);
                let buffer = include_bytes!(concat!($local_dir, $fname, ".", $ftype));

                impl_load_bytes_logic_local!(_filepath, buffer, expected_size, expected_checksum);
            }
        }

        paste::item! {
            #[cfg(test)]
            #[test]
            fn [< test_ $fname _ $ftype >]() {
                assert!($name::load_bytes().is_ok());
            }
        }
    };
}

#[macro_export]
macro_rules! impl_remote {
    ($name: ident, $remote_url: expr, $local_dir: expr, $fname: tt, "usrs") => {
        pub struct $name;

        impl $name {
            pub const METADATA: &'static str = include_str!(concat!($local_dir, $fname, ".metadata"));

            impl_store_and_remote_fetch!();

            pub fn load_bytes() -> Result<Vec<u8>, $crate::errors::ParameterError> {
                let metadata: serde_json::Value =
                    serde_json::from_str(Self::METADATA).expect("Metadata was not well-formatted");
                let expected_checksum: String =
                    metadata["checksum"].as_str().expect("Failed to parse checksum").to_string();
                let expected_size: usize =
                    metadata["size"].to_string().parse().expect("Failed to retrieve the file size");

                // Construct the versioned filename.
                let filename = match expected_checksum.get(0..7) {
                    Some(sum) => format!("{}.{}.{}", $fname, "usrs", sum),
                    _ => format!("{}.{}", $fname, "usrs"),
                };

                impl_load_bytes_logic_remote!(
                    $remote_url,
                    $local_dir,
                    &filename,
                    metadata,
                    expected_checksum,
                    expected_size
                );
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
    ($name: ident, $remote_url: expr, $local_dir: expr, $fname: tt, $ftype: tt) => {
        pub struct $name;

        impl $name {
            pub const METADATA: &'static str = include_str!(concat!($local_dir, $fname, ".metadata"));

            impl_store_and_remote_fetch!();

            pub fn load_bytes() -> Result<Vec<u8>, $crate::errors::ParameterError> {
                let metadata: serde_json::Value =
                    serde_json::from_str(Self::METADATA).expect("Metadata was not well-formatted");
                let expected_checksum: String =
                    metadata[concat!($ftype, "_checksum")].as_str().expect("Failed to parse checksum").to_string();
                let expected_size: usize =
                    metadata[concat!($ftype, "_size")].to_string().parse().expect("Failed to retrieve the file size");

                // Construct the versioned filename.
                let filename = match expected_checksum.get(0..7) {
                    Some(sum) => format!("{}.{}.{}", $fname, $ftype, sum),
                    _ => format!("{}.{}", $fname, $ftype),
                };

                impl_load_bytes_logic_remote!(
                    $remote_url,
                    $local_dir,
                    &filename,
                    metadata,
                    expected_checksum,
                    expected_size
                );
            }
        }

        paste::item! {
            #[cfg(test)]
            #[test]
            fn [< test_ $fname _ $ftype >]() {
                assert!($name::load_bytes().is_ok());
            }
        }
    };
}
