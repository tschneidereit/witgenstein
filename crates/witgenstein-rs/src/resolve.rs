// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Resolve exported items via rustdoc JSON.
//!
//! This module invokes `cargo rustdoc` (with `RUSTC_BOOTSTRAP=1`) to generate
//! JSON output, then parses it with `rustdoc-types` to build fully resolved
//! type information for all items discovered by the `discover` module.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

use rustdoc_types::{
    Crate, Enum, FunctionSignature, GenericArg, GenericArgs, Id, Impl, Item, ItemEnum, Struct,
    StructKind, Type, Variant, VariantKind,
};

use crate::discover::ExportedItem;

/// A fully resolved function signature.
#[derive(Debug, Clone)]
pub struct ResolvedFunction {
    pub name: String,
    pub params: Vec<(String, ResolvedType)>,
    pub return_type: Option<ResolvedType>,
    pub is_async: bool,
    /// Whether this method takes `&mut self` (vs `&self` or no self).
    pub is_mut_self: bool,
}

/// A fully resolved impl block (resource).
#[derive(Debug, Clone)]
pub struct ResolvedImpl {
    pub type_name: String,
    pub constructor: Option<ResolvedFunction>,
    pub methods: Vec<ResolvedFunction>,
    pub static_funcs: Vec<ResolvedFunction>,
}

/// A fully resolved type definition discovered transitively.
#[derive(Debug, Clone)]
pub enum ResolvedTypeDef {
    Record(ResolvedRecord),
    Enum(ResolvedEnum),
    Variant(ResolvedVariant),
}

/// A record (struct with named fields).
#[derive(Debug, Clone)]
pub struct ResolvedRecord {
    pub name: String,
    pub fields: Vec<(String, ResolvedType)>,
}

/// An enumeration of unit variants.
#[derive(Debug, Clone)]
pub struct ResolvedEnum {
    pub name: String,
    pub cases: Vec<String>,
}

/// A variant (enum with data-carrying cases).
#[derive(Debug, Clone)]
pub struct ResolvedVariant {
    pub name: String,
    pub cases: Vec<(String, Option<ResolvedType>)>,
}

/// A resolved type reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedType {
    // Primitives
    Bool,
    U8,
    U16,
    U32,
    U64,
    S8,
    S16,
    S32,
    S64,
    F32,
    F64,
    Char,
    WitString,
    // Compound
    List(Box<ResolvedType>),
    Option(Box<ResolvedType>),
    Result {
        ok: Option<Box<ResolvedType>>,
        err: Option<Box<ResolvedType>>,
    },
    Tuple(Vec<ResolvedType>),
    // Reference to a named user-defined type
    Named(String),
    // Handle types for resources
    Own(String),
    Borrow(String),
}

/// The complete result of resolving all exports.
#[derive(Debug)]
pub struct ResolveResult {
    pub functions: Vec<ResolvedFunction>,
    pub impls: Vec<ResolvedImpl>,
    pub type_defs: Vec<ResolvedTypeDef>,
    pub warnings: Vec<String>,
}

impl ResolveResult {
    /// Whether there are any exports (type defs, impls, or functions).
    pub fn has_exports(&self) -> bool {
        !self.type_defs.is_empty() || !self.impls.is_empty() || !self.functions.is_empty()
    }
}

/// Invoke `cargo rustdoc` and resolve all exported items.
pub fn resolve_exports(
    crate_root: &Path,
    exports: &HashSet<ExportedItem>,
) -> miette::Result<ResolveResult> {
    let krate = run_rustdoc(crate_root)?;
    let mut resolver: Resolver<'_> = Resolver::new(&krate);
    resolver.resolve(exports);
    Ok(resolver.into_result())
}

