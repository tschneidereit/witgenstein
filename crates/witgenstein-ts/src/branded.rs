// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Branded type detection: scan modules for branded type aliases and imports
//! from the `witgenstein-ts` package to build a map of type name → WIT type.

use std::collections::HashMap;

use oxc_ast::ast::{
    Declaration, ImportDeclarationSpecifier, Program, PropertyKey, TSType, TSTypeName,
};
use oxc_ast_visit::Visit;

use crate::types::WitType;

/// Map from TypeScript type name to the WIT type it represents.
pub type BrandedTypeMap = HashMap<String, WitType>;

/// Well-known branded type names and their WIT equivalents.
const BRANDED_NAMES: &[(&str, WitType)] = &[
    ("u8", WitType::U8),
    ("u16", WitType::U16),
    ("u32", WitType::U32),
    ("u64", WitType::U64),
    ("s8", WitType::S8),
    ("s16", WitType::S16),
    ("s32", WitType::S32),
    ("s64", WitType::S64),
    ("f32", WitType::F32),
    ("f64", WitType::F64),
    ("char", WitType::Char),
];

/// Scan a program's AST for branded type definitions and witgenstein-ts imports.
pub fn collect_branded_types(program: &Program<'_>) -> BrandedTypeMap {
    let mut collector = BrandedTypeCollector {
        branded: HashMap::new(),
    };
    collector.visit_program(program);
    collector.branded
}

struct BrandedTypeCollector {
    branded: BrandedTypeMap,
}

impl<'a> Visit<'a> for BrandedTypeCollector {
    fn visit_import_declaration(&mut self, decl: &oxc_ast::ast::ImportDeclaration<'a>) {
        // Detect `import { u32, s64, ... } from 'witgenstein-ts'`
        if decl.source.value.as_str() != "witgenstein-ts" {
            return;
        }

        let Some(specifiers) = &decl.specifiers else {
            return;
        };

        for spec in specifiers {
            if let ImportDeclarationSpecifier::ImportSpecifier(s) = spec {
                let local_name = s.local.name.as_str();
                if let Some(wit_type) = lookup_branded_name(local_name) {
                    self.branded.insert(local_name.to_string(), wit_type);
                }
            }
        }
    }

    fn visit_ts_type_alias_declaration(&mut self, decl: &oxc_ast::ast::TSTypeAliasDeclaration<'a>) {
        let name = decl.id.name.as_str();

        // Only consider names that match known branded type names
        let Some(wit_type) = lookup_branded_name(name) else {
            return;
        };

        // Check if it matches the pattern: `type u32 = number & { __brand: 'u32' }`
        // This is a TSIntersectionType with a number keyword and an object with __brand
        if is_branded_type_pattern(&decl.type_annotation, name) {
            self.branded.insert(name.to_string(), wit_type);
        }
    }

    fn visit_declaration(&mut self, decl: &Declaration<'a>) {
        // Walk into declarations to find type aliases
        if let Declaration::TSTypeAliasDeclaration(alias) = decl {
            self.visit_ts_type_alias_declaration(alias);
        }
    }
}

/// Look up a branded type name in the well-known list.
fn lookup_branded_name(name: &str) -> Option<WitType> {
    BRANDED_NAMES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, t)| t.clone())
}

/// Check if a type annotation matches the branded type pattern:
/// `number & { __brand: 'name' }` or similar intersection patterns.
fn is_branded_type_pattern(ty: &TSType<'_>, expected_name: &str) -> bool {
    match ty {
        TSType::TSIntersectionType(intersection) => {
            let mut has_base = false;
            let mut has_brand = false;

            for member in &intersection.types {
                match member {
                    // The base type (number, string, etc.)
                    TSType::TSNumberKeyword(_)
                    | TSType::TSStringKeyword(_)
                    | TSType::TSBigIntKeyword(_) => {
                        has_base = true;
                    }
                    // The brand object: { __brand: 'name' }
                    TSType::TSTypeLiteral(literal) => {
                        for member in &literal.members {
                            if let oxc_ast::ast::TSSignature::TSPropertySignature(prop) = member
                                && let PropertyKey::StaticIdentifier(id) = &prop.key
                                && id.name.as_str() == "__brand"
                            {
                                // Check if the value is a string literal matching expected_name
                                if let Some(type_ann) = &prop.type_annotation
                                    && is_string_literal_type(
                                        &type_ann.type_annotation,
                                        expected_name,
                                    )
                                {
                                    has_brand = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            has_base && has_brand
        }

        // Also accept bare type reference to a known branded name if it was
        // previously registered (e.g. re-imported under same name)
        TSType::TSTypeReference(reference) => {
            if let TSTypeName::IdentifierReference(ident) = &reference.type_name {
                ident.name.as_str() == expected_name
            } else {
                false
            }
        }

        _ => false,
    }
}

/// Check if a TSType is a string literal type with the given value.
fn is_string_literal_type(ty: &TSType<'_>, expected: &str) -> bool {
    if let TSType::TSLiteralType(lit) = ty
        && let oxc_ast::ast::TSLiteral::StringLiteral(s) = &lit.literal
    {
        return s.value.as_str() == expected;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_branded_name() {
        assert_eq!(lookup_branded_name("u32"), Some(WitType::U32));
        assert_eq!(lookup_branded_name("s64"), Some(WitType::S64));
        assert_eq!(lookup_branded_name("f64"), Some(WitType::F64));
        assert_eq!(lookup_branded_name("char"), Some(WitType::Char));
        assert_eq!(lookup_branded_name("int"), None);
        assert_eq!(lookup_branded_name("string"), None);
    }
}
