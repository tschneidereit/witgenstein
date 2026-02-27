// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

use clap::Parser;
use std::path::PathBuf;

/// Generate WIT interface definitions from TypeScript source files.
#[derive(Parser, Debug)]
#[command(name = "ts-auto-wit", version, about)]
pub struct Args {
    /// Path to the output .wit file. Prints to stdout if omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// WIT package identifier (e.g. "myorg:my-pkg@1.0.0").
    /// Derived from package.json if omitted.
    #[arg(long)]
    pub package: Option<String>,

    /// Error on ambiguous types (e.g. bare `number`) instead of using defaults.
    #[arg(long)]
    pub strict: bool,

    /// Input path: a TypeScript file, a directory containing a TS project, or
    /// omitted to use the current working directory.
    pub input: Option<PathBuf>,
}
