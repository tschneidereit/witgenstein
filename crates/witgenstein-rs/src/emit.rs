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
    /// The crate name from Cargo.toml, used for the interface and world names.
    pub crate_name: String,
}

impl EmitConfig {
    /// The WIT interface name, derived from the crate name.
    pub fn interface_name(&self) -> String {
        wit_common::to_kebab_case(&self.crate_name)
    }

    /// The WIT world name (`{interface_name}-world`).
    pub fn world_name(&self) -> String {
        format!("{}-world", self.interface_name())
    }

    /// A fully-qualified world selector for `wit_bindgen::generate!`.
    pub fn world_selector(&self) -> String {
        let world_name = self.world_name();
        match &self.version {
            Some(ver) => {
                format!(
                    "{}:{}/{}@{}",
                    self.namespace, self.package_name, world_name, ver
                )
            }
            None => format!("{}:{}/{}", self.namespace, self.package_name, world_name),
        }
    }

    /// The Rust identifier for the interface (kebab replaced with `_`).
    pub fn interface_ident(&self) -> String {
        self.interface_name().replace('-', "_")
    }
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

    // Build the types interface with all type definitions and standalone
    // functions.  Placing everything in a single interface ensures the
    // component's exports all live under the user-provided package identity
    // rather than the synthetic `root:component` world that the component
    // model binary format produces.
    let has_exports = resolved.has_exports();
    let interface_name = config.interface_name();

    if has_exports {
        let mut iface = Interface::new(Ident::new(interface_name.clone()));

        for type_def in &resolved.type_defs {
            let wit_td = mapper::map_type_def(type_def);
            iface.type_def(wit_td);
        }

        // Add resources from impl blocks.
        for imp in &resolved.impls {
            let resource = mapper::map_resource(imp);
            iface.type_def(TypeDef::new(
                wit_common::to_kebab_case(&imp.type_name),
                TypeDefKind::Resource(resource),
            ));
        }

        // Add standalone exported functions.
        for func in &resolved.functions {
            let wit_func = mapper::map_function(func);
            iface.function(wit_func);
        }

        package.interface(iface);
    }

    // Build the world. Use a distinct name to avoid conflicting with the
    // interface that shares the package name.
    let mut world = World::new(Ident::new(config.world_name()));

    if has_exports {
        world.named_interface_export(Ident::new(interface_name.clone()));
    }

    package.world(world);

    package.to_string()
}
