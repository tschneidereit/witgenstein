// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Export collector: walk the entry file's AST and collect all exported
//! declarations (functions, classes, interfaces, type aliases, enums).
//!
//! The collected items are stored as lightweight descriptors referencing back
//! to the source AST positions for later type mapping.

use oxc_ast::ast::{
    Class, Declaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind,
    ExportNamedDeclaration, FormalParameter, Function, MethodDefinition, MethodDefinitionKind,
    PropertyDefinition, TSEnumDeclaration, TSInterfaceDeclaration, TSType, TSTypeAliasDeclaration,
    TSTypeName,
};
use oxc_ast_visit::Visit;

/// A collected exported item from the TypeScript source.
#[derive(Debug)]
pub enum ExportedItem {
    Function(ExportedFunction),
    Class(ExportedClass),
    Interface(ExportedInterface),
    TypeAlias(ExportedTypeAlias),
    Enum(ExportedEnum),
}

/// An exported function.
#[derive(Debug)]
pub struct ExportedFunction {
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub return_type: Option<TypeInfo>,
    pub is_async: bool,
}

/// An exported class (potentially a WIT resource).
#[derive(Debug)]
pub struct ExportedClass {
    pub name: String,
    pub constructor_params: Option<Vec<ParamInfo>>,
    pub methods: Vec<MethodInfo>,
    pub static_methods: Vec<MethodInfo>,
    pub properties: Vec<PropertyInfo>,
    pub decorators: Vec<String>,
}

/// An exported interface (maps to a WIT record).
#[derive(Debug)]
pub struct ExportedInterface {
    pub name: String,
    pub properties: Vec<PropertyInfo>,
}

/// An exported type alias.
#[derive(Debug)]
pub struct ExportedTypeAlias {
    pub name: String,
    pub type_info: TypeInfo,
}

/// An exported enum.
#[derive(Debug)]
pub struct ExportedEnum {
    pub name: String,
    pub members: Vec<EnumMemberInfo>,
    pub has_flags_decorator: bool,
}

/// Information about a function/method parameter.
#[derive(Debug)]
pub struct ParamInfo {
    pub name: String,
    pub type_info: TypeInfo,
    pub optional: bool,
}

/// Information about a class/interface method.
#[derive(Debug)]
pub struct MethodInfo {
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub return_type: Option<TypeInfo>,
    pub is_async: bool,
}

/// Information about a class/interface property.
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    pub name: String,
    pub type_info: TypeInfo,
    pub optional: bool,
}

/// Information about an enum member.
#[derive(Debug)]
pub struct EnumMemberInfo {
    pub name: String,
}

/// A lightweight description of a TypeScript type, extracted from the AST.
/// This is the bridge between the OXC AST types and our WIT IR.
#[derive(Debug, Clone)]
pub enum TypeInfo {
    /// A keyword type (number, string, boolean, etc.)
    Keyword(KeywordType),
    /// A reference to a named type (e.g. `MyInterface`, `u32`)
    Reference {
        name: String,
        type_args: Vec<TypeInfo>,
    },
    /// An array type: `T[]`
    Array(Box<TypeInfo>),
    /// A tuple type: `[T1, T2]`
    Tuple(Vec<TypeInfo>),
    /// A union type: `A | B`
    Union(Vec<TypeInfo>),
    /// An intersection type: `A & B` (payload elided; not mappable to WIT)
    Intersection,
    /// An object literal type: `{ foo: string, bar: number }`
    ObjectLiteral(Vec<PropertyInfo>),
    /// A function type (not mappable to WIT)
    FunctionType,
    /// null literal type
    Null,
    /// undefined literal type
    Undefined,
    /// A type we couldn't resolve
    Unknown(String),
}

/// TypeScript keyword types.
#[derive(Debug, Clone, Copy)]
pub enum KeywordType {
    Number,
    String,
    Boolean,
    BigInt,
    Void,
    Any,
    Unknown,
    Never,
    Symbol,
    Object,
}

