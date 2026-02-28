// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Generate WIT interface definitions from Rust source files, and optionally
/// build wasm32-wasip2 components.
#[derive(Parser, Debug)]
#[command(name = "witgenstein", version, about)]
pub struct Cli {
    /// Enable verbose output.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Generate WIT interface definitions from a Rust crate.
    Generate(GenerateArgs),
    /// Build a wasm32-wasip2 component from a Rust crate with #\[export\] annotations.
    Build(BuildArgs),
}

/// Arguments for the `generate` subcommand.
#[derive(Parser, Debug)]
pub struct GenerateArgs {
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

/// Arguments for the `build` subcommand.
#[derive(Parser, Debug)]
pub struct BuildArgs {
    /// WIT package identifier (e.g. "myorg:my-pkg@1.0.0").
    /// Derived from Cargo.toml if omitted.
    #[arg(long)]
    pub package: Option<String>,

    /// Build in release mode.
    #[arg(long)]
    pub release: bool,

    /// Path to the Rust crate (directory containing Cargo.toml).
    /// Uses the current directory if omitted.
    pub input: Option<PathBuf>,
}
