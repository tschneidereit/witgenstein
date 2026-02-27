// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! WIT emitter: format the WIT IR into valid `.wit` syntax and write it out.

use std::fmt::Write;

use crate::types::{
    WitEnum, WitFlags, WitFunc, WitPackage, WitRecord, WitResource, WitTypeAlias, WitTypeDef,
    WitVariant, WitWorld,
};

/// Emit a complete WIT package to a string.
pub fn emit_wit(package: &WitPackage) -> String {
    let mut out = String::with_capacity(1024);

    emit_package_header(&mut out, package);
    out.push('\n');
    emit_world(&mut out, &package.world);

    out
}

fn emit_package_header(out: &mut String, package: &WitPackage) {
    write!(out, "package {}:{}", package.namespace, package.name).unwrap();
    if let Some(version) = &package.version {
        write!(out, "@{version}").unwrap();
    }
    out.push_str(";\n");
}

fn emit_world(out: &mut String, world: &WitWorld) {
    // Emit import interfaces first
    for import in &world.imports {
        writeln!(out, "interface {} {{", import.name).unwrap();
        emit_interface_body(out, &import.type_defs, &import.functions);
        out.push_str("}\n\n");
    }

    // Emit the exports interface if it has type definitions
    let has_export_types = !world.exports.type_defs.is_empty();
    if has_export_types {
        writeln!(out, "interface {} {{", world.exports.name).unwrap();
        emit_interface_body(out, &world.exports.type_defs, &[]);
        out.push_str("}\n\n");
    }

    // Emit the world
    writeln!(out, "world {} {{", world.name).unwrap();

    // Import external interfaces
    for import in &world.imports {
        writeln!(out, "  import {};", import.name).unwrap();
    }

    // Export the types interface
    if has_export_types {
        writeln!(out, "  export {};", world.exports.name).unwrap();
    }

    // Export functions directly
    for func in &world.exports.functions {
        emit_func(out, func, "  export ");
    }

    out.push_str("}\n");
}

fn emit_interface_body(out: &mut String, type_defs: &[WitTypeDef], functions: &[WitFunc]) {
    for (i, type_def) in type_defs.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        emit_type_def(out, type_def, "  ");
    }

    if !type_defs.is_empty() && !functions.is_empty() {
        out.push('\n');
    }

    for func in functions {
        emit_func(out, func, "  ");
    }
}

fn emit_type_def(out: &mut String, type_def: &WitTypeDef, indent: &str) {
    match type_def {
        WitTypeDef::Record(record) => emit_record(out, record, indent),
        WitTypeDef::Resource(resource) => emit_resource(out, resource, indent),
        WitTypeDef::Enum(e) => emit_enum(out, e, indent),
        WitTypeDef::Variant(v) => emit_variant(out, v, indent),
        WitTypeDef::Flags(f) => emit_flags(out, f, indent),
        WitTypeDef::TypeAlias(a) => emit_type_alias(out, a, indent),
    }
}

fn emit_record(out: &mut String, record: &WitRecord, indent: &str) {
    writeln!(out, "{indent}record {} {{", record.name).unwrap();
    for (name, ty) in &record.fields {
        writeln!(out, "{indent}  {name}: {ty},").unwrap();
    }
    writeln!(out, "{indent}}}").unwrap();
}

fn emit_resource(out: &mut String, resource: &WitResource, indent: &str) {
    writeln!(out, "{indent}resource {} {{", resource.name).unwrap();

    if let Some(params) = &resource.constructor_params {
        write!(out, "{indent}  constructor(").unwrap();
        emit_param_list(out, params);
        out.push_str(");\n");
    }

    for method in &resource.methods {
        emit_func(out, method, &format!("{indent}  "));
    }

    for func in &resource.static_funcs {
        write!(out, "{indent}  ").unwrap();
        emit_static_func(out, func);
    }

    writeln!(out, "{indent}}}").unwrap();
}

fn emit_enum(out: &mut String, e: &WitEnum, indent: &str) {
    writeln!(out, "{indent}enum {} {{", e.name).unwrap();
    for case in &e.cases {
        writeln!(out, "{indent}  {case},").unwrap();
    }
    writeln!(out, "{indent}}}").unwrap();
}

fn emit_variant(out: &mut String, v: &WitVariant, indent: &str) {
    writeln!(out, "{indent}variant {} {{", v.name).unwrap();
    for (case_name, payload) in &v.cases {
        match payload {
            Some(ty) => writeln!(out, "{indent}  {case_name}({ty}),").unwrap(),
            None => writeln!(out, "{indent}  {case_name},").unwrap(),
        }
    }
    writeln!(out, "{indent}}}").unwrap();
}

fn emit_flags(out: &mut String, f: &WitFlags, indent: &str) {
    writeln!(out, "{indent}flags {} {{", f.name).unwrap();
    for flag in &f.flags {
        writeln!(out, "{indent}  {flag},").unwrap();
    }
    writeln!(out, "{indent}}}").unwrap();
}

fn emit_type_alias(out: &mut String, a: &WitTypeAlias, indent: &str) {
    writeln!(out, "{indent}type {} = {};", a.name, a.target).unwrap();
}

fn emit_func(out: &mut String, func: &WitFunc, prefix: &str) {
    write!(out, "{prefix}").unwrap();
    if func.is_async {
        write!(out, "async ").unwrap();
    }
    write!(out, "{}: func(", func.name).unwrap();
    emit_param_list(out, &func.params);
    out.push(')');

    if let Some(result) = &func.result {
        write!(out, " -> {result}").unwrap();
    }

    out.push_str(";\n");
}

fn emit_static_func(out: &mut String, func: &WitFunc) {
    write!(out, "{}: static func(", func.name).unwrap();
    emit_param_list(out, &func.params);
    out.push(')');

    if let Some(result) = &func.result {
        write!(out, " -> {result}").unwrap();
    }

    out.push_str(";\n");
}

fn emit_param_list(out: &mut String, params: &[(String, crate::types::WitType)]) {
    for (i, (name, ty)) in params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write!(out, "{name}: {ty}").unwrap();
    }
}