/// Collects exported items from a program's AST.
pub struct ExportCollector {
    pub items: Vec<ExportedItem>,
}

impl ExportCollector {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn collect(program: &oxc_ast::ast::Program<'_>) -> Vec<ExportedItem> {
        let mut collector = Self::new();
        collector.visit_program(program);
        collector.items
    }
}

impl<'a> Visit<'a> for ExportCollector {
    fn visit_export_named_declaration(&mut self, decl: &ExportNamedDeclaration<'a>) {
        if let Some(declaration) = &decl.declaration {
            match declaration {
                Declaration::FunctionDeclaration(func) => {
                    if let Some(item) = collect_function(func) {
                        self.items.push(ExportedItem::Function(item));
                    }
                }
                Declaration::ClassDeclaration(class) => {
                    if let Some(item) = collect_class(class) {
                        self.items.push(ExportedItem::Class(item));
                    }
                }
                Declaration::TSInterfaceDeclaration(iface) => {
                    if let Some(item) = collect_interface(iface) {
                        self.items.push(ExportedItem::Interface(item));
                    }
                }
                Declaration::TSTypeAliasDeclaration(alias) => {
                    if let Some(item) = collect_type_alias(alias) {
                        self.items.push(ExportedItem::TypeAlias(item));
                    }
                }
                Declaration::TSEnumDeclaration(enumeration) => {
                    if let Some(item) = collect_enum(enumeration) {
                        self.items.push(ExportedItem::Enum(item));
                    }
                }
                _ => {}
            }
        }

        // Handle re-exports: `export { Foo } from './module'`
        // These are resolved through the module graph and handled separately
    }

    fn visit_export_default_declaration(&mut self, decl: &ExportDefaultDeclaration<'a>) {
        match &decl.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                if let Some(mut item) = collect_function(func) {
                    if item.name.is_empty() {
                        item.name = "default".to_string();
                    }
                    self.items.push(ExportedItem::Function(item));
                }
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                if let Some(mut item) = collect_class(class) {
                    if item.name.is_empty() {
                        item.name = "default".to_string();
                    }
                    self.items.push(ExportedItem::Class(item));
                }
            }
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(iface) => {
                if let Some(mut item) = collect_interface(iface) {
                    if item.name.is_empty() {
                        item.name = "default".to_string();
                    }
                    self.items.push(ExportedItem::Interface(item));
                }
            }
            _ => {}
        }
    }
}

fn collect_function(func: &Function<'_>) -> Option<ExportedFunction> {
    let name = func
        .id
        .as_ref()
        .map(|id| id.name.to_string())
        .unwrap_or_default();

    let params = collect_params(&func.params);

    let return_type = func
        .return_type
        .as_ref()
        .map(|rt| extract_type_info(&rt.type_annotation));

    Some(ExportedFunction {
        name,
        params,
        return_type,
        is_async: func.r#async,
    })
}

fn collect_class(class: &Class<'_>) -> Option<ExportedClass> {
    let name = class
        .id
        .as_ref()
        .map(|id| id.name.to_string())
        .unwrap_or_default();

    let mut constructor_params = None;
    let mut methods = Vec::new();
    let mut static_methods = Vec::new();
    let mut properties = Vec::new();

    // Collect decorators
    let decorators: Vec<String> = class
        .decorators
        .iter()
        .filter_map(|d| extract_decorator_name(&d.expression))
        .collect();

    for element in &class.body.body {
        match element {
            oxc_ast::ast::ClassElement::MethodDefinition(method) => match method.kind {
                MethodDefinitionKind::Constructor => {
                    constructor_params = Some(collect_method_params(method));
                }
                MethodDefinitionKind::Method
                | MethodDefinitionKind::Get
                | MethodDefinitionKind::Set => {
                    let info = collect_method_info(method);
                    if method.r#static {
                        static_methods.push(info);
                    } else {
                        methods.push(info);
                    }
                }
            },
            oxc_ast::ast::ClassElement::PropertyDefinition(prop) => {
                if let Some(info) = collect_property_from_definition(prop) {
                    properties.push(info);
                }
            }
            _ => {}
        }
    }

    Some(ExportedClass {
        name,
        constructor_params,
        methods,
        static_methods,
        properties,
        decorators,
    })
}

