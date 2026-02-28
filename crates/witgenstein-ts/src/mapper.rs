// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Type mapper: convert collected TypeScript type information into WIT IR types.
//!
//! This module takes the `TypeInfo` descriptors from the export collector and
//! the branded type map, and produces `WitType` values for the WIT emitter.

use std::collections::HashMap;

use crate::branded::BrandedTypeMap;
use crate::collect::{
    ExportedClass, ExportedEnum, ExportedFunction, ExportedInterface, ExportedItem,
    ExportedTypeAlias, KeywordType, MethodInfo, PropertyInfo, TypeInfo,
};
use crate::diagnostic::{Diagnostic, DiagnosticKind};
use crate::types::{
    WitEnum, WitFlags, WitFunc, WitRecord, WitResource, WitType, WitTypeAlias, WitTypeDef,
    WitVariant,
};

/// Result of mapping TypeScript exports to WIT IR.
pub struct MapResult {
    pub type_defs: Vec<WitTypeDef>,
    pub functions: Vec<WitFunc>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Configuration for the type mapper.
pub struct MapperConfig {
    pub strict: bool,
}

/// Map collected TypeScript exports to WIT IR.
pub fn map_exports(
    items: &[ExportedItem],
    branded: &BrandedTypeMap,
    config: &MapperConfig,
) -> MapResult {
    let mut ctx = MapContext {
        branded,
        config,
        type_defs: Vec::new(),
        functions: Vec::new(),
        diagnostics: Vec::new(),
        known_types: HashMap::new(),
    };

    // First pass: register all type names so we can reference them
    for item in items {
        match item {
            ExportedItem::Interface(iface) => {
                ctx.known_types
                    .insert(iface.name.clone(), to_kebab_case(&iface.name));
            }
            ExportedItem::TypeAlias(alias) => {
                ctx.known_types
                    .insert(alias.name.clone(), to_kebab_case(&alias.name));
            }
            ExportedItem::Enum(e) => {
                ctx.known_types
                    .insert(e.name.clone(), to_kebab_case(&e.name));
            }
            ExportedItem::Class(c) => {
                ctx.known_types
                    .insert(c.name.clone(), to_kebab_case(&c.name));
            }
            _ => {}
        }
    }

    // Second pass: map all items
    for item in items {
        match item {
            ExportedItem::Function(func) => ctx.map_function(func),
            ExportedItem::Class(class) => ctx.map_class(class),
            ExportedItem::Interface(iface) => ctx.map_interface(iface),
            ExportedItem::TypeAlias(alias) => ctx.map_type_alias(alias),
            ExportedItem::Enum(e) => ctx.map_enum(e),
        }
    }

    MapResult {
        type_defs: ctx.type_defs,
        functions: ctx.functions,
        diagnostics: ctx.diagnostics,
    }
}

struct MapContext<'a> {
    branded: &'a BrandedTypeMap,
    config: &'a MapperConfig,
    type_defs: Vec<WitTypeDef>,
    functions: Vec<WitFunc>,
    diagnostics: Vec<Diagnostic>,
    /// Map from TS type name to WIT kebab-case name.
    known_types: HashMap<String, String>,
}

