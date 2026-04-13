// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Proc-macro crate for witgenstein component generation.
//!
//! Provides two macros:
//!
//! - `#[export]` — marks functions and `impl` blocks for export.  Identity
//!   transform; consumed by `component!`.
//! - `component! { ... }` — parses all items inside the braces, discovers
//!   `#[export]` annotations, generates inline WIT and `wit_bindgen` glue so
//!   that `cargo build --target wasm32-wasip2` produces a working component.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Item};

mod extract;
mod glue;
mod wit;

/// Mark a function or `impl` block for export in the generated WIT interface.
///
/// This attribute is consumed by the [`component!`] macro.  On its own it is an
/// identity transform — it re-emits the decorated item unchanged.
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

/// Generate a WIT component from the annotated items.
///
/// Wraps a set of exported items (functions, `impl` blocks) and generates the
/// `wit_bindgen` glue code so that `cargo build --target wasm32-wasip2`
/// produces a working WASI component.
///
/// # Automatic type discovery
///
/// Types (structs, enums) do **not** need to be inside `component!`.  The macro
/// scans the crate's source files at expansion time and automatically discovers
/// definitions used in exported function signatures — including transitively
/// referenced types (e.g. a record field that is itself an enum).
///
/// Types *may* still be placed inside `component!` and will take precedence
/// over external definitions of the same name.
///
/// # Configuration
///
/// Use inner attributes to configure the WIT package and interface names:
///
/// ```ignore
/// pub struct Point { pub x: f64, pub y: f64 }
///
/// witgenstein_rs_macros::component! {
///     #![package("myorg:my-pkg@1.0.0")]
///     #![interface("my-api")]
///
///     // Point is automatically included as a WIT record because
///     // it appears in the return type of an exported function.
///     #[export]
///     pub fn origin() -> Point { Point { x: 0.0, y: 0.0 } }
/// }
/// ```
///
/// Both are optional.  Defaults: `package("component:pkg")`,
/// `interface("exports")`.
#[proc_macro]
pub fn component(input: TokenStream) -> TokenStream {
    let input2: TokenStream2 = input.into();
    match component_impl(input2) {
        Ok(ts) => ts.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Configuration extracted from inner attributes.
struct ComponentConfig {
    /// Full WIT package id, e.g. `"myorg:my-pkg@1.0.0"`.
    package: String,
    /// WIT interface name, e.g. `"my-api"`.
    interface: String,
}

impl Default for ComponentConfig {
    fn default() -> Self {
        Self {
            package: "component:pkg".into(),
            interface: "exports".into(),
        }
    }
}

fn parse_config(attrs: &[syn::Attribute]) -> syn::Result<ComponentConfig> {
    let mut cfg = ComponentConfig::default();
    for attr in attrs {
        if attr.path().is_ident("package") {
            let value: syn::LitStr = attr.parse_args()?;
            cfg.package = value.value();
        } else if attr.path().is_ident("interface") {
            let value: syn::LitStr = attr.parse_args()?;
            cfg.interface = value.value();
        }
    }
    Ok(cfg)
}

fn component_impl(input: TokenStream2) -> syn::Result<TokenStream2> {
    // Parse the body as a sequence of items (inner attributes + items).
    let file: syn::File = syn::parse2(input)?;
    let config = parse_config(&file.attrs)?;
    let items = &file.items;

    // Discover #[export] annotations and collect type information.
    let mut discovery = extract::TypeDiscovery::new();
    discovery.scan_crate_sources();
    discovery.process_items(items);

    // If there are no exports, just re-emit the items with no glue.
    if discovery.functions.is_empty() && discovery.impls.is_empty() {
        let stripped = strip_export_attrs(items);
        return Ok(quote! { #(#stripped)* });
    }

    // Generate WIT.
    let wit_content = wit::generate_wit(&discovery, &config.package, &config.interface);

    // Generate glue code.
    let glue_tokens =
        glue::generate_glue(&discovery, &wit_content, &config.package, &config.interface);

    // Re-emit original items with #[export] stripped.
    let stripped = strip_export_attrs(items);

    Ok(quote! {
        #(#stripped)*

        // --- witgenstein generated glue (wasm32 only) ---
        #[cfg(target_arch = "wasm32")]
        mod __witgenstein_glue {
            use super::*;
            #glue_tokens
        }
    })
}

/// Strip `#[export]` attributes from items so they don't cause "unresolved"
/// errors when the proc-macro crate isn't in scope for downstream compilation.
fn strip_export_attrs(items: &[Item]) -> Vec<Item> {
    items
        .iter()
        .map(|item| {
            let mut item = item.clone();
            match &mut item {
                Item::Fn(f) => {
                    f.attrs.retain(|a| !is_export_attr(a));
                }
                Item::Impl(imp) => {
                    imp.attrs.retain(|a| !is_export_attr(a));
                }
                _ => {}
            }
            item
        })
        .collect()
}

fn is_export_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("export")
}