fn collect_interface(iface: &TSInterfaceDeclaration<'_>) -> Option<ExportedInterface> {
    let name = iface.id.name.to_string();
    let mut properties = Vec::new();

    for member in &iface.body.body {
        if let oxc_ast::ast::TSSignature::TSPropertySignature(prop) = member
            && let oxc_ast::ast::PropertyKey::StaticIdentifier(ident) = &prop.key
        {
            let type_info = prop
                .type_annotation
                .as_ref()
                .map(|ta| extract_type_info(&ta.type_annotation))
                .unwrap_or(TypeInfo::Unknown("missing type annotation".into()));

            properties.push(PropertyInfo {
                name: ident.name.to_string(),
                type_info,
                optional: prop.optional,
            });
        }
    }

    Some(ExportedInterface { name, properties })
}

fn collect_type_alias(alias: &TSTypeAliasDeclaration<'_>) -> Option<ExportedTypeAlias> {
    let name = alias.id.name.to_string();

    // Skip branded type aliases (they're handled by the branded type collector)
    if is_branded_type_name(&name) {
        return None;
    }

    let type_info = extract_type_info(&alias.type_annotation);

    Some(ExportedTypeAlias { name, type_info })
}

fn collect_enum(enumeration: &TSEnumDeclaration<'_>) -> Option<ExportedEnum> {
    let name = enumeration.id.name.to_string();

    let members: Vec<EnumMemberInfo> = enumeration
        .body
        .members
        .iter()
        .map(|m| {
            let member_name = match &m.id {
                oxc_ast::ast::TSEnumMemberName::Identifier(id) => id.name.to_string(),
                oxc_ast::ast::TSEnumMemberName::String(s) => s.value.to_string(),
                oxc_ast::ast::TSEnumMemberName::ComputedString(s) => s.value.to_string(),
                oxc_ast::ast::TSEnumMemberName::ComputedTemplateString(_) => {
                    "<computed>".to_string()
                }
            };
            EnumMemberInfo { name: member_name }
        })
        .collect();

    Some(ExportedEnum {
        name,
        members,
        has_flags_decorator: false, // Decorator detection handled separately
    })
}

fn collect_params(params: &oxc_ast::ast::FormalParameters<'_>) -> Vec<ParamInfo> {
    params.items.iter().map(collect_param).collect()
}

fn collect_param(param: &FormalParameter<'_>) -> ParamInfo {
    let name = extract_pattern_name(&param.pattern);
    let type_info = param
        .type_annotation
        .as_ref()
        .map(|ta| extract_type_info(&ta.type_annotation))
        .unwrap_or(TypeInfo::Unknown("missing type annotation".into()));

    ParamInfo {
        name,
        type_info,
        optional: param.optional,
    }
}

fn collect_method_params(method: &MethodDefinition<'_>) -> Vec<ParamInfo> {
    method
        .value
        .params
        .items
        .iter()
        .map(collect_param)
        .collect()
}

fn collect_method_info(method: &MethodDefinition<'_>) -> MethodInfo {
    let name = method.key.name().map(|n| n.to_string()).unwrap_or_default();

    let params = collect_method_params(method);

    let return_type = method
        .value
        .return_type
        .as_ref()
        .map(|rt| extract_type_info(&rt.type_annotation));

    MethodInfo {
        name,
        params,
        return_type,
        is_async: method.value.r#async,
    }
}

fn collect_property_from_definition(prop: &PropertyDefinition<'_>) -> Option<PropertyInfo> {
    let name = prop.key.name()?.to_string();

    let type_info = prop
        .type_annotation
        .as_ref()
        .map(|ta| extract_type_info(&ta.type_annotation))
        .unwrap_or(TypeInfo::Unknown("missing type annotation".into()));

    Some(PropertyInfo {
        name,
        type_info,
        optional: prop.optional,
    })
}