impl MapContext<'_> {
    fn map_function(&mut self, func: &ExportedFunction) {
        let params: Vec<(String, WitType)> = func
            .params
            .iter()
            .filter_map(|p| {
                let wit_type = self.map_type_info(&p.type_info, &p.name);
                let wit_type = if p.optional {
                    wit_type.map(|t| WitType::Option(Box::new(t)))
                } else {
                    wit_type
                };
                wit_type.map(|t| (to_kebab_case(&p.name), t))
            })
            .collect();

        let result = func
            .return_type
            .as_ref()
            .and_then(|rt| self.map_return_type(rt, func.is_async));

        self.functions.push(WitFunc {
            name: to_kebab_case(&func.name),
            params,
            result,
            is_async: func.is_async,
        });
    }

    fn map_class(&mut self, class: &ExportedClass) {
        let is_resource = class.decorators.iter().any(|d| d == "wit.resource");
        let is_flags = class.decorators.iter().any(|d| d == "wit.flags");

        if is_flags {
            // Treat as flags (unusual but supported via decorator)
            let flags: Vec<String> = class
                .properties
                .iter()
                .map(|p| to_kebab_case(&p.name))
                .collect();
            self.type_defs.push(WitTypeDef::Flags(WitFlags {
                name: to_kebab_case(&class.name),
                flags,
            }));
            return;
        }

        // Default: treat classes as resources
        let _ = is_resource; // decorator just confirms the default behavior

        let constructor_params = class.constructor_params.as_ref().map(|params| {
            params
                .iter()
                .filter_map(|p| {
                    self.map_type_info(&p.type_info, &p.name)
                        .map(|t| (to_kebab_case(&p.name), t))
                })
                .collect()
        });

        let methods: Vec<WitFunc> = class.methods.iter().map(|m| self.map_method(m)).collect();

        let static_funcs: Vec<WitFunc> = class
            .static_methods
            .iter()
            .map(|m| self.map_method(m))
            .collect();

        self.type_defs.push(WitTypeDef::Resource(WitResource {
            name: to_kebab_case(&class.name),
            constructor_params,
            methods,
            static_funcs,
        }));
    }

    fn map_interface(&mut self, iface: &ExportedInterface) {
        let fields: Vec<(String, WitType)> = iface
            .properties
            .iter()
            .filter_map(|p| {
                let wit_type = self.map_type_info(&p.type_info, &p.name);
                let wit_type = if p.optional {
                    wit_type.map(|t| WitType::Option(Box::new(t)))
                } else {
                    wit_type
                };
                wit_type.map(|t| (to_kebab_case(&p.name), t))
            })
            .collect();

        self.type_defs.push(WitTypeDef::Record(WitRecord {
            name: to_kebab_case(&iface.name),
            fields,
        }));
    }

    fn map_type_alias(&mut self, alias: &ExportedTypeAlias) {
        let wit_name = to_kebab_case(&alias.name);

        match &alias.type_info {
            // Union of object literals → could be a variant
            TypeInfo::Union(members) => {
                if let Some(variant) = try_map_variant(&wit_name, members, self) {
                    self.type_defs.push(WitTypeDef::Variant(variant));
                    return;
                }
                // Otherwise treat as a regular type alias
                if let Some(wit_type) = self.map_type_info(&alias.type_info, &alias.name) {
                    self.type_defs.push(WitTypeDef::TypeAlias(WitTypeAlias {
                        name: wit_name,
                        target: wit_type,
                    }));
                }
            }
            TypeInfo::ObjectLiteral(props) => {
                // Object literal type alias → record
                let fields: Vec<(String, WitType)> = props
                    .iter()
                    .filter_map(|p| {
                        let wit_type = self.map_type_info(&p.type_info, &p.name);
                        let wit_type = if p.optional {
                            wit_type.map(|t| WitType::Option(Box::new(t)))
                        } else {
                            wit_type
                        };
                        wit_type.map(|t| (to_kebab_case(&p.name), t))
                    })
                    .collect();
                self.type_defs.push(WitTypeDef::Record(WitRecord {
                    name: wit_name,
                    fields,
                }));
            }
            _ => {
                if let Some(wit_type) = self.map_type_info(&alias.type_info, &alias.name) {
                    self.type_defs.push(WitTypeDef::TypeAlias(WitTypeAlias {
                        name: wit_name,
                        target: wit_type,
                    }));
                }
            }
        }
    }

    fn map_enum(&mut self, e: &ExportedEnum) {
        let wit_name = to_kebab_case(&e.name);

        if e.has_flags_decorator {
            self.type_defs.push(WitTypeDef::Flags(WitFlags {
                name: wit_name,
                flags: e.members.iter().map(|m| to_kebab_case(&m.name)).collect(),
            }));
        } else {
            self.type_defs.push(WitTypeDef::Enum(WitEnum {
                name: wit_name,
                cases: e.members.iter().map(|m| to_kebab_case(&m.name)).collect(),
            }));
        }
    }

    fn map_method(&mut self, method: &MethodInfo) -> WitFunc {
        let params: Vec<(String, WitType)> = method
            .params
            .iter()
            .filter_map(|p| {
                self.map_type_info(&p.type_info, &p.name)
                    .map(|t| (to_kebab_case(&p.name), t))
            })
            .collect();

        let result = method
            .return_type
            .as_ref()
            .and_then(|rt| self.map_return_type(rt, method.is_async));

        WitFunc {
            name: to_kebab_case(&method.name),
            params,
            result,
            is_async: method.is_async,
        }
    }

    fn map_return_type(&mut self, type_info: &TypeInfo, is_async: bool) -> Option<WitType> {
        // For async functions, unwrap Promise<T> to get the inner type
        if is_async
            && let TypeInfo::Reference { name, type_args } = type_info
            && name == "Promise"
            && type_args.len() == 1
        {
            return self.map_type_info(&type_args[0], "return");
        }

        match type_info {
            TypeInfo::Keyword(KeywordType::Void) => None,
            _ => self.map_type_info(type_info, "return"),
        }
    }

    /// Map a TypeInfo to a WitType, recording diagnostics for unmappable types.
    fn map_type_info(&mut self, type_info: &TypeInfo, context: &str) -> Option<WitType> {
        match type_info {
            TypeInfo::Keyword(kw) => self.map_keyword(*kw, context),

            TypeInfo::Reference { name, type_args } => {
                self.map_type_reference(name, type_args, context)
            }

            TypeInfo::Array(elem) => {
                let inner = self.map_type_info(elem, context)?;
                Some(WitType::List(Box::new(inner)))
            }

            TypeInfo::Tuple(elems) => {
                let mapped: Vec<WitType> = elems
                    .iter()
                    .filter_map(|e| self.map_type_info(e, context))
                    .collect();
                Some(WitType::Tuple(mapped))
            }

            TypeInfo::Union(members) => self.map_union(members, context),

            TypeInfo::ObjectLiteral(props) => {
                // Inline object literal → anonymous record isn't directly supported
                // in WIT, but we map it to a named record via the parent context
                let fields: Vec<(String, WitType)> = props
                    .iter()
                    .filter_map(|p| {
                        let wt = self.map_type_info(&p.type_info, &p.name)?;
                        let wt = if p.optional {
                            WitType::Option(Box::new(wt))
                        } else {
                            wt
                        };
                        Some((to_kebab_case(&p.name), wt))
                    })
                    .collect();
                // Return as a named reference; the caller should create the record
                // For now, create an inline record type def
                let name = format!("{context}-record");
                let wit_name = to_kebab_case(&name);
                self.type_defs.push(WitTypeDef::Record(WitRecord {
                    name: wit_name.clone(),
                    fields,
                }));
                Some(WitType::Named(wit_name))
            }

            TypeInfo::Intersection => {
                // Intersection types can't be directly mapped to WIT
                self.diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::UnsupportedType,
                    message: format!("intersection types cannot be mapped to WIT (in {context})"),
                });
                None
            }

            TypeInfo::FunctionType => {
                self.diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::UnsupportedType,
                    message: format!(
                        "function types cannot be mapped to WIT — WIT has no first-class functions (in {context})"
                    ),
                });
                None
            }

            TypeInfo::Null | TypeInfo::Undefined => {
                // Standalone null/undefined aren't valid WIT types
                None
            }

            TypeInfo::Unknown(desc) => {
                self.diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::UnresolvableType,
                    message: format!("could not resolve type in {context}: {desc}"),
                });
                None
            }
        }
    }

    fn map_keyword(&mut self, kw: KeywordType, context: &str) -> Option<WitType> {
        match kw {
            KeywordType::String => Some(WitType::WitString),
            KeywordType::Boolean => Some(WitType::Bool),
            KeywordType::BigInt => Some(WitType::S64),
            KeywordType::Void => None,
            KeywordType::Never => None,

            KeywordType::Number => {
                if self.config.strict {
                    self.diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::AmbiguousNumeric,
                        message: format!(
                            "ambiguous `number` type requires a WIT type annotation (in {context})"
                        ),
                    });
                    None
                } else {
                    Some(WitType::F64)
                }
            }

            KeywordType::Any => {
                self.diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::UnsupportedType,
                    message: format!("`any` has no WIT equivalent (in {context})"),
                });
                None
            }

            KeywordType::Unknown => {
                self.diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::UnsupportedType,
                    message: format!("`unknown` has no WIT equivalent (in {context})"),
                });
                None
            }

            KeywordType::Symbol => {
                self.diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::UnsupportedType,
                    message: format!("`symbol` has no WIT equivalent (in {context})"),
                });
                None
            }

            KeywordType::Object => {
                self.diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::UnsupportedType,
                    message: format!(
                        "`object` is too broad for WIT — use a specific interface or type (in {context})"
                    ),
                });
                None
            }
        }
    }

    fn map_type_reference(
        &mut self,
        name: &str,
        type_args: &[TypeInfo],
        context: &str,
    ) -> Option<WitType> {
        // Check branded types first
        if let Some(wit_type) = self.branded.get(name) {
            return Some(wit_type.clone());
        }

        // Built-in generic mappings
        match name {
            "Array" | "ReadonlyArray" => {
                if type_args.len() == 1 {
                    let inner = self.map_type_info(&type_args[0], context)?;
                    return Some(WitType::List(Box::new(inner)));
                }
            }
            "Promise" => {
                if type_args.len() == 1 {
                    let inner = self.map_type_info(&type_args[0], context)?;
                    return Some(WitType::Future(Some(Box::new(inner))));
                } else {
                    return Some(WitType::Future(None));
                }
            }
            "Map" | "Set" | "WeakMap" | "WeakSet" => {
                self.diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::UnsupportedType,
                    message: format!("`{name}` has no direct WIT equivalent (in {context})"),
                });
                return None;
            }
            _ => {}
        }

        // Typed array special cases
        if let Some(wit_type) = map_typed_array(name) {
            return Some(wit_type);
        }

        // Check if it's a known type (interface, enum, class, type alias)
        if let Some(wit_name) = self.known_types.get(name) {
            return Some(WitType::Named(wit_name.clone()));
        }

        // If it has type args and isn't a built-in, it's a user generic → error
        if !type_args.is_empty() {
            self.diagnostics.push(Diagnostic {
                kind: DiagnosticKind::UnsupportedType,
                message: format!(
                    "user-defined generic type `{name}<...>` cannot be mapped to WIT (in {context})"
                ),
            });
            return None;
        }

        // Assume it's a reference to a type defined elsewhere
        Some(WitType::Named(to_kebab_case(name)))
    }

    fn map_union(&mut self, members: &[TypeInfo], context: &str) -> Option<WitType> {
        // Check for Option pattern: T | null, T | undefined
        let non_null: Vec<&TypeInfo> = members
            .iter()
            .filter(|m| !matches!(m, TypeInfo::Null | TypeInfo::Undefined))
            .collect();

        if non_null.len() < members.len() {
            // Has null/undefined → this is an Option
            if non_null.len() == 1 {
                let inner = self.map_type_info(non_null[0], context)?;
                return Some(WitType::Option(Box::new(inner)));
            }
            // Multiple non-null types in a nullable union
            // Map the non-null part as a union, then wrap in Option
            let inner = self.map_union(
                &non_null.iter().copied().cloned().collect::<Vec<_>>(),
                context,
            )?;
            return Some(WitType::Option(Box::new(inner)));
        }

        // Check for Result pattern: { ok: T } | { err: E }
        if let Some(result) = try_map_result(members, self) {
            return Some(result);
        }

        // Otherwise, this is a proper variant union (handled at type alias level)
        // For inline unions, diagnostic
        self.diagnostics.push(Diagnostic {
            kind: DiagnosticKind::UnsupportedType,
            message: format!(
                "inline union types should be extracted to a named type alias for WIT variant mapping (in {context})"
            ),
        });
        None
    }
}

