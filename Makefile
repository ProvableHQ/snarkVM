PHONY: test_private test_public bench_private bench_public

test_private:
	cargo test test_transfer_private_execution --package snarkvm-synthesizer --lib --features cuda --release -- --nocapture

test_public:
	cargo test test_transfer_public_execution --package snarkvm-synthesizer --lib --features cuda --release -- --nocapture

bench_private:
	cargo bench --bench transfer_private --features cuda -p snarkvm-synthesizer

bench_public:
	cargo bench --bench transfer_public --features cuda -p snarkvm-synthesizer