/// Extract a type info from an OXC TSType AST node.
pub fn extract_type_info(ts_type: &TSType<'_>) -> TypeInfo {
    match ts_type {
        TSType::TSNumberKeyword(_) => TypeInfo::Keyword(KeywordType::Number),
        TSType::TSStringKeyword(_) => TypeInfo::Keyword(KeywordType::String),
        TSType::TSBooleanKeyword(_) => TypeInfo::Keyword(KeywordType::Boolean),
        TSType::TSBigIntKeyword(_) => TypeInfo::Keyword(KeywordType::BigInt),
        TSType::TSVoidKeyword(_) => TypeInfo::Keyword(KeywordType::Void),
        TSType::TSAnyKeyword(_) => TypeInfo::Keyword(KeywordType::Any),
        TSType::TSUnknownKeyword(_) => TypeInfo::Keyword(KeywordType::Unknown),
        TSType::TSNeverKeyword(_) => TypeInfo::Keyword(KeywordType::Never),
        TSType::TSNullKeyword(_) => TypeInfo::Null,
        TSType::TSUndefinedKeyword(_) => TypeInfo::Undefined,
        TSType::TSSymbolKeyword(_) => TypeInfo::Keyword(KeywordType::Symbol),
        TSType::TSObjectKeyword(_) => TypeInfo::Keyword(KeywordType::Object),

        TSType::TSArrayType(arr) => {
            let elem = extract_type_info(&arr.element_type);
            TypeInfo::Array(Box::new(elem))
        }

        TSType::TSTupleType(tuple) => {
            let elems: Vec<TypeInfo> = tuple
                .element_types
                .iter()
                .map(|e| extract_type_info_from_tuple_element(e))
                .collect();
            TypeInfo::Tuple(elems)
        }

        TSType::TSUnionType(union) => {
            let types: Vec<TypeInfo> = union.types.iter().map(|t| extract_type_info(t)).collect();
            TypeInfo::Union(types)
        }

        TSType::TSIntersectionType(_) => TypeInfo::Intersection,

        TSType::TSTypeReference(reference) => {
            let name = extract_type_name(&reference.type_name);
            let type_args = reference
                .type_arguments
                .as_ref()
                .map(|tp| tp.params.iter().map(|t| extract_type_info(t)).collect())
                .unwrap_or_default();
            TypeInfo::Reference { name, type_args }
        }

        TSType::TSTypeLiteral(literal) => {
            let mut props = Vec::new();
            for member in &literal.members {
                if let oxc_ast::ast::TSSignature::TSPropertySignature(prop) = member
                    && let oxc_ast::ast::PropertyKey::StaticIdentifier(ident) = &prop.key
                {
                    let type_info = prop
                        .type_annotation
                        .as_ref()
                        .map(|ta| extract_type_info(&ta.type_annotation))
                        .unwrap_or(TypeInfo::Unknown("missing type annotation".into()));

                    props.push(PropertyInfo {
                        name: ident.name.to_string(),
                        type_info,
                        optional: prop.optional,
                    });
                }
            }
            TypeInfo::ObjectLiteral(props)
        }

        TSType::TSFunctionType(_) => TypeInfo::FunctionType,

        TSType::TSParenthesizedType(paren) => extract_type_info(&paren.type_annotation),

        _ => TypeInfo::Unknown(format!("{ts_type:?}")),
    }
}

fn extract_type_info_from_tuple_element(elem: &oxc_ast::ast::TSTupleElement<'_>) -> TypeInfo {
    match elem {
        oxc_ast::ast::TSTupleElement::TSNamedTupleMember(named) => {
            extract_type_info_from_tuple_element(&named.element_type)
        }
        oxc_ast::ast::TSTupleElement::TSOptionalType(opt) => {
            let inner = extract_type_info(&opt.type_annotation);
            TypeInfo::Union(vec![inner, TypeInfo::Undefined])
        }
        oxc_ast::ast::TSTupleElement::TSRestType(rest) => extract_type_info(&rest.type_annotation),
        // All TSType variants are inherited
        other => {
            // TSTupleElement inherits all TSType variants, so we can
            // extract type info by matching on common patterns.
            extract_type_info_from_tuple_element_tstype(other)
        }
    }
}

