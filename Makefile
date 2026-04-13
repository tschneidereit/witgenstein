# SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

.PHONY: all fmt clippy test check clean

all: fmt clippy test

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets

test:
	cargo test --workspace

check: fmt clippy test

clean:
	cargo clean
