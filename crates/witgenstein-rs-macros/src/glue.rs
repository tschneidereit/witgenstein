// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Generate Rust glue code: `wit_bindgen::generate!` invocation, `Guest` trait
//! implementations, and type conversion helpers.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;

use crate::extract::{
    ExportedFunction, ExportedImpl, ResolvedType, ResolvedTypeDef, TypeDiscovery,
};
use crate::wit::to_kebab_case;

/// Generate all glue code tokens.
pub fn generate_glue(
    discovery: &TypeDiscovery,
    wit_content: &str,
    package: &str,
    interface: &str,
) -> TokenStream {
    let bindings_mod = generate_bindings_mod(wit_content);

    // Derive the iface_path from the package and interface names.
    // WIT package "ns:pkg" → bindings::exports::ns::pkg::{interface}
    // WIT package "ns:pkg@ver" → bindings::exports::ns::pkg::{interface} (version stripped)
    let iface_path = build_iface_path(package, interface);

    // Conversion helpers for named types.
    let named_types = collect_named_types(discovery);
    let conversions = generate_type_conversions(discovery, &iface_path, &named_types);

    // Resource wrappers.
    let resource_wrappers = generate_resource_wrappers(discovery);

    // Component struct + Guest impl.
    let guest_impl = generate_guest_impl(discovery, &iface_path);

    // Resource Guest impls.
    let resource_impls = generate_resource_impls(discovery, &iface_path);

    // Export macro invocation.
    let export_call = quote! {
        bindings::export!(__WitgensteinComponent with_types_in bindings);
    };

    quote! {
        #bindings_mod
        #conversions
        #resource_wrappers

        struct __WitgensteinComponent;

        #guest_impl
        #resource_impls
        #export_call
    }
}

