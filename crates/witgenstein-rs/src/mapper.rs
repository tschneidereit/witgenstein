// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Map resolved Rust type information into `wit-encoder` types.

use wit_common::to_kebab_case;
use wit_encoder::{
    EnumCase, Field, Ident, Record, Resource, ResourceFunc, StandaloneFunc, Type, TypeDef,
    TypeDefKind, VariantCase,
};

use crate::resolve::{
    ResolvedEnum, ResolvedFunction, ResolvedImpl, ResolvedRecord, ResolvedType, ResolvedTypeDef,
    ResolvedVariant,
};

/// Map a `ResolvedType` to a `wit-encoder` `Type`.
pub fn map_type(resolved: &ResolvedType) -> Type {
    match resolved {
        ResolvedType::Bool => Type::Bool,
        ResolvedType::U8 => Type::U8,
        ResolvedType::U16 => Type::U16,
        ResolvedType::U32 => Type::U32,
        ResolvedType::U64 => Type::U64,
        ResolvedType::S8 => Type::S8,
        ResolvedType::S16 => Type::S16,
        ResolvedType::S32 => Type::S32,
        ResolvedType::S64 => Type::S64,
        ResolvedType::F32 => Type::F32,
        ResolvedType::F64 => Type::F64,
        ResolvedType::Char => Type::Char,
        ResolvedType::WitString => Type::String,
        ResolvedType::List(inner) => Type::list(map_type(inner)),
        ResolvedType::Option(inner) => Type::option(map_type(inner)),
        ResolvedType::Result { ok, err } => {
            let ok_type = ok.as_ref().map(|t| map_type(t));
            let err_type = err.as_ref().map(|t| map_type(t));
            match (ok_type, err_type) {
                (Some(ok), Some(err)) => Type::result_both(ok, err),
                (Some(ok), None) => Type::result_ok(ok),
                (None, Some(err)) => Type::result_err(err),
                (None, None) => Type::result_empty(),
            }
        }
        ResolvedType::Tuple(elems) => {
            if elems.is_empty() {
                Type::Bool // fallback for unit tuple
            } else {
                Type::tuple(elems.iter().map(map_type).collect::<Vec<_>>())
            }
        }
        ResolvedType::Named(name) => Type::named(to_kebab_case(name)),
        ResolvedType::Own(name) => {
            // WIT handles owned resources as just the named type.
            Type::named(to_kebab_case(name))
        }
        ResolvedType::Borrow(name) => Type::borrow(to_kebab_case(name)),
    }
}

/// Map a `ResolvedFunction` to a `wit-encoder` `StandaloneFunc`.
pub fn map_function(func: &ResolvedFunction) -> StandaloneFunc {
    let mut wit_func = StandaloneFunc::new(Ident::new(to_kebab_case(&func.name)), func.is_async);

    for (name, ty) in &func.params {
        wit_func
            .params_mut()
            .push(to_kebab_case(name), map_type(ty));
    }

    if let Some(ret) = &func.return_type
        && !matches!(ret, ResolvedType::Tuple(elems) if elems.is_empty())
    {
        wit_func.set_result(Some(map_type(ret)));
    }

    wit_func
}

/// Map a `ResolvedImpl` to a `wit-encoder` `Resource`.
pub fn map_resource(imp: &ResolvedImpl) -> Resource {
    let mut resource = Resource::empty();

    // Constructor.
    if let Some(ctor) = &imp.constructor {
        let mut func = ResourceFunc::constructor();
        for (name, ty) in &ctor.params {
            func.params_mut().push(to_kebab_case(name), map_type(ty));
        }
        resource.func(func);
    }

    // Methods.
    for method in &imp.methods {
        let mut func =
            ResourceFunc::method(Ident::new(to_kebab_case(&method.name)), method.is_async);
        for (name, ty) in &method.params {
            func.params_mut().push(to_kebab_case(name), map_type(ty));
        }
        if let Some(ret) = &method.return_type
            && !matches!(ret, ResolvedType::Tuple(elems) if elems.is_empty())
        {
            func.set_result(Some(map_type(ret)));
        }
        resource.func(func);
    }

    // Static functions.
    for static_fn in &imp.static_funcs {
        let mut func = ResourceFunc::static_(
            Ident::new(to_kebab_case(&static_fn.name)),
            static_fn.is_async,
        );
        for (name, ty) in &static_fn.params {
            func.params_mut().push(to_kebab_case(name), map_type(ty));
        }
        if let Some(ret) = &static_fn.return_type
            && !matches!(ret, ResolvedType::Tuple(elems) if elems.is_empty())
        {
            func.set_result(Some(map_type(ret)));
        }
        resource.func(func);
    }

    resource
}

/// Map a `ResolvedTypeDef` to a `wit-encoder` `TypeDef`.
pub fn map_type_def(resolved: &ResolvedTypeDef) -> TypeDef {
    match resolved {
        ResolvedTypeDef::Record(record) => map_record(record),
        ResolvedTypeDef::Enum(e) => map_enum(e),
        ResolvedTypeDef::Variant(v) => map_variant(v),
    }
}

fn map_record(record: &ResolvedRecord) -> TypeDef {
    let fields: Vec<Field> = record
        .fields
        .iter()
        .map(|(name, ty)| Field::new(to_kebab_case(name), map_type(ty)))
        .collect();
    TypeDef::new(
        to_kebab_case(&record.name),
        TypeDefKind::Record(Record::new(fields)),
    )
}

fn map_enum(e: &ResolvedEnum) -> TypeDef {
    let cases: Vec<EnumCase> = e
        .cases
        .iter()
        .map(|case| EnumCase::new(to_kebab_case(case)))
        .collect();
    let mut wit_enum = wit_encoder::Enum::empty();
    for case in cases {
        wit_enum.case(case);
    }
    TypeDef::new(to_kebab_case(&e.name), TypeDefKind::Enum(wit_enum))
}

fn map_variant(v: &ResolvedVariant) -> TypeDef {
    let mut wit_variant = wit_encoder::Variant::empty();
    for (case_name, payload) in &v.cases {
        let case = if let Some(ty) = payload {
            VariantCase::value(to_kebab_case(case_name), map_type(ty))
        } else {
            VariantCase::empty(to_kebab_case(case_name))
        };
        wit_variant.case(case);
    }
    TypeDef::new(to_kebab_case(&v.name), TypeDefKind::Variant(wit_variant))
}
