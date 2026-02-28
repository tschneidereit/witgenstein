// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! witgenstein-ts: generate WIT interface definitions from TypeScript source files.

mod branded;
mod cli;
mod collect;
mod diagnostic;
mod emit;
mod mapper;
mod module_graph;
mod project;
mod types;

use clap::Parser;
use miette::{Context, IntoDiagnostic, Result};
use oxc_allocator::Allocator;
use oxc_parser::{ParseOptions, Parser as OxcParser};
use oxc_span::SourceType;

use crate::cli::Args;
use crate::types::{WitInterface, WitPackage, WitWorld};

fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Resolve the TypeScript project
    let project = project::resolve_project(args.input.as_deref())
        .wrap_err("failed to resolve TypeScript project")?;

    eprintln!("entry: {}", project.entry_file.display());
    eprintln!("root:  {}", project.project_root.display());

    // 2. Determine output path (stdout if omitted)
    let output_path = args.output;

    // 3. Determine WIT package identity
    let (namespace, pkg_name, version) = if let Some(pkg_str) = &args.package {
        wit_common::parse_wit_package_id(pkg_str).map_err(|e| miette::miette!("{e}"))?
    } else if let Some(meta) = &project.package_meta {
        project::derive_wit_package(meta).ok_or_else(|| {
            miette::miette!("could not derive WIT package name from package.json — use --package")
        })?
    } else {
        miette::bail!("no package.json found and --package not specified");
    };

    // 4. Build module graph (parse entry + follow local imports)
    let graph = module_graph::build_module_graph(&project.entry_file, &project.project_root)
        .wrap_err("failed to build module graph")?;

    // 5. Collect branded types from all modules
    let mut branded_types = branded::BrandedTypeMap::new();
    for (path, module) in &graph.modules {
        let allocator = Allocator::default();
        let source_type = SourceType::from_path(path)
            .map_err(|e| miette::miette!("unsupported file type: {e}"))?;
        let ret = OxcParser::new(&allocator, &module.source_text, source_type)
            .with_options(ParseOptions::default())
            .parse();
        let module_branded = branded::collect_branded_types(&ret.program);
        branded_types.extend(module_branded);
    }

    // 6. Collect exports from entry file
    let entry_module = graph
        .modules
        .get(&graph.entry)
        .ok_or_else(|| miette::miette!("entry file not found in module graph"))?;

    let exported_items = {
        let allocator = Allocator::default();
        let source_type = SourceType::from_path(&graph.entry)
            .map_err(|e| miette::miette!("unsupported file type: {e}"))?;
        let ret = OxcParser::new(&allocator, &entry_module.source_text, source_type)
            .with_options(ParseOptions::default())
            .parse();
        collect::ExportCollector::collect(&ret.program)
    };

    eprintln!("found {} exported items", exported_items.len());

    // 7. Map TypeScript types to WIT IR
    let map_config = mapper::MapperConfig {
        strict: args.strict,
    };
    let map_result = mapper::map_exports(&exported_items, &branded_types, &map_config);

    // 8. Report diagnostics
    if !map_result.diagnostics.is_empty() {
        diagnostic::report_diagnostics(&map_result.diagnostics);
    }

    // 9. Detect external imports used in the public API
    let import_interfaces = build_import_interfaces(&graph.external_imports);

    // 10. Build the WIT package
    let wit_name = to_wit_interface_name(&pkg_name);
    let package = WitPackage {
        namespace,
        name: pkg_name.clone(),
        version,
        world: WitWorld {
            name: wit_name.clone(),
            exports: WitInterface {
                name: format!("{wit_name}-types"),
                type_defs: map_result.type_defs,
                functions: map_result.functions,
            },
            imports: import_interfaces,
        },
    };

    // 11. Emit WIT and write to file
    let wit_output = emit::emit_wit(&package);

    if let Some(path) = &output_path {
        std::fs::write(path, &wit_output)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to write output: {}", path.display()))?;
        eprintln!("wrote {}", path.display());
    } else {
        print!("{wit_output}");
    }

    // Return error if there were diagnostics in strict mode
    if args.strict && !map_result.diagnostics.is_empty() {
        miette::bail!(
            "{} type mapping issue(s) found — fix them and try again",
            map_result.diagnostics.len()
        );
    }

    Ok(())
}

/// Build WIT import interfaces from external import information.
fn build_import_interfaces(external_imports: &[module_graph::ExternalImport]) -> Vec<WitInterface> {
    // Group imports by specifier
    let mut by_specifier: std::collections::HashMap<&str, Vec<&str>> =
        std::collections::HashMap::new();
    for imp in external_imports {
        let names: Vec<&str> = imp.names.iter().map(|s| s.as_str()).collect();
        by_specifier
            .entry(&imp.specifier)
            .or_default()
            .extend(names);
    }

    by_specifier
        .into_keys()
        .map(|specifier| {
            let iface_name = to_wit_interface_name(specifier);
            WitInterface {
                name: iface_name,
                type_defs: Vec::new(),
                functions: Vec::new(),
            }
        })
        .collect()
}

fn to_wit_interface_name(name: &str) -> String {
    // Strip @ and / from npm-style names, convert to kebab-case
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();

    // Remove leading/trailing hyphens and collapse multiple hyphens
    let mut result = String::new();
    let mut prev_was_hyphen = true; // treat start as after hyphen to skip leading
    for c in cleaned.chars() {
        if c == '-' {
            if !prev_was_hyphen {
                result.push('-');
            }
            prev_was_hyphen = true;
        } else {
            result.push(c.to_ascii_lowercase());
            prev_was_hyphen = false;
        }
    }

    // Remove trailing hyphen
    if result.ends_with('-') {
        result.pop();
    }

    if result.is_empty() {
        "unnamed".to_string()
    } else {
        result
    }
}
