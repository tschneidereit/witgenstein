// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Extract type information from `syn` AST items.
//!
//! This module walks a `Vec<Item>` and builds a model of exported functions,
//! impl blocks, and the type definitions they reference.  It can also scan the
//! crate's source files at macro-expansion time so that types defined outside
//! `component!` are automatically discovered.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use syn::{
    Attribute, FnArg, GenericArgument, ImplItem, Item, ItemEnum, ItemFn, ItemImpl, ItemStruct, Pat,
    PathArguments, ReturnType, Type,
};

// ---------------------------------------------------------------------------
// Model types — similar to resolve.rs but built from syn AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedType {
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
    List(Box<ResolvedType>),
    Option(Box<ResolvedType>),
    Result {
        ok: Option<Box<ResolvedType>>,
        err: Option<Box<ResolvedType>>,
    },
    Tuple(Vec<ResolvedType>),
    Future(Option<Box<ResolvedType>>),
    Stream(Option<Box<ResolvedType>>),
    Named(String),
    Own(String),
    Borrow(String),
}

#[derive(Debug, Clone)]
pub struct ExportedFunction {
    pub name: String,
    pub params: Vec<(String, ResolvedType)>,
    pub return_type: Option<ResolvedType>,
    pub is_async: bool,
    pub is_mut_self: bool,
}

#[derive(Debug, Clone)]
pub struct ExportedImpl {
    pub type_name: String,
    pub constructor: Option<ExportedFunction>,
    pub methods: Vec<ExportedFunction>,
    pub static_funcs: Vec<ExportedFunction>,
}

#[derive(Debug, Clone)]
pub enum ResolvedTypeDef {
    Record(ResolvedRecord),
    Enum(ResolvedEnum),
    Variant(ResolvedVariant),
}

#[derive(Debug, Clone)]
pub struct ResolvedRecord {
    pub name: String,
    pub fields: Vec<(String, ResolvedType)>,
}

#[derive(Debug, Clone)]
pub struct ResolvedEnum {
    pub name: String,
    pub cases: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedVariant {
    pub name: String,
    pub cases: Vec<(String, Option<ResolvedType>)>,
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Discovers exports and referenced types from a set of parsed `syn::Item`s.
pub struct TypeDiscovery {
    pub functions: Vec<ExportedFunction>,
    pub impls: Vec<ExportedImpl>,
    pub type_defs: Vec<ResolvedTypeDef>,

    /// Struct/enum definitions available in the module, keyed by name.
    structs: HashMap<String, ItemStruct>,
    enums: HashMap<String, ItemEnum>,

    /// Types for which we've already emitted a typedef.
    visited_types: HashSet<String>,

    /// Types that are impl'd as resources.
    resource_types: HashSet<String>,
}

impl TypeDiscovery {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            impls: Vec::new(),
            type_defs: Vec::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            visited_types: HashSet::new(),
            resource_types: HashSet::new(),
        }
    }

