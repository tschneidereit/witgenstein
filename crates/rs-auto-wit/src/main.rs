// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! rs-auto-wit: generate WIT interface definitions from Rust source files.

mod cli;
mod diagnostic;
mod discover;
mod emit;
mod mapper;
mod resolve;

use std::path::Path;

use clap::Parser;
use miette::{Context, IntoDiagnostic, Result};

use crate::cli::Args;
use crate::emit::EmitConfig;

fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Resolve the crate root directory.
    let crate_root = if let Some(input) = &args.input {
        let path = std::fs::canonicalize(input)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to resolve input path: {}", input.display()))?;
        if path.is_file() {
            path.parent()
                .ok_or_else(|| miette::miette!("cannot determine crate directory"))?
                .to_path_buf()
        } else {
            path
        }
    } else {
        std::env::current_dir()
            .into_diagnostic()
            .wrap_err("failed to get current directory")?
    };

    // Verify Cargo.toml exists.
    if !crate_root.join("Cargo.toml").is_file() {
        miette::bail!("no Cargo.toml found in {}", crate_root.display());
    }

    eprintln!("crate: {}", crate_root.display());

    // 2. Determine WIT package identity.
    let (namespace, pkg_name, version) = if let Some(pkg_str) = &args.package {
        parse_wit_package_id(pkg_str)?
    } else {
        derive_package_from_cargo(&crate_root)?
    };

    // 3. Discover #[export] annotations via syn.
    let discovered =
        discover::discover_exports(&crate_root).wrap_err("failed to discover exports")?;

    if discovered.items.is_empty() {
        miette::bail!("no #[export] annotations found in the crate");
    }

    eprintln!("found {} exported items", discovered.items.len());
    for item in &discovered.items {
        match item {
            discover::ExportedItem::Function(name) => eprintln!("  fn {name}"),
            discover::ExportedItem::Impl(name) => eprintln!("  impl {name}"),
        }
    }

    // 4. Resolve types via rustdoc JSON.
    let resolved = resolve::resolve_exports(&crate_root, &discovered.items)
        .wrap_err("failed to resolve types via rustdoc")?;

    // Print any warnings.
    for warning in &resolved.warnings {
        eprintln!("warning: {warning}");
    }

    // 5. Generate WIT output.
    let config = EmitConfig {
        namespace,
        package_name: pkg_name,
        version,
    };
    let wit_output = emit::emit_wit(&config, &resolved);

    // 6. Write output.
    if let Some(output_path) = &args.output {
        std::fs::write(output_path, &wit_output)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to write {}", output_path.display()))?;
        eprintln!("wrote {}", output_path.display());
    } else {
        print!("{wit_output}");
    }

    Ok(())
}

/// Parse a WIT package identifier like "myorg:my-pkg@1.0.0".
fn parse_wit_package_id(s: &str) -> Result<(String, String, Option<String>)> {
    let (rest, version) = if let Some((rest, ver)) = s.rsplit_once('@') {
        (rest, Some(ver.to_string()))
    } else {
        (s, None)
    };

    let (namespace, name) = rest.split_once(':').ok_or_else(|| {
        miette::miette!("invalid WIT package ID: expected 'namespace:name', got '{s}'")
    })?;

    Ok((namespace.to_string(), name.to_string(), version))
}

/// Derive a WIT package identity from the crate's Cargo.toml.
fn derive_package_from_cargo(crate_root: &Path) -> Result<(String, String, Option<String>)> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(crate_root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .map_err(|e| miette::miette!("failed to read cargo metadata: {e}"))?;

    let package = metadata
        .packages
        .first()
        .ok_or_else(|| miette::miette!("no packages found in cargo metadata"))?;

    let name = package.name.replace('_', "-");
    let version = Some(package.version.to_string());

    // Use the crate name as both namespace and package name if no better option.
    // Users should use --package for proper namespace control.
    Ok((name.clone(), name, version))
}
