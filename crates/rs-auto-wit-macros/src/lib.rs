// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Proc-macro crate providing the `#[export]` attribute.
//!
//! The `#[export]` attribute marks functions and `impl` blocks for inclusion in
//! a generated WIT interface. It is an identity transform — the decorated item
//! is re-emitted unchanged, with zero runtime cost.
//!
//! `rs-auto-wit` discovers these annotations by scanning source files with `syn`
//! and then resolves their types via rustdoc JSON.
//!
//! # Examples
//!
//! ```ignore
//! use rs_auto_wit_macros::export;
//!
//! #[export]
//! pub fn greet(name: String) -> String {
//!     format!("Hello, {name}!")
//! }
//!
//! pub struct Counter {
//!     count: u32,
//! }
//!
//! #[export]
//! impl Counter {
//!     pub fn new(initial: u32) -> Self {
//!         Self { count: initial }
//!     }
//!
//!     pub fn increment(&mut self) {
//!         self.count += 1;
//!     }
//!
//!     pub fn value(&self) -> u32 {
//!         self.count
//!     }
//! }
//! ```

use proc_macro::TokenStream;

/// Mark a function or `impl` block for export in the generated WIT interface.
///
/// This attribute is a no-op identity transform — it re-emits the decorated
/// item unchanged. The `rs-auto-wit` tool discovers these markers by scanning
/// the source with `syn` and uses them to determine which items appear in the
/// generated WIT file.
///
/// # Supported targets
///
/// - **Functions**: `#[export] pub fn name(...) -> T { ... }`
/// - **Impl blocks**: `#[export] impl Type { ... }` (generates a WIT `resource`)
#[proc_macro_attribute]
pub fn export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Identity transform: re-emit the item unchanged.
    item
}
