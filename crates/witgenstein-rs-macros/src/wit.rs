// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Generate a WIT string from extracted type information.
//!
//! Produces inline WIT content suitable for `wit_bindgen::generate!({ inline: ... })`.

use std::fmt::Write;

use crate::extract::{
    ExportedFunction, ExportedImpl, ResolvedEnum, ResolvedRecord, ResolvedType, ResolvedTypeDef,
    ResolvedVariant, TypeDiscovery,
};

/// Convert a Rust identifier to WIT kebab-case.
pub fn to_kebab_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let mut chars = s.chars().peekable();
    let mut prev_was_upper = false;
    let mut prev_was_separator = false;

    while let Some(c) = chars.next() {
        if c == '_' || c == '-' {
            if !result.is_empty() && !prev_was_separator {
                result.push('-');
            }
            prev_was_separator = true;
            prev_was_upper = false;
            continue;
        }

        if c.is_uppercase() {
            let next_is_lower = chars.peek().is_some_and(|n| n.is_lowercase());

            if !result.is_empty() && !prev_was_separator && (!prev_was_upper || next_is_lower) {
                result.push('-');
            }

            result.push(c.to_lowercase().next().unwrap());
            prev_was_upper = true;
        } else {
            result.push(c);
            prev_was_upper = false;
        }
        prev_was_separator = false;
    }

    result
}

/// Generate the complete WIT package string.
pub fn generate_wit(discovery: &TypeDiscovery, package: &str, interface: &str) -> String {
    let mut wit = String::new();
    writeln!(wit, "package {package};").unwrap();
    writeln!(wit).unwrap();
    writeln!(wit, "interface {interface} {{").unwrap();

    // Type definitions.
    for td in &discovery.type_defs {
        emit_type_def(&mut wit, td);
    }

    // Resources.
    for imp in &discovery.impls {
        emit_resource(&mut wit, imp);
    }

    // Standalone functions.
    for func in &discovery.functions {
        emit_function(&mut wit, func, "  ");
    }

    writeln!(wit, "}}").unwrap();
    writeln!(wit).unwrap();
    writeln!(wit, "world component-world {{").unwrap();
    writeln!(wit, "  export {interface};").unwrap();
    writeln!(wit, "}}").unwrap();

    wit
}

fn emit_type_def(wit: &mut String, td: &ResolvedTypeDef) {
    match td {
        ResolvedTypeDef::Record(r) => emit_record(wit, r),
        ResolvedTypeDef::Enum(e) => emit_enum(wit, e),
        ResolvedTypeDef::Variant(v) => emit_variant(wit, v),
    }
}

fn emit_record(wit: &mut String, r: &ResolvedRecord) {
    let name = to_kebab_case(&r.name);
    writeln!(wit, "  record {name} {{").unwrap();
    for (i, (field_name, field_type)) in r.fields.iter().enumerate() {
        let fname = to_kebab_case(field_name);
        let ftype = type_to_wit(field_type);
        let comma = if i < r.fields.len() - 1 { "," } else { "" };
        writeln!(wit, "    {fname}: {ftype}{comma}").unwrap();
    }
    writeln!(wit, "  }}").unwrap();
    writeln!(wit).unwrap();
}

fn emit_enum(wit: &mut String, e: &ResolvedEnum) {
    let name = to_kebab_case(&e.name);
    writeln!(wit, "  enum {name} {{").unwrap();
    for (i, case) in e.cases.iter().enumerate() {
        let cname = to_kebab_case(case);
        let comma = if i < e.cases.len() - 1 { "," } else { "" };
        writeln!(wit, "    {cname}{comma}").unwrap();
    }
    writeln!(wit, "  }}").unwrap();
    writeln!(wit).unwrap();
}

fn emit_variant(wit: &mut String, v: &ResolvedVariant) {
    let name = to_kebab_case(&v.name);
    writeln!(wit, "  variant {name} {{").unwrap();
    for (i, (case_name, payload)) in v.cases.iter().enumerate() {
        let cname = to_kebab_case(case_name);
        let comma = if i < v.cases.len() - 1 { "," } else { "" };
        if let Some(ty) = payload {
            let wit_ty = type_to_wit(ty);
            writeln!(wit, "    {cname}({wit_ty}){comma}").unwrap();
        } else {
            writeln!(wit, "    {cname}{comma}").unwrap();
        }
    }
    writeln!(wit, "  }}").unwrap();
    writeln!(wit).unwrap();
}