    /// Scan the current crate's source tree and pre-populate struct/enum
    /// definitions so that types defined *outside* `component!` are available
    /// for WIT generation.  Uses `CARGO_MANIFEST_DIR` to locate source files.
    ///
    /// Call this **before** [`process_items`](Self::process_items) so that
    /// definitions inside the macro take precedence over external ones.
    pub fn scan_crate_sources(&mut self) {
        let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
            Ok(dir) => dir,
            Err(_) => return,
        };
        let src_dir = PathBuf::from(&manifest_dir).join("src");
        if src_dir.is_dir() {
            self.scan_directory(&src_dir);
        }
    }

    fn scan_directory(&mut self, dir: &Path) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.scan_directory(&path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                self.scan_source_file(&path);
            }
        }
    }

    fn scan_source_file(&mut self, path: &Path) {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return,
        };
        let file = match syn::parse_file(&source) {
            Ok(f) => f,
            Err(_) => return,
        };
        self.collect_external_types(&file.items);
    }

    fn collect_external_types(&mut self, items: &[Item]) {
        for item in items {
            match item {
                Item::Struct(s) => {
                    let name = s.ident.to_string();
                    self.structs.entry(name).or_insert_with(|| s.clone());
                }
                Item::Enum(e) => {
                    let name = e.ident.to_string();
                    self.enums.entry(name).or_insert_with(|| e.clone());
                }
                Item::Mod(m) => {
                    if let Some((_, inner_items)) = &m.content {
                        self.collect_external_types(inner_items);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn process_items(&mut self, items: &[Item]) {
        // First pass: collect all struct / enum definitions and resource types.
        for item in items {
            match item {
                Item::Struct(s) => {
                    self.structs.insert(s.ident.to_string(), s.clone());
                }
                Item::Enum(e) => {
                    self.enums.insert(e.ident.to_string(), e.clone());
                }
                Item::Impl(imp) if has_export_attr(&imp.attrs) => {
                    if let Some(name) = impl_type_name(imp) {
                        self.resource_types.insert(name);
                    }
                }
                _ => {}
            }
        }

        // Second pass: extract exported functions and impl blocks.
        for item in items {
            match item {
                Item::Fn(f) if has_export_attr(&f.attrs) => {
                    let func = self.extract_function(f);
                    self.functions.push(func);
                }
                Item::Impl(imp) if has_export_attr(&imp.attrs) => {
                    let extracted = self.extract_impl(imp);
                    self.impls.push(extracted);
                }
                _ => {}
            }
        }
    }

    fn extract_function(&mut self, f: &ItemFn) -> ExportedFunction {
        let name = f.sig.ident.to_string();
        let is_async = f.sig.asyncness.is_some();
        let params = self.extract_params(&f.sig.inputs);
        let return_type = self.extract_return_type(&f.sig.output);

        ExportedFunction {
            name,
            params,
            return_type,
            is_async,
            is_mut_self: false,
        }
    }

    fn extract_impl(&mut self, imp: &ItemImpl) -> ExportedImpl {
        let type_name = impl_type_name(imp).unwrap_or_else(|| "Unknown".into());
        let mut constructor = None;
        let mut methods = Vec::new();
        let mut static_funcs = Vec::new();

        for item in &imp.items {
            if let ImplItem::Fn(method) = item {
                let name = method.sig.ident.to_string();
                let is_async = method.sig.asyncness.is_some();
                let params = self.extract_params(&method.sig.inputs);
                let return_type = self.extract_return_type(&method.sig.output).map(|rt| {
                    // Replace `Self` → actual type name in return types.
                    replace_self_type(rt, &type_name)
                });

                let has_self = method
                    .sig
                    .inputs
                    .iter()
                    .any(|arg| matches!(arg, FnArg::Receiver(_)));

                let is_mut_self = method.sig.inputs.iter().any(|arg| {
                    if let FnArg::Receiver(r) = arg {
                        r.mutability.is_some()
                    } else {
                        false
                    }
                });

                let func = ExportedFunction {
                    name: name.clone(),
                    params,
                    return_type,
                    is_async,
                    is_mut_self,
                };

                if is_constructor(&name, &method.sig, &type_name) {
                    constructor = Some(func);
                } else if has_self {
                    methods.push(func);
                } else {
                    static_funcs.push(func);
                }
            }
        }

        ExportedImpl {
            type_name,
            constructor,
            methods,
            static_funcs,
        }
    }

    fn extract_params(
        &mut self,
        inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
    ) -> Vec<(String, ResolvedType)> {
        inputs
            .iter()
            .filter_map(|arg| {
                if let FnArg::Typed(pat_type) = arg {
                    let name = match pat_type.pat.as_ref() {
                        Pat::Ident(ident) => ident.ident.to_string(),
                        _ => return None,
                    };
                    let ty = self.resolve_type(&pat_type.ty);
                    Some((name, ty))
                } else {
                    None // skip self
                }
            })
            .collect()
    }

    fn extract_return_type(&mut self, output: &ReturnType) -> Option<ResolvedType> {
        match output {
            ReturnType::Default => None,
            ReturnType::Type(_, ty) => Some(self.resolve_type(ty)),
        }
    }

    pub fn resolve_type(&mut self, ty: &Type) -> ResolvedType {
        match ty {
            Type::Path(type_path) => {
                let path = &type_path.path;
                let last = path.segments.last().unwrap();
                let name = last.ident.to_string();

                match name.as_str() {
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
                    "String" => ResolvedType::WitString,
                    "str" => ResolvedType::WitString,
                    "Vec" => {
                        let inner = self.extract_first_generic(&last.arguments);
                        ResolvedType::List(Box::new(inner))
                    }
                    "Option" => {
                        let inner = self.extract_first_generic(&last.arguments);
                        ResolvedType::Option(Box::new(inner))
                    }
                    "Result" => {
                        let (ok, err) = self.extract_result_generics(&last.arguments);
                        ResolvedType::Result { ok, err }
                    }
                    "Box" | "Arc" | "Rc" => self.extract_first_generic(&last.arguments),
                    "HashMap" | "BTreeMap" => {
                        let args = self.extract_two_generics(&last.arguments);
                        ResolvedType::List(Box::new(ResolvedType::Tuple(args)))
                    }
                    "HashSet" | "BTreeSet" => {
                        let inner = self.extract_first_generic(&last.arguments);
                        ResolvedType::List(Box::new(inner))
                    }
                    "Stream" => {
                        let inner = self.extract_optional_first_generic(&last.arguments);
                        ResolvedType::Stream(inner.map(Box::new))
                    }
                    "Future" => {
                        let inner = self.extract_optional_first_generic(&last.arguments);
                        ResolvedType::Future(inner.map(Box::new))
                    }
                    "Self" => {
                        // Will be handled by the caller knowing the impl context.
                        ResolvedType::Named("Self".into())
                    }
                    _ => {
                        // Named type — discover its definition.
                        self.discover_type(&name);
                        if self.resource_types.contains(&name) {
                            ResolvedType::Own(name)
                        } else {
                            ResolvedType::Named(name)
                        }
                    }
                }
            }
            Type::Reference(r) => {
                if let Type::Path(tp) = r.elem.as_ref() {
                    let last = tp.path.segments.last().unwrap();
                    let name = last.ident.to_string();
                    if name == "str" {
                        return ResolvedType::WitString;
                    }
                    if self.resource_types.contains(&name) {
                        return ResolvedType::Borrow(name);
                    }
                }
                // Fall through: resolve the inner type.
                self.resolve_type(&r.elem)
            }
            Type::Slice(s) => {
                let inner = self.resolve_type(&s.elem);
                ResolvedType::List(Box::new(inner))
            }
            Type::Tuple(t) => {
                if t.elems.is_empty() {
                    ResolvedType::Tuple(vec![])
                } else {
                    let resolved: Vec<_> = t.elems.iter().map(|e| self.resolve_type(e)).collect();
                    ResolvedType::Tuple(resolved)
                }
            }
            _ => ResolvedType::WitString, // fallback
        }
    }

    fn discover_type(&mut self, name: &str) {
        if self.visited_types.contains(name) || self.resource_types.contains(name) {
            return;
        }
        self.visited_types.insert(name.to_string());

        if let Some(s) = self.structs.get(name).cloned() {
            self.resolve_struct(&s);
        } else if let Some(e) = self.enums.get(name).cloned() {
            self.resolve_enum(&e);
        }
    }

    fn resolve_struct(&mut self, s: &ItemStruct) {
        let name = s.ident.to_string();
        match &s.fields {
            syn::Fields::Named(fields) => {
                let record_fields: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| {
                        let field_name = f.ident.as_ref().unwrap().to_string();
                        let field_type = self.resolve_type(&f.ty);
                        (field_name, field_type)
                    })
                    .collect();
                self.type_defs.push(ResolvedTypeDef::Record(ResolvedRecord {
                    name,
                    fields: record_fields,
                }));
            }
            syn::Fields::Unnamed(fields) => {
                let record_fields: Vec<_> = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let field_type = self.resolve_type(&f.ty);
                        (format!("f{i}"), field_type)
                    })
                    .collect();
                self.type_defs.push(ResolvedTypeDef::Record(ResolvedRecord {
                    name,
                    fields: record_fields,
                }));
            }
            syn::Fields::Unit => {
                self.type_defs.push(ResolvedTypeDef::Record(ResolvedRecord {
                    name,
                    fields: vec![],
                }));
            }
        }
    }

    fn resolve_enum(&mut self, e: &ItemEnum) {
        let name = e.ident.to_string();

        let all_unit = e
            .variants
            .iter()
            .all(|v| matches!(v.fields, syn::Fields::Unit));

        if all_unit {
            let cases: Vec<String> = e.variants.iter().map(|v| v.ident.to_string()).collect();
            self.type_defs
                .push(ResolvedTypeDef::Enum(ResolvedEnum { name, cases }));
        } else {
            let cases: Vec<(String, Option<ResolvedType>)> = e
                .variants
                .iter()
                .map(|v| {
                    let variant_name = v.ident.to_string();
                    let payload = match &v.fields {
                        syn::Fields::Unit => None,
                        syn::Fields::Unnamed(fields) => {
                            let types: Vec<_> = fields
                                .unnamed
                                .iter()
                                .map(|f| self.resolve_type(&f.ty))
                                .collect();
                            match types.len() {
                                0 => None,
                                1 => Some(types.into_iter().next().unwrap()),
                                _ => Some(ResolvedType::Tuple(types)),
                            }
                        }
                        syn::Fields::Named(fields) => {
                            // Named fields in a variant — treat as a single record-like tuple.
                            let types: Vec<_> = fields
                                .named
                                .iter()
                                .map(|f| self.resolve_type(&f.ty))
                                .collect();
                            match types.len() {
                                0 => None,
                                1 => Some(types.into_iter().next().unwrap()),
                                _ => Some(ResolvedType::Tuple(types)),
                            }
                        }
                    };
                    (variant_name, payload)
                })
                .collect();
            self.type_defs
                .push(ResolvedTypeDef::Variant(ResolvedVariant { name, cases }));
        }
    }

    fn extract_first_generic(&mut self, args: &PathArguments) -> ResolvedType {
        self.extract_optional_first_generic(args)
            .unwrap_or(ResolvedType::WitString)
    }

    fn extract_optional_first_generic(&mut self, args: &PathArguments) -> Option<ResolvedType> {
        if let PathArguments::AngleBracketed(ab) = args
            && let Some(GenericArgument::Type(ty)) = ab.args.first()
        {
            return Some(self.resolve_type(ty));
        }
        None
    }

    fn extract_result_generics(
        &mut self,
        args: &PathArguments,
    ) -> (Option<Box<ResolvedType>>, Option<Box<ResolvedType>>) {
        if let PathArguments::AngleBracketed(ab) = args {
            let mut iter = ab.args.iter();
            let ok = iter.next().and_then(|a| {
                if let GenericArgument::Type(ty) = a {
                    let resolved = self.resolve_type(ty);
                    // Map `()` → None (WIT result with no ok payload).
                    if matches!(&resolved, ResolvedType::Tuple(elems) if elems.is_empty()) {
                        None
                    } else {
                        Some(Box::new(resolved))
                    }
                } else {
                    None
                }
            });
            let err = iter.next().and_then(|a| {
                if let GenericArgument::Type(ty) = a {
                    let resolved = self.resolve_type(ty);
                    if matches!(&resolved, ResolvedType::Tuple(elems) if elems.is_empty()) {
                        None
                    } else {
                        Some(Box::new(resolved))
                    }
                } else {
                    None
                }
            });
            (ok, err)
        } else {
            (None, None)
        }
    }

    fn extract_two_generics(&mut self, args: &PathArguments) -> Vec<ResolvedType> {
        if let PathArguments::AngleBracketed(ab) = args {
            ab.args
                .iter()
                .filter_map(|a| {
                    if let GenericArgument::Type(ty) = a {
                        Some(self.resolve_type(ty))
                    } else {
                        None
                    }
                })
                .take(2)
                .collect()
        } else {
            vec![]
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn has_export_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("export"))
}

fn impl_type_name(imp: &ItemImpl) -> Option<String> {
    if let Type::Path(tp) = imp.self_ty.as_ref() {
        tp.path.segments.last().map(|s| s.ident.to_string())
    } else {
        None
    }
}

fn is_constructor(name: &str, sig: &syn::Signature, type_name: &str) -> bool {
    if name != "new" {
        return false;
    }
    // Must not have a self parameter.
    let has_self = sig
        .inputs
        .iter()
        .any(|arg| matches!(arg, FnArg::Receiver(_)));
    if has_self {
        return false;
    }
    // Return type should be Self or the type itself.
    match &sig.output {
        ReturnType::Type(_, ty) => match ty.as_ref() {
            Type::Path(tp) => {
                let last = tp.path.segments.last().map(|s| s.ident.to_string());
                last.as_deref() == Some("Self") || last.as_deref() == Some(type_name)
            }
            _ => false,
        },
        ReturnType::Default => false,
    }
}

/// Replace `Named("Self")` with `Own(type_name)` throughout a resolved type.
fn replace_self_type(ty: ResolvedType, type_name: &str) -> ResolvedType {
    match ty {
        ResolvedType::Named(ref n) if n == "Self" => ResolvedType::Own(type_name.into()),
        ResolvedType::List(inner) => {
            ResolvedType::List(Box::new(replace_self_type(*inner, type_name)))
        }
        ResolvedType::Option(inner) => {
            ResolvedType::Option(Box::new(replace_self_type(*inner, type_name)))
        }
        ResolvedType::Result { ok, err } => ResolvedType::Result {
            ok: ok.map(|t| Box::new(replace_self_type(*t, type_name))),
            err: err.map(|t| Box::new(replace_self_type(*t, type_name))),
        },
        ResolvedType::Tuple(elems) => ResolvedType::Tuple(
            elems
                .into_iter()
                .map(|e| replace_self_type(e, type_name))
                .collect(),
        ),
        ResolvedType::Future(inner) => {
            ResolvedType::Future(inner.map(|t| Box::new(replace_self_type(*t, type_name))))
        }
        ResolvedType::Stream(inner) => {
            ResolvedType::Stream(inner.map(|t| Box::new(replace_self_type(*t, type_name))))
        }
        other => other,
    }
}