/// Handle TSTupleElement variants that are inherited from TSType.
fn extract_type_info_from_tuple_element_tstype(
    elem: &oxc_ast::ast::TSTupleElement<'_>,
) -> TypeInfo {
    use oxc_ast::ast::TSTupleElement;
    match elem {
        TSTupleElement::TSNumberKeyword(_) => TypeInfo::Keyword(KeywordType::Number),
        TSTupleElement::TSStringKeyword(_) => TypeInfo::Keyword(KeywordType::String),
        TSTupleElement::TSBooleanKeyword(_) => TypeInfo::Keyword(KeywordType::Boolean),
        TSTupleElement::TSBigIntKeyword(_) => TypeInfo::Keyword(KeywordType::BigInt),
        TSTupleElement::TSVoidKeyword(_) => TypeInfo::Keyword(KeywordType::Void),
        TSTupleElement::TSAnyKeyword(_) => TypeInfo::Keyword(KeywordType::Any),
        TSTupleElement::TSUnknownKeyword(_) => TypeInfo::Keyword(KeywordType::Unknown),
        TSTupleElement::TSNeverKeyword(_) => TypeInfo::Keyword(KeywordType::Never),
        TSTupleElement::TSNullKeyword(_) => TypeInfo::Null,
        TSTupleElement::TSUndefinedKeyword(_) => TypeInfo::Undefined,
        TSTupleElement::TSTypeReference(reference) => {
            let name = extract_type_name(&reference.type_name);
            let type_args = reference
                .type_arguments
                .as_ref()
                .map(|tp| tp.params.iter().map(|t| extract_type_info(t)).collect())
                .unwrap_or_default();
            TypeInfo::Reference { name, type_args }
        }
        TSTupleElement::TSArrayType(arr) => {
            let inner = extract_type_info(&arr.element_type);
            TypeInfo::Array(Box::new(inner))
        }
        TSTupleElement::TSUnionType(union) => {
            let types: Vec<TypeInfo> = union.types.iter().map(|t| extract_type_info(t)).collect();
            TypeInfo::Union(types)
        }
        _ => TypeInfo::Unknown("unsupported tuple element type".into()),
    }
}

fn extract_type_name(name: &TSTypeName<'_>) -> String {
    match name {
        TSTypeName::IdentifierReference(ident) => ident.name.to_string(),
        TSTypeName::QualifiedName(qualified) => {
            let left = extract_type_name(&qualified.left);
            format!("{left}.{}", qualified.right.name)
        }
        _ => "this".to_string(),
    }
}

fn extract_pattern_name(pattern: &oxc_ast::ast::BindingPattern<'_>) -> String {
    match pattern {
        oxc_ast::ast::BindingPattern::BindingIdentifier(ident) => ident.name.to_string(),
        _ => "_".to_string(),
    }
}

/// Extract a decorator name from a decorator expression.
/// Handles `@foo`, `@foo.bar`, `@foo()`, `@foo.bar()`.
fn extract_decorator_name(expr: &oxc_ast::ast::Expression<'_>) -> Option<String> {
    match expr {
        oxc_ast::ast::Expression::Identifier(ident) => Some(ident.name.to_string()),
        oxc_ast::ast::Expression::StaticMemberExpression(member) => {
            let obj = extract_decorator_name(&member.object)?;
            Some(format!("{obj}.{}", member.property.name))
        }
        oxc_ast::ast::Expression::CallExpression(call) => extract_decorator_name(&call.callee),
        _ => None,
    }
}

/// Check if a name is a well-known branded type name.
fn is_branded_type_name(name: &str) -> bool {
    matches!(
        name,
        "u8" | "u16" | "u32" | "u64" | "s8" | "s16" | "s32" | "s64" | "f32" | "f64" | "char"
    )
}
