// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! WIT generation using `wit-encoder`.
//!
//! This module assembles the mapped types and functions into a complete WIT
//! package using the `wit-encoder` crate, then renders it to text.

use wit_encoder::{Ident, Interface, Package, PackageName, TypeDef, TypeDefKind, World};

use crate::mapper;
use crate::resolve::ResolveResult;

/// Configuration for WIT emission.
pub struct EmitConfig {
    pub namespace: String,
    pub package_name: String,
    pub version: Option<String>,
}

/// Generate a WIT package string from resolved exports.
pub fn emit_wit(config: &EmitConfig, resolved: &ResolveResult) -> String {
    let version = config
        .version
        .as_ref()
        .and_then(|v| semver::Version::parse(v).ok());

    let package_name = PackageName::new(
        config.namespace.clone(),
        Ident::new(config.package_name.clone()),
        version,
    );
    let mut package = Package::new(package_name);

    // Build the types interface with all type definitions.
    let has_types = !resolved.type_defs.is_empty() || !resolved.impls.is_empty();

    if has_types {
        let mut types_interface = Interface::new(Ident::new("types"));

        for type_def in &resolved.type_defs {
            let wit_td = mapper::map_type_def(type_def);
            types_interface.type_def(wit_td);
        }

        // Add resources from impl blocks.
        for imp in &resolved.impls {
            let resource = mapper::map_resource(imp);
            types_interface.type_def(TypeDef::new(
                wit_common::to_kebab_case(&imp.type_name),
                TypeDefKind::Resource(resource),
            ));
        }

        package.interface(types_interface);
    }

    // Build the world.
    let mut world = World::new(Ident::new(config.package_name.clone()));

    if has_types {
        world.named_interface_export(Ident::new("types"));
    }

    // Add standalone exported functions.
    for func in &resolved.functions {
        let wit_func = mapper::map_function(func);
        world.function_export(wit_func);
    }

    package.world(world);

    package.to_string()
}