/// Try to map a union of object literals to a Result type.
/// Pattern: `{ ok: T } | { err: E }`
fn try_map_result(members: &[TypeInfo], ctx: &mut MapContext<'_>) -> Option<WitType> {
    if members.len() != 2 {
        return None;
    }

    let mut ok_type = None;
    let mut err_type = None;

    for member in members {
        if let TypeInfo::ObjectLiteral(props) = member
            && props.len() == 1
        {
            let prop = &props[0];
            if prop.name == "ok" {
                ok_type = ctx.map_type_info(&prop.type_info, "result-ok");
            } else if prop.name == "err" {
                err_type = ctx.map_type_info(&prop.type_info, "result-err");
            }
        }
    }

    if ok_type.is_some() || err_type.is_some() {
        Some(WitType::Result {
            ok: ok_type.map(Box::new),
            err: err_type.map(Box::new),
        })
    } else {
        None
    }
}

/// Try to map a union of object literals to a WIT variant.
/// Each member should have a common discriminant field.
fn try_map_variant(
    name: &str,
    members: &[TypeInfo],
    ctx: &mut MapContext<'_>,
) -> Option<WitVariant> {
    // All members must be object literals
    let object_members: Vec<&Vec<PropertyInfo>> = members
        .iter()
        .filter_map(|m| {
            if let TypeInfo::ObjectLiteral(props) = m {
                Some(props)
            } else {
                None
            }
        })
        .collect();

    if object_members.len() != members.len() || members.is_empty() {
        return None;
    }

    // Find a common discriminant field (a field present in all members with string literal types)
    // For simplicity, look for a "kind" or "type" or "tag" field
    let discriminant_candidates = ["kind", "type", "tag", "_tag"];
    let discriminant = discriminant_candidates.iter().find(|&&field| {
        object_members
            .iter()
            .all(|props| props.iter().any(|p| p.name == field))
    });

    let discriminant = discriminant?;

    let cases: Vec<(String, Option<WitType>)> = object_members
        .iter()
        .filter_map(|props| {
            // Find the discriminant value
            let disc_prop = props.iter().find(|p| p.name == *discriminant)?;
            let case_name = extract_literal_string_from_type(&disc_prop.type_info)?;

            // Collect the payload (all other fields)
            let payload_fields: Vec<&PropertyInfo> =
                props.iter().filter(|p| p.name != *discriminant).collect();

            let payload = if payload_fields.is_empty() {
                None
            } else if payload_fields.len() == 1 {
                ctx.map_type_info(&payload_fields[0].type_info, &case_name)
            } else {
                // Multiple payload fields → create a record
                let record_name = format!("{name}-{case_name}");
                let fields: Vec<(String, WitType)> = payload_fields
                    .iter()
                    .filter_map(|p| {
                        let wt = ctx.map_type_info(&p.type_info, &p.name)?;
                        Some((to_kebab_case(&p.name), wt))
                    })
                    .collect();
                ctx.type_defs.push(WitTypeDef::Record(WitRecord {
                    name: record_name.clone(),
                    fields,
                }));
                Some(WitType::Named(record_name))
            };

            Some((to_kebab_case(&case_name), payload))
        })
        .collect();

    if cases.is_empty() {
        return None;
    }

    Some(WitVariant {
        name: name.to_string(),
        cases,
    })
}

