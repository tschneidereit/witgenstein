// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Diagnostic types for reporting unmappable types with scaffolded fix suggestions.

use std::fmt;

/// The kind of diagnostic.
#[derive(Debug, Clone, Copy)]
pub enum DiagnosticKind {
    /// A bare `number` type without a branded type annotation.
    AmbiguousNumeric,
    /// A type that has no WIT equivalent (any, unknown, function types, etc.)
    UnsupportedType,
    /// A type reference that could not be resolved.
    UnresolvableType,
}

/// A diagnostic message about a type mapping issue.
#[derive(Debug)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    pub context: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind_label = match self.kind {
            DiagnosticKind::AmbiguousNumeric => "ambiguous type",
            DiagnosticKind::UnsupportedType => "unsupported type",
            DiagnosticKind::UnresolvableType => "unresolvable type",
        };
        write!(f, "{kind_label}: {}", self.message)
    }
}

impl Diagnostic {
    /// Generate a help message with scaffolded fix code.
    pub fn help(&self) -> String {
        match self.kind {
            DiagnosticKind::AmbiguousNumeric => "Import a branded type from 'ts-auto-wit':\n\
                     \n\
                     \x20 import { u32 } from 'ts-auto-wit';\n\
                     \n\
                     Then use it instead of `number`. Available types:\n\
                     \x20 u8, u16, u32, u64, s8, s16, s32, s64, f32, f64"
                .to_string(),
            DiagnosticKind::UnsupportedType => {
                "This type has no WIT equivalent. Consider refactoring to use\n\
                     a supported type (record, enum, variant, resource, etc.)."
                    .to_string()
            }
            DiagnosticKind::UnresolvableType => {
                "Ensure all types used in the public API have explicit type annotations.\n\
                     Add a type annotation or import the missing type."
                    .to_string()
            }
        }
    }
}

/// Print all diagnostics to stderr.
pub fn report_diagnostics(diagnostics: &[Diagnostic]) {
    for diag in diagnostics {
        eprintln!("warning: {diag}");
        let help = diag.help();
        for line in help.lines() {
            eprintln!("  help: {line}");
        }
        eprintln!();
    }
}
