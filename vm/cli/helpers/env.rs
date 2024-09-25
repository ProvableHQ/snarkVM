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

use crate::{cli::CurrentNetwork, console::account::PrivateKey};

use anyhow::{anyhow, Result};

fn env_template() -> String {
    r#"
NETWORK=mainnet
PRIVATE_KEY={{PASTE_YOUR_PRIVATE_KEY_HERE}}
"#
    .to_string()
}

/// Loads the environment variables from the .env file.
fn dotenv_load() -> Result<()> {
    // Load environment variables from .env file.
    // Fails if .env file not found, not readable or invalid.
    dotenvy::dotenv().map_err(|_| {
        anyhow!(
            "Missing a '.env' file. Create the '.env' file in your package's root directory with the following:\n\n{}\n",
            env_template()
        )
    })?;
    Ok(())
}

/// Returns the private key from the environment.
pub fn dotenv_private_key() -> Result<PrivateKey<CurrentNetwork>> {
    if cfg!(test) {
        let rng = &mut crate::utilities::TestRng::fixed(123456789);
        PrivateKey::<CurrentNetwork>::new(rng)
    } else {
        use std::str::FromStr;
        dotenv_load()?;
        // Load the private key from the environment.
        let private_key = dotenvy::var("PRIVATE_KEY").map_err(|e| anyhow!("Missing PRIVATE_KEY - {e}"))?;
        // Parse the private key.
        PrivateKey::<CurrentNetwork>::from_str(&private_key)
    }
}