/// Try to extract a string literal value from a TypeInfo.
fn extract_literal_string_from_type(type_info: &TypeInfo) -> Option<String> {
    // In practice, discriminant types are string literal types,
    // which show up as Reference types with the literal value as name
    // or as Unknown containing the literal info.
    // For now, handle Reference types that look like string literals.
    match type_info {
        TypeInfo::Reference { name, type_args } if type_args.is_empty() => Some(name.clone()),
        TypeInfo::Unknown(desc) if desc.contains("StringLiteral") => {
            // Try to extract from debug representation
            None
        }
        _ => None,
    }
}

/// Map typed array names to WIT list types.
fn map_typed_array(name: &str) -> Option<WitType> {
    match name {
        "Uint8Array" | "Uint8ClampedArray" => Some(WitType::List(Box::new(WitType::U8))),
        "Uint16Array" => Some(WitType::List(Box::new(WitType::U16))),
        "Uint32Array" => Some(WitType::List(Box::new(WitType::U32))),
        "Int8Array" => Some(WitType::List(Box::new(WitType::S8))),
        "Int16Array" => Some(WitType::List(Box::new(WitType::S16))),
        "Int32Array" => Some(WitType::List(Box::new(WitType::S32))),
        "Float32Array" => Some(WitType::List(Box::new(WitType::F32))),
        "Float64Array" => Some(WitType::List(Box::new(WitType::F64))),
        "BigInt64Array" => Some(WitType::List(Box::new(WitType::S64))),
        "BigUint64Array" => Some(WitType::List(Box::new(WitType::U64))),
        _ => None,
    }
}