/// Run `cargo rustdoc` with JSON output and parse the result.
///
/// Uses `RUSTC_BOOTSTRAP=1` so that `--output-format json` and
/// `-Z unstable-options` work on stable and beta toolchains, following the
/// approach used by `cargo-semver-checks`.
fn run_rustdoc(crate_root: &Path) -> miette::Result<Crate> {
    let output = Command::new("cargo")
        .args([
            "rustdoc",
            "--manifest-path",
            &crate_root.join("Cargo.toml").to_string_lossy(),
            "--output-format",
            "json",
            "-Z",
            "unstable-options",
            "--",
            "--document-private-items",
        ])
        .env("RUSTC_BOOTSTRAP", "1")
        .output()
        .map_err(|e| miette::miette!("failed to run cargo rustdoc: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        miette::bail!("cargo rustdoc failed:\n{stderr}");
    }

    let json_path = {
        let prefix = "   Generated ";
        let out = str::from_utf8(&output.stderr).expect("rustdoc output is not valid UTF-8");
        let start = out.find(prefix).ok_or_else(|| {
            miette::miette!("failed to find generated JSON path in rustdoc output")
        })? + prefix.len();
        let end = out[start..].find('\n').ok_or_else(|| {
            miette::miette!("failed to find end of generated JSON path in rustdoc output")
        })? + start;
        &out[start..end]
    };

    let json_str = std::fs::read_to_string(json_path)
        .map_err(|e| miette::miette!("failed to read rustdoc JSON at {json_path}: {e}",))?;

    let krate: Crate = serde_json::from_str(&json_str)
        .map_err(|e| miette::miette!("failed to parse rustdoc JSON: {e}"))?;

    Ok(krate)
}

/// Resolves exported items by looking them up in the rustdoc JSON index.
struct Resolver<'a> {
    krate: &'a Crate,
    visited_types: HashSet<String>,
    type_defs: Vec<ResolvedTypeDef>,
    functions: Vec<ResolvedFunction>,
    impls: Vec<ResolvedImpl>,
    resource_types: HashSet<String>,
    warnings: Vec<String>,
    /// The type name for `Self` when resolving inside an `impl` block.
    self_type: Option<String>,
}

impl<'a> Resolver<'a> {
    fn new(krate: &'a Crate) -> Self {
        Self {
            krate,
            visited_types: HashSet::new(),
            type_defs: Vec::new(),
            functions: Vec::new(),
            impls: Vec::new(),
            resource_types: HashSet::new(),
            warnings: Vec::new(),
            self_type: None,
        }
    }

    fn into_result(self) -> ResolveResult {
        ResolveResult {
            functions: self.functions,
            impls: self.impls,
            type_defs: self.type_defs,
            warnings: self.warnings,
        }
    }