fn emit_resource(wit: &mut String, imp: &ExportedImpl) {
    let name = to_kebab_case(&imp.type_name);
    writeln!(wit, "  resource {name} {{").unwrap();

    if let Some(ctor) = &imp.constructor {
        let params = params_to_wit(&ctor.params);
        writeln!(wit, "    constructor({params});").unwrap();
    }

    for method in &imp.methods {
        let fname = to_kebab_case(&method.name);
        let params = params_to_wit(&method.params);
        let ret = return_to_wit(&method.return_type);
        let async_kw = if method.is_async { "async " } else { "" };
        writeln!(wit, "    {fname}: {async_kw}func({params}){ret};").unwrap();
    }

    for static_fn in &imp.static_funcs {
        let fname = to_kebab_case(&static_fn.name);
        let params = params_to_wit(&static_fn.params);
        let ret = return_to_wit(&static_fn.return_type);
        let async_kw = if static_fn.is_async { "async " } else { "" };
        writeln!(wit, "    {fname}: static {async_kw}func({params}){ret};").unwrap();
    }

    writeln!(wit, "  }}").unwrap();
    writeln!(wit).unwrap();
}

fn emit_function(wit: &mut String, func: &ExportedFunction, indent: &str) {
    let fname = to_kebab_case(&func.name);
    let params = params_to_wit(&func.params);
    let ret = return_to_wit(&func.return_type);
    let async_kw = if func.is_async { "async " } else { "" };
    writeln!(wit, "{indent}{fname}: {async_kw}func({params}){ret};").unwrap();
}

fn params_to_wit(params: &[(String, ResolvedType)]) -> String {
    params
        .iter()
        .map(|(name, ty)| {
            let pname = to_kebab_case(name);
            let pty = type_to_wit(ty);
            format!("{pname}: {pty}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn return_to_wit(ret: &Option<ResolvedType>) -> String {
    match ret {
        Some(ty) if !matches!(ty, ResolvedType::Tuple(e) if e.is_empty()) => {
            format!(" -> {}", type_to_wit(ty))
        }
        _ => String::new(),
    }
}

fn type_to_wit(ty: &ResolvedType) -> String {
    match ty {
        ResolvedType::Bool => "bool".into(),
        ResolvedType::U8 => "u8".into(),
        ResolvedType::U16 => "u16".into(),
        ResolvedType::U32 => "u32".into(),
        ResolvedType::U64 => "u64".into(),
        ResolvedType::S8 => "s8".into(),
        ResolvedType::S16 => "s16".into(),
        ResolvedType::S32 => "s32".into(),
        ResolvedType::S64 => "s64".into(),
        ResolvedType::F32 => "f32".into(),
        ResolvedType::F64 => "f64".into(),
        ResolvedType::Char => "char".into(),
        ResolvedType::WitString => "string".into(),
        ResolvedType::List(inner) => format!("list<{}>", type_to_wit(inner)),
        ResolvedType::Option(inner) => format!("option<{}>", type_to_wit(inner)),
        ResolvedType::Result { ok, err } => {
            let ok_str = ok.as_ref().map(|t| type_to_wit(t));
            let err_str = err.as_ref().map(|t| type_to_wit(t));
            match (ok_str, err_str) {
                (Some(o), Some(e)) => format!("result<{o}, {e}>"),
                (Some(o), None) => format!("result<{o}>"),
                (None, Some(e)) => format!("result<_, {e}>"),
                (None, None) => "result".into(),
            }
        }
        ResolvedType::Tuple(elems) => {
            if elems.is_empty() {
                "tuple<>".into()
            } else {
                let parts: Vec<_> = elems.iter().map(type_to_wit).collect();
                format!("tuple<{}>", parts.join(", "))
            }
        }
        ResolvedType::Future(inner) => match inner {
            Some(t) => format!("future<{}>", type_to_wit(t)),
            None => "future".into(),
        },
        ResolvedType::Stream(inner) => match inner {
            Some(t) => format!("stream<{}>", type_to_wit(t)),
            None => "stream".into(),
        },
        ResolvedType::Named(name) | ResolvedType::Own(name) => to_kebab_case(name),
        ResolvedType::Borrow(name) => format!("borrow<{}>", to_kebab_case(name)),
    }
}
