// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Diagnostic types for reporting errors and warnings during WIT generation.

#![allow(dead_code)]

use std::path::PathBuf;

/// A diagnostic message produced during WIT generation.
#[derive(Debug)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    pub source_path: Option<PathBuf>,
}

/// The severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    Warning,
    Error,
}

impl Diagnostic {
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticKind::Warning,
            message: message.into(),
            source_path: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticKind::Error,
            message: message.into(),
            source_path: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.source_path = Some(path.into());
        self
    }
}