    fn resolve(&mut self, exports: &HashSet<ExportedItem>) {
        // First pass: identify resource types from impl exports.
        for export in exports {
            if let ExportedItem::Impl(name) = export {
                let simple_name = name.rsplit("::").next().unwrap_or(name);
                self.resource_types.insert(simple_name.to_string());
            }
        }

        // Build a lookup from item name to item for faster matching.
        let mut items_by_name: HashMap<&str, Vec<(&Id, &Item)>> = HashMap::new();
        for (id, item) in &self.krate.index {
            if let Some(name) = &item.name {
                items_by_name
                    .entry(name.as_str())
                    .or_default()
                    .push((id, item));
            }
        }

        for export in exports {
            match export {
                ExportedItem::Function(name) => {
                    let simple_name = name.rsplit("::").next().unwrap_or(name);
                    if let Some(candidates) = items_by_name.get(simple_name) {
                        let mut found = false;
                        for &(_, item) in candidates {
                            if let ItemEnum::Function(func) = &item.inner {
                                // Skip methods (they have a `self` parameter) —
                                // we only want standalone functions here.
                                let has_self = func.sig.inputs.iter().any(|(n, _)| n == "self");
                                if has_self {
                                    continue;
                                }
                                let resolved = self.resolve_function(
                                    simple_name,
                                    &func.sig,
                                    func.header.is_async,
                                );
                                self.functions.push(resolved);
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            self.warnings.push(format!(
                                "exported function `{name}` not found in rustdoc output"
                            ));
                        }
                    } else {
                        self.warnings.push(format!(
                            "exported function `{name}` not found in rustdoc output"
                        ));
                    }
                }
                ExportedItem::Impl(name) => {
                    let simple_name = name.rsplit("::").next().unwrap_or(name);
                    self.resolve_impl_block(simple_name);
                }
            }
        }
    }

    fn resolve_function(
        &mut self,
        name: &str,
        decl: &FunctionSignature,
        is_async: bool,
    ) -> ResolvedFunction {
        let is_mut_self = decl.inputs.iter().any(|(param_name, ty)| {
            param_name == "self"
                && matches!(
                    ty,
                    Type::BorrowedRef {
                        is_mutable: true,
                        ..
                    }
                )
        });

        let params = decl
            .inputs
            .iter()
            .filter_map(|(param_name, ty): &(String, Type)| {
                if param_name == "self" {
                    return None;
                }
                let resolved = self.resolve_type(ty);
                Some((param_name.clone(), resolved))
            })
            .collect();

        let return_type = decl.output.as_ref().map(|ty| self.resolve_type(ty));

        ResolvedFunction {
            name: name.to_string(),
            params,
            return_type,
            is_async,
            is_mut_self,
        }
    }

    fn resolve_impl_block(&mut self, type_name: &str) {
        let mut constructor = None;
        let mut methods = Vec::new();
        let mut static_funcs = Vec::new();

        let prev_self_type = self.self_type.replace(type_name.to_string());

        for item in self.krate.index.values() {
            if let ItemEnum::Impl(imp) = &item.inner {
                if imp.trait_.is_some() {
                    continue;
                }
                if !impl_matches_type(imp, type_name) {
                    continue;
                }

                for method_id in &imp.items {
                    if let Some(method_item) = self.krate.index.get(method_id)
                        && let ItemEnum::Function(func) = &method_item.inner
                    {
                        let method_name = method_item.name.as_deref().unwrap_or("unknown");

                        let resolved =
                            self.resolve_function(method_name, &func.sig, func.header.is_async);

                        let has_self = func.sig.inputs.iter().any(|(name, _)| name == "self");

                        if is_constructor(method_name, &func.sig) {
                            constructor = Some(resolved);
                        } else if has_self {
                            methods.push(resolved);
                        } else {
                            static_funcs.push(resolved);
                        }
                    }
                }
            }
        }

        self.self_type = prev_self_type;

        self.impls.push(ResolvedImpl {
            type_name: type_name.to_string(),
            constructor,
            methods,
            static_funcs,
        });
    }

    fn resolve_type(&mut self, ty: &Type) -> ResolvedType {
        match ty {
            Type::Primitive(name) => match name.as_str() {
                "bool" => ResolvedType::Bool,
                "u8" => ResolvedType::U8,
                "u16" => ResolvedType::U16,
                "u32" => ResolvedType::U32,
                "u64" => ResolvedType::U64,
                "i8" => ResolvedType::S8,
                "i16" => ResolvedType::S16,
                "i32" => ResolvedType::S32,
                "i64" => ResolvedType::S64,
                "f32" => ResolvedType::F32,
                "f64" => ResolvedType::F64,
                "char" => ResolvedType::Char,
                "str" => ResolvedType::WitString,
                _ => {
                    self.warnings
                        .push(format!("unsupported primitive type `{name}`"));
                    ResolvedType::WitString
                }
            },
            Type::ResolvedPath(path) => {
                let type_path = &path.path;
                let simple_name = type_path.rsplit("::").next().unwrap_or(type_path);
                let generic_args = path.args.as_deref();

                match simple_name {
                    "String" => ResolvedType::WitString,
                    "Vec" => {
                        let inner = self.extract_first_generic_arg(generic_args);
                        ResolvedType::List(Box::new(inner))
                    }
                    "Option" => {
                        let inner = self.extract_first_generic_arg(generic_args);
                        ResolvedType::Option(Box::new(inner))
                    }
                    "Result" => {
                        let (ok, err) = self.extract_result_args(generic_args);
                        ResolvedType::Result { ok, err }
                    }
                    "Box" | "Arc" | "Rc" => self.extract_first_generic_arg(generic_args),
                    "HashMap" | "BTreeMap" => {
                        let args = self.extract_two_generic_args(generic_args);
                        ResolvedType::List(Box::new(ResolvedType::Tuple(args)))
                    }
                    "HashSet" | "BTreeSet" => {
                        let inner = self.extract_first_generic_arg(generic_args);
                        ResolvedType::List(Box::new(inner))
                    }
                    _ => {
                        self.discover_type(&path.id, simple_name);

                        if self.resource_types.contains(simple_name) {
                            ResolvedType::Own(simple_name.to_string())
                        } else {
                            ResolvedType::Named(simple_name.to_string())
                        }
                    }
                }
            }
            Type::BorrowedRef { type_, .. } => match type_.as_ref() {
                Type::Primitive(name) if name == "str" => ResolvedType::WitString,
                Type::Slice(inner) => {
                    let resolved = self.resolve_type(inner);
                    ResolvedType::List(Box::new(resolved))
                }
                Type::ResolvedPath(path) => {
                    let simple_name = path.path.rsplit("::").next().unwrap_or(&path.path);
                    if self.resource_types.contains(simple_name) {
                        ResolvedType::Borrow(simple_name.to_string())
                    } else {
                        self.resolve_type(type_)
                    }
                }
                _ => self.resolve_type(type_),
            },
            Type::Slice(inner) => {
                let resolved = self.resolve_type(inner);
                ResolvedType::List(Box::new(resolved))
            }
            Type::Tuple(elems) => {
                if elems.is_empty() {
                    ResolvedType::Tuple(vec![])
                } else {
                    let resolved: Vec<_> = elems.iter().map(|t| self.resolve_type(t)).collect();
                    ResolvedType::Tuple(resolved)
                }
            }
            Type::RawPointer { .. } => {
                self.warnings
                    .push("raw pointers are not supported in WIT".into());
                ResolvedType::U32
            }
            Type::Generic(name) if name == "Self" => {
                if let Some(self_type) = &self.self_type {
                    let name = self_type.clone();
                    if self.resource_types.contains(&name) {
                        ResolvedType::Own(name)
                    } else {
                        ResolvedType::Named(name)
                    }
                } else {
                    self.warnings
                        .push("Self type used outside of impl block".into());
                    ResolvedType::WitString
                }
            }
            Type::Array { .. }
            | Type::QualifiedPath { .. }
            | Type::Pat { .. }
            | Type::DynTrait(_)
            | Type::Generic(_)
            | Type::FunctionPointer(_)
            | Type::ImplTrait(_)
            | Type::Infer => {
                self.warnings.push(format!("unsupported type: {ty:?}"));
                ResolvedType::WitString
            }
        }
    }

    fn discover_type(&mut self, id: &Id, type_name: &str) {
        let id_str = format!("{id:?}");
        if self.visited_types.contains(&id_str) {
            return;
        }
        self.visited_types.insert(id_str);

        let item_inner = self.krate.index.get(id).map(|item| item.inner.clone());
        let Some(inner) = item_inner else {
            return;
        };

        match &inner {
            ItemEnum::Struct(s) => self.resolve_struct(type_name, s),
            ItemEnum::Enum(e) => self.resolve_enum(type_name, e),
            ItemEnum::TypeAlias(alias) => {
                self.resolve_type(&alias.type_);
            }
            _ => {}
        }
    }

    fn resolve_struct(&mut self, name: &str, s: &Struct) {
        match &s.kind {
            StructKind::Plain {
                fields,
                has_stripped_fields: _,
            } => {
                let mut record_fields = Vec::new();
                for field_id in fields {
                    if let Some(field_item) = self.krate.index.get(field_id)
                        && let ItemEnum::StructField(field_type) = &field_item.inner
                    {
                        let field_name = field_item.name.as_deref().unwrap_or("unknown");
                        let resolved = self.resolve_type(field_type);
                        record_fields.push((field_name.to_string(), resolved));
                    }
                }
                self.type_defs.push(ResolvedTypeDef::Record(ResolvedRecord {
                    name: name.to_string(),
                    fields: record_fields,
                }));
            }
            StructKind::Tuple(fields) => {
                let record_fields: Vec<_> = fields
                    .iter()
                    .enumerate()
                    .filter_map(|(i, field_id)| {
                        field_id.as_ref().and_then(|id| {
                            self.krate.index.get(id).and_then(|item| {
                                if let ItemEnum::StructField(field_type) = &item.inner {
                                    Some((format!("f{i}"), self.resolve_type(field_type)))
                                } else {
                                    None
                                }
                            })
                        })
                    })
                    .collect();
                self.type_defs.push(ResolvedTypeDef::Record(ResolvedRecord {
                    name: name.to_string(),
                    fields: record_fields,
                }));
            }
            StructKind::Unit => {
                self.type_defs.push(ResolvedTypeDef::Record(ResolvedRecord {
                    name: name.to_string(),
                    fields: vec![],
                }));
            }
        }
    }

    fn resolve_enum(&mut self, name: &str, e: &Enum) {
        let all_unit = e.variants.iter().all(|variant_id| {
            self.krate
                .index
                .get(variant_id)
                .and_then(|item| {
                    if let ItemEnum::Variant(v) = &item.inner {
                        Some(matches!(v.kind, VariantKind::Plain))
                    } else {
                        None
                    }
                })
                .unwrap_or(false)
        });

        if all_unit {
            let cases: Vec<String> = e
                .variants
                .iter()
                .filter_map(|variant_id| {
                    self.krate
                        .index
                        .get(variant_id)
                        .and_then(|item| item.name.clone())
                })
                .collect();
            self.type_defs.push(ResolvedTypeDef::Enum(ResolvedEnum {
                name: name.to_string(),
                cases,
            }));
        } else {
            let cases: Vec<(String, Option<ResolvedType>)> = e
                .variants
                .iter()
                .filter_map(|variant_id| {
                    let item = self.krate.index.get(variant_id)?;
                    let variant_name = item.name.clone()?;
                    if let ItemEnum::Variant(v) = &item.inner {
                        let payload = self.resolve_variant_payload(v);
                        Some((variant_name, payload))
                    } else {
                        None
                    }
                })
                .collect();
            self.type_defs
                .push(ResolvedTypeDef::Variant(ResolvedVariant {
                    name: name.to_string(),
                    cases,
                }));
        }
    }

    fn resolve_variant_payload(&mut self, variant: &Variant) -> Option<ResolvedType> {
        match &variant.kind {
            VariantKind::Plain => None,
            VariantKind::Tuple(fields) => {
                let types: Vec<_> = fields
                    .iter()
                    .filter_map(|field_id| {
                        field_id.as_ref().and_then(|id| {
                            self.krate.index.get(id).and_then(|item| {
                                if let ItemEnum::StructField(ty) = &item.inner {
                                    Some(self.resolve_type(ty))
                                } else {
                                    None
                                }
                            })
                        })
                    })
                    .collect();
                match types.len() {
                    0 => None,
                    1 => Some(types.into_iter().next().unwrap()),
                    _ => Some(ResolvedType::Tuple(types)),
                }
            }
            VariantKind::Struct {
                fields,
                has_stripped_fields: _,
            } => {
                let types: Vec<_> = fields
                    .iter()
                    .filter_map(|field_id| {
                        let item = self.krate.index.get(field_id)?;
                        if let ItemEnum::StructField(ty) = &item.inner {
                            Some(self.resolve_type(ty))
                        } else {
                            None
                        }
                    })
                    .collect();
                match types.len() {
                    0 => None,
                    1 => Some(types.into_iter().next().unwrap()),
                    _ => Some(ResolvedType::Tuple(types)),
                }
            }
        }
    }

    fn extract_first_generic_arg(&mut self, args: Option<&GenericArgs>) -> ResolvedType {
        if let Some(GenericArgs::AngleBracketed { args, .. }) = args {
            for arg in args {
                if let GenericArg::Type(ty) = arg {
                    return self.resolve_type(ty);
                }
            }
        }
        self.warnings.push("missing generic type argument".into());
        ResolvedType::WitString
    }

    fn extract_two_generic_args(&mut self, args: Option<&GenericArgs>) -> Vec<ResolvedType> {
        let mut result = Vec::new();
        if let Some(GenericArgs::AngleBracketed { args, .. }) = args {
            for arg in args {
                if let GenericArg::Type(ty) = arg {
                    result.push(self.resolve_type(ty));
                }
            }
        }
        result
    }

    fn extract_result_args(
        &mut self,
        args: Option<&GenericArgs>,
    ) -> (Option<Box<ResolvedType>>, Option<Box<ResolvedType>>) {
        let types = self.extract_two_generic_args(args);
        let ok = types.first().cloned().map(Box::new);
        let err = types.get(1).cloned().map(Box::new);
        (ok, err)
    }
}

fn impl_matches_type(imp: &Impl, type_name: &str) -> bool {
    match &imp.for_ {
        Type::ResolvedPath(path) => {
            let simple_name = path.path.rsplit("::").next().unwrap_or(&path.path);
            simple_name == type_name
        }
        _ => false,
    }
}

fn is_constructor(name: &str, decl: &FunctionSignature) -> bool {
    name == "new" && decl.output.is_some()
}