/// Re-export `to_kebab_case` from the shared crate for use throughout this crate.
pub use wit_common::to_kebab_case;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("camelCase"), "camel-case");
        assert_eq!(to_kebab_case("PascalCase"), "pascal-case");
        assert_eq!(to_kebab_case("simpleword"), "simpleword");
        assert_eq!(to_kebab_case("HTMLParser"), "html-parser");
        assert_eq!(to_kebab_case("getHTTPResponse"), "get-http-response");
        assert_eq!(to_kebab_case("already-kebab"), "already-kebab");
        assert_eq!(to_kebab_case("snake_case"), "snake-case");
        assert_eq!(to_kebab_case("myFunc"), "my-func");
        assert_eq!(to_kebab_case("a"), "a");
        assert_eq!(to_kebab_case(""), "");
    }

    #[test]
    fn test_map_typed_array() {
        assert_eq!(
            map_typed_array("Uint8Array"),
            Some(WitType::List(Box::new(WitType::U8)))
        );
        assert_eq!(
            map_typed_array("Int32Array"),
            Some(WitType::List(Box::new(WitType::S32)))
        );
        assert_eq!(
            map_typed_array("Float64Array"),
            Some(WitType::List(Box::new(WitType::F64)))
        );
        assert_eq!(
            map_typed_array("BigInt64Array"),
            Some(WitType::List(Box::new(WitType::S64)))
        );
        assert_eq!(map_typed_array("String"), None);
    }
}
