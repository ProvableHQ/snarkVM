# Generates the translation proving and verifying key.

# Inputs: network

cargo run --release --example translation mainnet -- --nocapture || exit

mv translation_credits.metadata ../../src/mainnet/resources/credits || exit
mv translation_credits.prover.* ~/.aleo/resources || exit
mv translation_credits.verifier ../../src/mainnet/resources/credits || exit
