// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! witgenstein: generate WIT interface definitions from Rust source files,
//! and build wasm32-wasip2 components.

mod cli;
mod codegen;
mod discover;
mod emit;
mod mapper;
mod resolve;

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use miette::{Context, IntoDiagnostic, Result};

use crate::cli::{BuildArgs, Cli, GenerateArgs};
use crate::emit::EmitConfig;
use crate::resolve::ResolveResult;

fn main() -> Result<()> {
    let cli = Cli::parse();

    let verbose = cli.verbose;
    match cli.command {
        cli::Command::Generate(args) => cmd_generate(&args, verbose),
        cli::Command::Build(args) => cmd_build(&args, verbose),
    }
}

// ---------------------------------------------------------------------------
// Cargo metadata — read once, use everywhere
// ---------------------------------------------------------------------------

/// All crate metadata needed by both commands, read from a single
/// `cargo_metadata` invocation.
struct CrateInfo {
    crate_name: String,
    target_dir: PathBuf,
}

fn read_crate_info(crate_root: &Path) -> Result<CrateInfo> {
    let manifest_path = crate_root.join("Cargo.toml");
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(&manifest_path)
        .no_deps()
        .exec()
        .map_err(|e| miette::miette!("failed to read cargo metadata: {e}"))?;

    let package = metadata
        .packages
        .iter()
        .find(|p| p.manifest_path == manifest_path)
        .ok_or_else(|| miette::miette!("package not found in cargo metadata"))?;

    Ok(CrateInfo {
        crate_name: package.name.to_string(),
        target_dir: metadata.target_directory.as_std_path().to_path_buf(),
    })
}

// ---------------------------------------------------------------------------
// Shared pipeline: discover → resolve → emit
// ---------------------------------------------------------------------------

/// Result of the shared discovery/resolution/emission pipeline.
struct PrepareResult {
    config: EmitConfig,
    wit_content: String,
    resolved: ResolveResult,
    info: CrateInfo,
}

/// Run the common pipeline used by both `generate` and `build`:
/// read metadata → discover exports → resolve via rustdoc → emit WIT.
fn prepare_crate(
    crate_root: &Path,
    package_arg: &Option<String>,
    verbose: bool,
) -> Result<PrepareResult> {
    let info = read_crate_info(crate_root)?;

    let (namespace, pkg_name, version) = resolve_package(package_arg, &info)?;

    let discovered =
        discover::discover_exports(crate_root).wrap_err("failed to discover exports")?;

    if discovered.items.is_empty() {
        miette::bail!("no #[export] annotations found in the crate");
    }

    if verbose {
        print_discovered(&discovered);
    }

    let resolved = resolve::resolve_exports(crate_root, &discovered.items)
        .wrap_err("failed to resolve types via rustdoc")?;

    for warning in &resolved.warnings {
        eprintln!("warning: {warning}");
    }

    let config = EmitConfig {
        namespace,
        package_name: pkg_name,
        version,
        crate_name: info.crate_name.clone(),
    };
    let wit_content = emit::emit_wit(&config, &resolved);

    Ok(PrepareResult {
        config,
        wit_content,
        resolved,
        info,
    })
}

// ---------------------------------------------------------------------------
// generate subcommand
// ---------------------------------------------------------------------------

fn cmd_generate(args: &GenerateArgs, verbose: bool) -> Result<()> {
    let crate_root = resolve_crate_root(args.input.as_ref())?;
    let prepared = prepare_crate(&crate_root, &args.package, verbose)?;

    if let Some(output_path) = &args.output {
        std::fs::write(output_path, &prepared.wit_content)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to write {}", output_path.display()))?;
        eprintln!("wrote {}", output_path.display());
    } else {
        print!("{}", prepared.wit_content);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// build subcommand
// ---------------------------------------------------------------------------

fn cmd_build(args: &BuildArgs, verbose: bool) -> Result<()> {
    let crate_root = resolve_crate_root(args.input.as_ref())?;

    // Components must be library crates.
    if !crate_root.join("src/lib.rs").is_file() {
        miette::bail!(
            "build requires a library crate (src/lib.rs) in {}",
            crate_root.display()
        );
    }

    let prepared = prepare_crate(&crate_root, &args.package, verbose)?;

    let wrapper_dir = prepared.info.target_dir.join("witgenstein/component");
    let build_target_dir = prepared.info.target_dir.join("witgenstein/build");

    // Generate wrapper crate.
    codegen::generate_wrapper_crate(
        &wrapper_dir,
        &crate_root,
        &prepared.wit_content,
        &prepared.config,
        &prepared.resolved,
        &prepared.info.crate_name,
    )
    .wrap_err("failed to generate wrapper crate")?;

    // Build for wasm32-wasip2.
    let profile = if args.release { "release" } else { "debug" };

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--target")
        .arg("wasm32-wasip2")
        .arg("--manifest-path")
        .arg(wrapper_dir.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&build_target_dir);

    if args.release {
        cmd.arg("--release");
    }

    // Report output location.
    let wasm_name = format!(
        "{}_component.wasm",
        prepared.info.crate_name.replace('-', "_")
    );

    let output = cmd
        .status()
        .into_diagnostic()
        .wrap_err("failed to invoke cargo build")?;

    if !output.success() {
        miette::bail!("component build failed");
    }

    let wasm_path = build_target_dir
        .join("wasm32-wasip2")
        .join(profile)
        .join(&wasm_name);
    let display_path = std::env::current_dir()
        .ok()
        .and_then(|cwd| wasm_path.strip_prefix(&cwd).ok().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| wasm_path.clone());
    eprintln!("Component: {}", display_path.display());

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the crate root directory from an optional input path.
fn resolve_crate_root(input: Option<&PathBuf>) -> Result<PathBuf> {
    let crate_root = if let Some(input) = input {
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

    if !crate_root.join("Cargo.toml").is_file() {
        miette::bail!("no Cargo.toml found in {}", crate_root.display());
    }

    Ok(crate_root)
}

/// Resolve the WIT package identity from an explicit `--package` flag or
/// by deriving it from cargo metadata.
fn resolve_package(
    package_arg: &Option<String>,
    info: &CrateInfo,
) -> Result<(String, String, Option<String>)> {
    if let Some(pkg_str) = package_arg {
        wit_common::parse_wit_package_id(pkg_str).map_err(|e| miette::miette!("{e}"))
    } else {
        let name = info.crate_name.replace('_', "-");
        Ok((name.clone(), name, None))
    }
}

fn print_discovered(discovered: &discover::DiscoverResult) {
    eprintln!("found {} exported items", discovered.items.len());
    for item in &discovered.items {
        match item {
            discover::ExportedItem::Function(name) => eprintln!("  fn {name}"),
            discover::ExportedItem::Impl(name) => eprintln!("  impl {name}"),
        }
    }
}
