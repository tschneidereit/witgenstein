// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

use clap::Parser;
use std::path::PathBuf;

/// Generate WIT interface definitions from Rust source files.
#[derive(Parser, Debug)]
#[command(name = "rs-auto-wit", version, about)]
pub struct Args {
    /// Path to the output .wit file. Prints to stdout if omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// WIT package identifier (e.g. "myorg:my-pkg@1.0.0").
    /// Derived from Cargo.toml if omitted.
    #[arg(long)]
    pub package: Option<String>,

    /// Error on unsupported types instead of skipping them.
    #[arg(long)]
    pub strict: bool,

    /// Path to the Rust crate (directory containing Cargo.toml).
    /// Uses the current directory if omitted.
    pub input: Option<PathBuf>,
}