fn generate_bindings_mod(wit_content: &str) -> TokenStream {
    quote! {
        #[allow(warnings)]
        mod bindings {
            wit_bindgen::generate!({
                world: "component-world",
                inline: #wit_content,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Named type collection
// ---------------------------------------------------------------------------

fn collect_named_types(discovery: &TypeDiscovery) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut visit = |ty: &ResolvedType| collect_named_in(ty, &mut names);
    for func in &discovery.functions {
        for (_, ty) in &func.params {
            visit(ty);
        }
        if let Some(ret) = &func.return_type {
            visit(ret);
        }
    }
    for imp in &discovery.impls {
        if let Some(ctor) = &imp.constructor {
            for (_, ty) in &ctor.params {
                visit(ty);
            }
        }
        for m in imp.methods.iter().chain(imp.static_funcs.iter()) {
            for (_, ty) in &m.params {
                visit(ty);
            }
            if let Some(ret) = &m.return_type {
                visit(ret);
            }
        }
    }

    // Transitively discover named types referenced inside record/variant fields.
    let mut changed = true;
    while changed {
        changed = false;
        for td in &discovery.type_defs {
            let type_name = match td {
                ResolvedTypeDef::Record(r) => &r.name,
                ResolvedTypeDef::Enum(_) => continue,
                ResolvedTypeDef::Variant(v) => &v.name,
            };
            if !names.contains(type_name) {
                continue;
            }
            match td {
                ResolvedTypeDef::Record(r) => {
                    for (_, ty) in &r.fields {
                        let before = names.len();
                        collect_named_in(ty, &mut names);
                        if names.len() > before {
                            changed = true;
                        }
                    }
                }
                ResolvedTypeDef::Variant(v) => {
                    for (_, payload) in &v.cases {
                        if let Some(ty) = payload {
                            let before = names.len();
                            collect_named_in(ty, &mut names);
                            if names.len() > before {
                                changed = true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    names
}

fn collect_named_in(ty: &ResolvedType, names: &mut HashSet<String>) {
    match ty {
        ResolvedType::Named(n) => {
            names.insert(n.clone());
        }
        ResolvedType::List(inner)
        | ResolvedType::Option(inner)
        | ResolvedType::Future(Some(inner))
        | ResolvedType::Stream(Some(inner)) => collect_named_in(inner, names),
        ResolvedType::Result { ok, err } => {
            if let Some(t) = ok {
                collect_named_in(t, names);
            }
            if let Some(t) = err {
                collect_named_in(t, names);
            }
        }
        ResolvedType::Tuple(elems) => {
            for e in elems {
                collect_named_in(e, names);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Type conversions
// ---------------------------------------------------------------------------

fn generate_type_conversions(
    discovery: &TypeDiscovery,
    iface_path: &TokenStream,
    named_types: &HashSet<String>,
) -> TokenStream {
    let mut conversions = Vec::new();

    for td in &discovery.type_defs {
        let type_name = match td {
            ResolvedTypeDef::Record(r) => &r.name,
            ResolvedTypeDef::Enum(e) => &e.name,
            ResolvedTypeDef::Variant(v) => &v.name,
        };
        if !named_types.contains(type_name) {
            continue;
        }
        match td {
            ResolvedTypeDef::Record(r) => {
                conversions.push(gen_record_conversions(r, iface_path));
            }
            ResolvedTypeDef::Enum(e) => {
                conversions.push(gen_enum_conversions(e, iface_path));
            }
            ResolvedTypeDef::Variant(v) => {
                conversions.push(gen_variant_conversions(v, iface_path));
            }
        }
    }

    quote! { #(#conversions)* }
}

fn bindings_type_ident(type_name: &str, iface_path: &TokenStream) -> TokenStream {
    let kebab = to_kebab_case(type_name);
    let pascal = kebab
        .split('-')
        .map(|seg| {
            let mut c = seg.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<String>();
    let ident = format_ident!("{}", pascal);
    quote! { #iface_path::#ident }
}

fn to_user_fn_name(type_name: &str) -> proc_macro2::Ident {
    let kebab = to_kebab_case(type_name);
    format_ident!("__cvt_{}_to_user", kebab.replace('-', "_"))
}

fn from_user_fn_name(type_name: &str) -> proc_macro2::Ident {
    let kebab = to_kebab_case(type_name);
    format_ident!("__cvt_{}_from_user", kebab.replace('-', "_"))
}

fn gen_record_conversions(
    r: &crate::extract::ResolvedRecord,
    iface_path: &TokenStream,
) -> TokenStream {
    let b_ty = bindings_type_ident(&r.name, iface_path);
    let u_ty = format_ident!("{}", &r.name);
    let to_user = to_user_fn_name(&r.name);
    let from_user = from_user_fn_name(&r.name);

    let to_user_fields: Vec<TokenStream> = r
        .fields
        .iter()
        .map(|(name, ty)| {
            let field_ident = format_ident!("{}", name);
            let bindings_field = format_ident!("{}", to_kebab_case(name).replace('-', "_"));
            let conversion = wrap_to_user_expr(quote! { v.#bindings_field }, ty);
            quote! { #field_ident: #conversion }
        })
        .collect();

    let from_user_fields: Vec<TokenStream> = r
        .fields
        .iter()
        .map(|(name, ty)| {
            let field_ident = format_ident!("{}", name);
            let bindings_field = format_ident!("{}", to_kebab_case(name).replace('-', "_"));
            let conversion = wrap_from_user_expr(quote! { v.#field_ident }, ty);
            quote! { #bindings_field: #conversion }
        })
        .collect();

    quote! {
        fn #to_user(v: #b_ty) -> #u_ty {
            #u_ty { #(#to_user_fields),* }
        }
        fn #from_user(v: #u_ty) -> #b_ty {
            #b_ty { #(#from_user_fields),* }
        }
    }
}

fn gen_enum_conversions(e: &crate::extract::ResolvedEnum, iface_path: &TokenStream) -> TokenStream {
    let b_ty = bindings_type_ident(&e.name, iface_path);
    let u_ty = format_ident!("{}", &e.name);
    let to_user = to_user_fn_name(&e.name);
    let from_user = from_user_fn_name(&e.name);

    let to_user_arms: Vec<TokenStream> = e
        .cases
        .iter()
        .map(|case| {
            let user_ident = format_ident!("{}", case);
            let kebab = to_kebab_case(case);
            let bindings_ident = format_ident!("{}", pascal_case(&kebab));
            quote! { #b_ty::#bindings_ident => #u_ty::#user_ident }
        })
        .collect();

    let from_user_arms: Vec<TokenStream> = e
        .cases
        .iter()
        .map(|case| {
            let user_ident = format_ident!("{}", case);
            let kebab = to_kebab_case(case);
            let bindings_ident = format_ident!("{}", pascal_case(&kebab));
            quote! { #u_ty::#user_ident => #b_ty::#bindings_ident }
        })
        .collect();

    quote! {
        fn #to_user(v: #b_ty) -> #u_ty {
            match v { #(#to_user_arms),* }
        }
        fn #from_user(v: #u_ty) -> #b_ty {
            match v { #(#from_user_arms),* }
        }
    }
}

fn gen_variant_conversions(
    v: &crate::extract::ResolvedVariant,
    iface_path: &TokenStream,
) -> TokenStream {
    let b_ty = bindings_type_ident(&v.name, iface_path);
    let u_ty = format_ident!("{}", &v.name);
    let to_user = to_user_fn_name(&v.name);
    let from_user = from_user_fn_name(&v.name);

    let to_user_arms: Vec<TokenStream> = v
        .cases
        .iter()
        .map(|(case_name, payload)| {
            let user_ident = format_ident!("{}", case_name);
            let kebab = to_kebab_case(case_name);
            let bindings_ident = format_ident!("{}", pascal_case(&kebab));
            if payload.is_some() {
                let inner_conv = wrap_to_user_expr(quote! { inner }, payload.as_ref().unwrap());
                quote! { #b_ty::#bindings_ident(inner) => #u_ty::#user_ident(#inner_conv) }
            } else {
                quote! { #b_ty::#bindings_ident => #u_ty::#user_ident }
            }
        })
        .collect();

    let from_user_arms: Vec<TokenStream> = v
        .cases
        .iter()
        .map(|(case_name, payload)| {
            let user_ident = format_ident!("{}", case_name);
            let kebab = to_kebab_case(case_name);
            let bindings_ident = format_ident!("{}", pascal_case(&kebab));
            if payload.is_some() {
                let inner_conv = wrap_from_user_expr(quote! { inner }, payload.as_ref().unwrap());
                quote! { #u_ty::#user_ident(inner) => #b_ty::#bindings_ident(#inner_conv) }
            } else {
                quote! { #u_ty::#user_ident => #b_ty::#bindings_ident }
            }
        })
        .collect();

    quote! {
        fn #to_user(v: #b_ty) -> #u_ty {
            match v { #(#to_user_arms),* }
        }
        fn #from_user(v: #u_ty) -> #b_ty {
            match v { #(#from_user_arms),* }
        }
    }
}

// ---------------------------------------------------------------------------
// Conversion expression wrapping
// ---------------------------------------------------------------------------

fn wrap_to_user_expr(expr: TokenStream, ty: &ResolvedType) -> TokenStream {
    match ty {
        ResolvedType::Named(name) => {
            let fn_name = to_user_fn_name(name);
            quote! { #fn_name(#expr) }
        }
        ResolvedType::Option(inner) if contains_named(inner) => {
            let inner_conv = wrap_to_user_expr(quote! { v }, inner);
            quote! { #expr.map(|v| #inner_conv) }
        }
        ResolvedType::List(inner) if contains_named(inner) => {
            let inner_conv = wrap_to_user_expr(quote! { v }, inner);
            quote! { #expr.into_iter().map(|v| #inner_conv).collect() }
        }
        ResolvedType::Result { ok, err } if result_contains_named(ok, err) => {
            let ok_conv = ok
                .as_ref()
                .map(|t| wrap_to_user_expr(quote! { v }, t))
                .unwrap_or_else(|| quote! { v });
            let err_conv = err
                .as_ref()
                .map(|t| wrap_to_user_expr(quote! { e }, t))
                .unwrap_or_else(|| quote! { e });
            quote! { #expr.map(|v| #ok_conv).map_err(|e| #err_conv) }
        }
        _ => expr,
    }
}

fn wrap_from_user_expr(expr: TokenStream, ty: &ResolvedType) -> TokenStream {
    match ty {
        ResolvedType::Named(name) => {
            let fn_name = from_user_fn_name(name);
            quote! { #fn_name(#expr) }
        }
        ResolvedType::Option(inner) if contains_named(inner) => {
            let inner_conv = wrap_from_user_expr(quote! { v }, inner);
            quote! { #expr.map(|v| #inner_conv) }
        }
        ResolvedType::List(inner) if contains_named(inner) => {
            let inner_conv = wrap_from_user_expr(quote! { v }, inner);
            quote! { #expr.into_iter().map(|v| #inner_conv).collect() }
        }
        ResolvedType::Result { ok, err } if result_contains_named(ok, err) => {
            let ok_conv = ok
                .as_ref()
                .map(|t| wrap_from_user_expr(quote! { v }, t))
                .unwrap_or_else(|| quote! { v });
            let err_conv = err
                .as_ref()
                .map(|t| wrap_from_user_expr(quote! { e }, t))
                .unwrap_or_else(|| quote! { e });
            quote! { #expr.map(|v| #ok_conv).map_err(|e| #err_conv) }
        }
        _ => expr,
    }
}

fn contains_named(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Named(_) => true,
        ResolvedType::List(inner)
        | ResolvedType::Option(inner)
        | ResolvedType::Future(Some(inner))
        | ResolvedType::Stream(Some(inner)) => contains_named(inner),
        ResolvedType::Result { ok, err } => {
            ok.as_ref().is_some_and(|t| contains_named(t))
                || err.as_ref().is_some_and(|t| contains_named(t))
        }
        ResolvedType::Tuple(elems) => elems.iter().any(contains_named),
        _ => false,
    }
}

fn result_contains_named(ok: &Option<Box<ResolvedType>>, err: &Option<Box<ResolvedType>>) -> bool {
    ok.as_ref().is_some_and(|t| contains_named(t))
        || err.as_ref().is_some_and(|t| contains_named(t))
}

// ---------------------------------------------------------------------------
// Resource wrappers
// ---------------------------------------------------------------------------

fn generate_resource_wrappers(discovery: &TypeDiscovery) -> TokenStream {
    let wrappers: Vec<TokenStream> = discovery
        .impls
        .iter()
        .map(|imp| {
            let wrapper = format_ident!("__{}Wrapper", &imp.type_name);
            let inner = format_ident!("{}", &imp.type_name);
            quote! {
                struct #wrapper(std::cell::RefCell<#inner>);
            }
        })
        .collect();
    quote! { #(#wrappers)* }
}

// ---------------------------------------------------------------------------
// Guest trait implementations
// ---------------------------------------------------------------------------

fn generate_guest_impl(discovery: &TypeDiscovery, iface_path: &TokenStream) -> TokenStream {
    let resource_types: Vec<TokenStream> = discovery
        .impls
        .iter()
        .map(|imp| {
            let type_ident = format_ident!("{}", &imp.type_name);
            let wrapper = format_ident!("__{}Wrapper", &imp.type_name);
            quote! { type #type_ident = #wrapper; }
        })
        .collect();

    let fn_impls: Vec<TokenStream> = discovery
        .functions
        .iter()
        .map(|func| gen_fn_delegation(func, iface_path))
        .collect();

    quote! {
        impl #iface_path::Guest for __WitgensteinComponent {
            #(#resource_types)*
            #(#fn_impls)*
        }
    }
}

fn generate_resource_impls(discovery: &TypeDiscovery, iface_path: &TokenStream) -> TokenStream {
    let impls: Vec<TokenStream> = discovery
        .impls
        .iter()
        .map(|imp| gen_resource_impl(imp, iface_path))
        .collect();
    quote! { #(#impls)* }
}

fn gen_resource_impl(imp: &ExportedImpl, iface_path: &TokenStream) -> TokenStream {
    let wrapper = format_ident!("__{}Wrapper", &imp.type_name);
    let guest_trait = format_ident!("Guest{}", &imp.type_name);

    let ctor = imp
        .constructor
        .as_ref()
        .map(|ctor| gen_ctor_delegation(ctor, &imp.type_name, iface_path));

    let methods: Vec<TokenStream> = imp
        .methods
        .iter()
        .map(|m| gen_method_delegation(m, &imp.type_name, iface_path))
        .collect();

    let statics: Vec<TokenStream> = imp
        .static_funcs
        .iter()
        .map(|f| gen_static_delegation(f, &imp.type_name, iface_path))
        .collect();

    quote! {
        impl #iface_path::#guest_trait for #wrapper {
            #ctor
            #(#methods)*
            #(#statics)*
        }
    }
}

// ---------------------------------------------------------------------------
// Function delegation generators
// ---------------------------------------------------------------------------

fn gen_fn_delegation(func: &ExportedFunction, iface_path: &TokenStream) -> TokenStream {
    let wit_name = format_ident!("{}", to_kebab_case(&func.name).replace('-', "_"));
    let user_fn = format_ident!("{}", &func.name);

    let param_defs: Vec<TokenStream> = func
        .params
        .iter()
        .map(|(name, ty)| {
            let ident = format_ident!("{}", to_kebab_case(name).replace('-', "_"));
            let rust_ty = type_as_bindings(ty, iface_path);
            quote! { #ident: #rust_ty }
        })
        .collect();

    let ret_ty = return_type_bindings(&func.return_type, iface_path);

    let args: Vec<TokenStream> = func
        .params
        .iter()
        .map(|(name, ty)| {
            let ident = format_ident!("{}", to_kebab_case(name).replace('-', "_"));
            wrap_to_user_expr(quote! { #ident }, ty)
        })
        .collect();

    let call = quote! { #user_fn(#(#args),*) };
    let call = if func.is_async {
        quote! { #call.await }
    } else {
        call
    };
    let body = wrap_return_from_user(call, &func.return_type);

    let async_kw = if func.is_async {
        quote! { async }
    } else {
        quote! {}
    };

    quote! {
        #async_kw fn #wit_name(#(#param_defs),*) #ret_ty {
            #body
        }
    }
}

fn gen_ctor_delegation(
    func: &ExportedFunction,
    type_name: &str,
    iface_path: &TokenStream,
) -> TokenStream {
    let type_ident = format_ident!("{}", type_name);
    let wrapper = format_ident!("__{}Wrapper", type_name);

    let param_defs: Vec<TokenStream> = func
        .params
        .iter()
        .map(|(name, ty)| {
            let ident = format_ident!("{}", to_kebab_case(name).replace('-', "_"));
            let rust_ty = type_as_bindings(ty, iface_path);
            quote! { #ident: #rust_ty }
        })
        .collect();

    let args: Vec<TokenStream> = func
        .params
        .iter()
        .map(|(name, ty)| {
            let ident = format_ident!("{}", to_kebab_case(name).replace('-', "_"));
            wrap_to_user_expr(quote! { #ident }, ty)
        })
        .collect();

    quote! {
        fn new(#(#param_defs),*) -> Self {
            #wrapper(std::cell::RefCell::new(#type_ident::new(#(#args),*)))
        }
    }
}

fn gen_method_delegation(
    func: &ExportedFunction,
    type_name: &str,
    iface_path: &TokenStream,
) -> TokenStream {
    let wit_name = format_ident!("{}", to_kebab_case(&func.name).replace('-', "_"));
    let user_fn = format_ident!("{}", &func.name);
    let type_ident = format_ident!("{}", type_name);

    let param_defs: Vec<TokenStream> = func
        .params
        .iter()
        .map(|(name, ty)| {
            let ident = format_ident!("{}", to_kebab_case(name).replace('-', "_"));
            let rust_ty = type_as_bindings(ty, iface_path);
            quote! { #ident: #rust_ty }
        })
        .collect();

    let ret_ty = return_type_bindings(&func.return_type, iface_path);

    let args: Vec<TokenStream> = func
        .params
        .iter()
        .map(|(name, ty)| {
            let ident = format_ident!("{}", to_kebab_case(name).replace('-', "_"));
            wrap_to_user_expr(quote! { #ident }, ty)
        })
        .collect();

    let borrow = if func.is_mut_self {
        quote! { &mut *self.0.borrow_mut() }
    } else {
        quote! { &*self.0.borrow() }
    };

    let comma = if args.is_empty() {
        quote! {}
    } else {
        quote! { , }
    };

    let call = quote! { #type_ident::#user_fn(#borrow #comma #(#args),*) };
    let call = if func.is_async {
        quote! { #call.await }
    } else {
        call
    };
    let body = wrap_return_from_user(call, &func.return_type);

    let async_kw = if func.is_async {
        quote! { async }
    } else {
        quote! {}
    };

    quote! {
        #async_kw fn #wit_name(&self #(, #param_defs)*) #ret_ty {
            #body
        }
    }
}

fn gen_static_delegation(
    func: &ExportedFunction,
    type_name: &str,
    iface_path: &TokenStream,
) -> TokenStream {
    let wit_name = format_ident!("{}", to_kebab_case(&func.name).replace('-', "_"));
    let user_fn = format_ident!("{}", &func.name);
    let type_ident = format_ident!("{}", type_name);

    let param_defs: Vec<TokenStream> = func
        .params
        .iter()
        .map(|(name, ty)| {
            let ident = format_ident!("{}", to_kebab_case(name).replace('-', "_"));
            let rust_ty = type_as_bindings(ty, iface_path);
            quote! { #ident: #rust_ty }
        })
        .collect();

    let ret_ty = return_type_bindings(&func.return_type, iface_path);

    let args: Vec<TokenStream> = func
        .params
        .iter()
        .map(|(name, ty)| {
            let ident = format_ident!("{}", to_kebab_case(name).replace('-', "_"));
            wrap_to_user_expr(quote! { #ident }, ty)
        })
        .collect();

    let call = quote! { #type_ident::#user_fn(#(#args),*) };
    let call = if func.is_async {
        quote! { #call.await }
    } else {
        call
    };
    let body = wrap_return_from_user(call, &func.return_type);

    let async_kw = if func.is_async {
        quote! { async }
    } else {
        quote! {}
    };

    quote! {
        #async_kw fn #wit_name(#(#param_defs),*) #ret_ty {
            #body
        }
    }
}

fn wrap_return_from_user(call: TokenStream, return_type: &Option<ResolvedType>) -> TokenStream {
    match return_type {
        Some(ty) if contains_named(ty) => wrap_from_user_expr(call, ty),
        Some(ResolvedType::Stream(inner)) => {
            let inner_ty = inner
                .as_ref()
                .map(|t| type_as_bindings(t, &quote! {}))
                .unwrap_or_else(|| quote! { () });
            quote! {{
                let _ = #call;
                let (_writer, reader) = bindings::wit_stream::new::<#inner_ty>();
                reader
            }}
        }
        _ => call,
    }
}

// ---------------------------------------------------------------------------
// Type rendering — bindings types
// ---------------------------------------------------------------------------

fn type_as_bindings(ty: &ResolvedType, iface_path: &TokenStream) -> TokenStream {
    match ty {
        ResolvedType::Bool => quote! { bool },
        ResolvedType::U8 => quote! { u8 },
        ResolvedType::U16 => quote! { u16 },
        ResolvedType::U32 => quote! { u32 },
        ResolvedType::U64 => quote! { u64 },
        ResolvedType::S8 => quote! { i8 },
        ResolvedType::S16 => quote! { i16 },
        ResolvedType::S32 => quote! { i32 },
        ResolvedType::S64 => quote! { i64 },
        ResolvedType::F32 => quote! { f32 },
        ResolvedType::F64 => quote! { f64 },
        ResolvedType::Char => quote! { char },
        ResolvedType::WitString => quote! { String },
        ResolvedType::List(inner) => {
            let inner_ty = type_as_bindings(inner, iface_path);
            quote! { Vec<#inner_ty> }
        }
        ResolvedType::Option(inner) => {
            let inner_ty = type_as_bindings(inner, iface_path);
            quote! { Option<#inner_ty> }
        }
        ResolvedType::Result { ok, err } => {
            let ok_ty = ok
                .as_ref()
                .map(|t| type_as_bindings(t, iface_path))
                .unwrap_or_else(|| quote! { () });
            let err_ty = err
                .as_ref()
                .map(|t| type_as_bindings(t, iface_path))
                .unwrap_or_else(|| quote! { () });
            quote! { Result<#ok_ty, #err_ty> }
        }
        ResolvedType::Tuple(elems) => {
            if elems.is_empty() {
                quote! { () }
            } else {
                let parts: Vec<_> = elems
                    .iter()
                    .map(|t| type_as_bindings(t, iface_path))
                    .collect();
                quote! { (#(#parts),*) }
            }
        }
        ResolvedType::Named(name) | ResolvedType::Own(name) => {
            bindings_type_ident(name, iface_path)
        }
        ResolvedType::Borrow(name) => bindings_type_ident(name, iface_path),
        ResolvedType::Future(inner) => {
            // wit-bindgen: async fn returns the payload directly.
            match inner {
                Some(t) => type_as_bindings(t, iface_path),
                None => quote! { () },
            }
        }
        ResolvedType::Stream(inner) => {
            let payload = inner
                .as_ref()
                .map(|t| type_as_bindings(t, iface_path))
                .unwrap_or_else(|| quote! { () });
            quote! { wit_bindgen::rt::async_support::StreamReader<#payload> }
        }
    }
}

fn return_type_bindings(
    return_type: &Option<ResolvedType>,
    iface_path: &TokenStream,
) -> TokenStream {
    match return_type {
        Some(ret) if !matches!(ret, ResolvedType::Tuple(e) if e.is_empty()) => {
            let ty = type_as_bindings(ret, iface_path);
            quote! { -> #ty }
        }
        _ => quote! {},
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the `bindings::exports::ns::pkg::iface` token path from a WIT package
/// id like `"ns:pkg@1.0.0"` and interface name.
fn build_iface_path(package: &str, interface: &str) -> TokenStream {
    // Strip version: "ns:pkg@1.0.0" → "ns:pkg"
    let no_version = package.split('@').next().unwrap_or(package);
    // Split on ':' → ["ns", "pkg"]
    let parts: Vec<&str> = no_version.splitn(2, ':').collect();
    let (ns, pkg) = if parts.len() == 2 {
        (parts[0], parts[1])
    } else {
        (parts[0], parts[0])
    };

    // wit-bindgen converts kebab-case identifiers to snake_case for Rust modules
    let ns_ident = format_ident!("{}", ns.replace('-', "_"));
    let pkg_ident = format_ident!("{}", pkg.replace('-', "_"));
    let iface_ident = format_ident!("{}", interface.replace('-', "_"));

    quote! { bindings::exports::#ns_ident::#pkg_ident::#iface_ident }
}

fn pascal_case(kebab: &str) -> String {
    kebab
        .split('-')
        .map(|seg| {
            let mut c = seg.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}
