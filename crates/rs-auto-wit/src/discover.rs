// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Discover `#[export]`-annotated items by scanning Rust source files with `syn`.
//!
//! This module parses each `.rs` file in the target crate and walks the AST to
//! find functions and impl blocks carrying the `#[export]` attribute. It
//! collects their names so `resolve.rs` can look them up in rustdoc JSON.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use syn::visit::Visit;
use syn::{Attribute, File, Ident, ItemFn, ItemImpl, ItemMod};

/// An exported item discovered from source scanning.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExportedItem {
    /// A standalone exported function, with its module-qualified name.
    Function(String),
    /// An exported impl block, with the type name and module-qualified path.
    Impl(String),
}

/// Result of scanning a crate's source files for `#[export]` markers.
#[derive(Debug, Default)]
pub struct DiscoverResult {
    /// Names of exported items (module-qualified, e.g. "my_func" or "MyType").
    pub items: HashSet<ExportedItem>,
}

/// Scan all `.rs` files in the given crate root for `#[export]` annotations.
///
/// `crate_root` should be the directory containing `Cargo.toml`. The entry
/// point is found by looking for `src/lib.rs` or `src/main.rs`.
pub fn discover_exports(crate_root: &Path) -> miette::Result<DiscoverResult> {
    let src_dir = crate_root.join("src");
    if !src_dir.is_dir() {
        miette::bail!("no src/ directory found in {}", crate_root.display());
    }

    let mut result = DiscoverResult::default();

    // Find the crate entry point.
    let entry = if src_dir.join("lib.rs").is_file() {
        src_dir.join("lib.rs")
    } else if src_dir.join("main.rs").is_file() {
        src_dir.join("main.rs")
    } else {
        miette::bail!("no lib.rs or main.rs found in {}", src_dir.display());
    };

    scan_file(&entry, &[], &mut result)?;

    Ok(result)
}

/// Scan a single `.rs` file and any inline/file-based submodules.
fn scan_file(
    path: &Path,
    module_path: &[String],
    result: &mut DiscoverResult,
) -> miette::Result<()> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| miette::miette!("failed to read {}: {e}", path.display()))?;

    let ast: File = syn::parse_file(&source)
        .map_err(|e| miette::miette!("failed to parse {}: {e}", path.display()))?;

    let mut visitor = ExportVisitor {
        module_path: module_path.to_vec(),
        result,
        file_dir: path.parent().unwrap_or(Path::new(".")).to_path_buf(),
    };
    visitor.visit_file(&ast);

    Ok(())
}

/// AST visitor that collects `#[export]`-annotated items.
struct ExportVisitor<'a> {
    module_path: Vec<String>,
    result: &'a mut DiscoverResult,
    file_dir: PathBuf,
}

impl ExportVisitor<'_> {
    /// Build a qualified name from the current module path and a local name.
    fn qualified_name(&self, name: &Ident) -> String {
        if self.module_path.is_empty() {
            name.to_string()
        } else {
            format!("{}::{}", self.module_path.join("::"), name)
        }
    }
}

impl<'ast> Visit<'ast> for ExportVisitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if has_export_attr(&node.attrs) {
            let name = self.qualified_name(&node.sig.ident);
            self.result.items.insert(ExportedItem::Function(name));
        }
        // Don't recurse into function bodies.
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        if has_export_attr(&node.attrs) {
            // Extract the type name from the impl target.
            if let Some(type_name) = extract_type_name(&node.self_ty) {
                let name = if self.module_path.is_empty() {
                    type_name
                } else {
                    format!("{}::{type_name}", self.module_path.join("::"))
                };
                self.result.items.insert(ExportedItem::Impl(name));
            }
        }
        // Don't recurse into impl bodies — we resolve methods via rustdoc.
    }

    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let mod_name = node.ident.to_string();

        if let Some((_, items)) = &node.content {
            // Inline module: recurse with extended module path.
            let mut child_path = self.module_path.clone();
            child_path.push(mod_name);
            let mut child_visitor = ExportVisitor {
                module_path: child_path,
                result: self.result,
                file_dir: self.file_dir.clone(),
            };
            for item in items {
                child_visitor.visit_item(item);
            }
        } else {
            // External module: look for the file.
            let mod_file = self.file_dir.join(format!("{mod_name}.rs"));
            let mod_dir_file = self.file_dir.join(&mod_name).join("mod.rs");

            let mut child_path = self.module_path.clone();
            child_path.push(mod_name);

            if mod_file.is_file() {
                let _ = scan_file(&mod_file, &child_path, self.result);
            } else if mod_dir_file.is_file() {
                let _ = scan_file(&mod_dir_file, &child_path, self.result);
            }
            // If neither file exists, silently skip — it might be a generated module.
        }
    }
}

/// Check whether any attribute in the list is `#[export]`.
fn has_export_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("export"))
}

/// Extract the simple type name from a `syn::Type`, if it's a plain path.
fn extract_type_name(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(type_path) = ty {
        // Take the last segment (e.g. `MyStruct` from `crate::MyStruct`).
        type_path
            .path
            .segments
            .last()
            .map(|seg| seg.ident.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_export_attr() {
        let code = r#"
            #[export]
            fn foo() {}
        "#;
        let file: File = syn::parse_file(code).unwrap();
        if let syn::Item::Fn(item_fn) = &file.items[0] {
            assert!(has_export_attr(&item_fn.attrs));
        } else {
            panic!("expected function item");
        }
    }

    #[test]
    fn test_no_export_attr() {
        let code = r#"
            #[inline]
            fn foo() {}
        "#;
        let file: File = syn::parse_file(code).unwrap();
        if let syn::Item::Fn(item_fn) = &file.items[0] {
            assert!(!has_export_attr(&item_fn.attrs));
        } else {
            panic!("expected function item");
        }
    }

    #[test]
    fn test_discover_from_source() {
        let code = r#"
            use rs_auto_wit_macros::export;

            #[export]
            pub fn greet(name: String) -> String {
                format!("Hello, {name}!")
            }

            pub struct Counter {
                count: u32,
            }

            #[export]
            impl Counter {
                pub fn new(initial: u32) -> Self {
                    Self { count: initial }
                }
            }

            fn not_exported() {}
        "#;
        let file: File = syn::parse_file(code).unwrap();
        let mut result = DiscoverResult::default();
        let mut visitor = ExportVisitor {
            module_path: vec![],
            result: &mut result,
            file_dir: PathBuf::from("."),
        };
        visitor.visit_file(&file);

        assert!(
            result
                .items
                .contains(&ExportedItem::Function("greet".into()))
        );
        assert!(result.items.contains(&ExportedItem::Impl("Counter".into())));
        assert_eq!(result.items.len(), 2);
    }
}
