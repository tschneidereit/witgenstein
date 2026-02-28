// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Module graph: parse TypeScript files and follow local imports to build
//! a complete picture of all modules involved in the project.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use miette::{Context, IntoDiagnostic, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::ImportDeclaration;
use oxc_ast_visit::Visit;
use oxc_parser::{ParseOptions, Parser};
use oxc_resolver::{
    ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};
use oxc_span::SourceType;

use crate::project::has_ts_extension;

/// A parsed TypeScript module.
pub struct ParsedModule {
    /// The source text of the module (owned, since the allocator borrows it).
    pub source_text: String,
}

/// An import from an external (non-local) package.
#[derive(Debug, Clone)]
pub struct ExternalImport {
    /// The import specifier (e.g. "lodash", "@types/node").
    pub specifier: String,
    /// The imported names (or None for namespace/default imports).
    pub names: Vec<String>,
}

/// The complete module graph for a project.
pub struct ModuleGraph {
    /// Map from canonical file path to parsed module.
    pub modules: HashMap<PathBuf, ParsedModule>,
    /// The entry file path.
    pub entry: PathBuf,
    /// All external imports encountered across all modules.
    pub external_imports: Vec<ExternalImport>,
}

/// Collects import specifiers from a parsed AST.
struct ImportCollector {
    local_specifiers: Vec<String>,
    external_imports: Vec<ExternalImport>,
}

impl ImportCollector {
    fn new() -> Self {
        Self {
            local_specifiers: Vec::new(),
            external_imports: Vec::new(),
        }
    }

    fn is_local_specifier(specifier: &str) -> bool {
        specifier.starts_with('.') || specifier.starts_with('/')
    }
}

impl<'a> Visit<'a> for ImportCollector {
    fn visit_import_declaration(&mut self, decl: &ImportDeclaration<'a>) {
        let specifier = decl.source.value.as_str();

        if Self::is_local_specifier(specifier) {
            self.local_specifiers.push(specifier.to_string());
        } else if specifier != "witgenstein-ts" {
            // Collect imported names for external modules (skip witgenstein-ts branded types)
            let names: Vec<String> = decl
                .specifiers
                .as_ref()
                .into_iter()
                .flat_map(|s| s.iter())
                .map(|s| {
                    use oxc_ast::ast::ImportDeclarationSpecifier;
                    match s {
                        ImportDeclarationSpecifier::ImportSpecifier(s) => s.local.name.to_string(),
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                            s.local.name.to_string()
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                            s.local.name.to_string()
                        }
                    }
                })
                .collect();

            self.external_imports.push(ExternalImport {
                specifier: specifier.to_string(),
                names,
            });
        }
    }
}

/// Build the module graph starting from the given entry file.
pub fn build_module_graph(entry: &Path, project_root: &Path) -> Result<ModuleGraph> {
    let resolver = create_resolver(project_root);
    let mut modules = HashMap::new();
    let mut all_external_imports = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = vec![entry.to_path_buf()];

    while let Some(file_path) = queue.pop() {
        let canonical = file_path
            .canonicalize()
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to canonicalize path: {}", file_path.display()))?;

        if !visited.insert(canonical.clone()) {
            continue;
        }

        let source_text = std::fs::read_to_string(&canonical)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to read file: {}", canonical.display()))?;

        let allocator = Allocator::default();
        let source_type = SourceType::from_path(&canonical)
            .map_err(|e| miette::miette!("unsupported file type {}: {e}", canonical.display()))?;

        // Parse the file
        let ret = Parser::new(&allocator, &source_text, source_type)
            .with_options(ParseOptions::default())
            .parse();

        if ret.panicked {
            miette::bail!("parser panicked on file: {}", canonical.display());
        }

        if !ret.errors.is_empty() {
            let errors: Vec<String> = ret.errors.iter().map(|e| e.to_string()).collect();
            miette::bail!(
                "parse errors in {}:\n{}",
                canonical.display(),
                errors.join("\n")
            );
        }

        // Collect imports
        let mut collector = ImportCollector::new();
        collector.visit_program(&ret.program);

        // Resolve local imports and queue them
        let parent_dir = canonical.parent().unwrap_or(Path::new("."));
        for specifier in &collector.local_specifiers {
            match resolver.resolve(parent_dir, specifier) {
                Ok(resolution) => {
                    let resolved_path = resolution.full_path();
                    if has_ts_extension(&resolved_path) && !visited.contains(&resolved_path) {
                        queue.push(resolved_path);
                    }
                }
                Err(e) => {
                    // Try manual resolution with common extensions
                    if let Some(resolved) = try_manual_resolve(parent_dir, specifier) {
                        if !visited.contains(&resolved) {
                            queue.push(resolved);
                        }
                    } else {
                        eprintln!(
                            "warning: could not resolve import '{specifier}' from {}: {e}",
                            canonical.display()
                        );
                    }
                }
            }
        }

        all_external_imports.extend(collector.external_imports);

        modules.insert(canonical, ParsedModule { source_text });
    }

    let entry_canonical = entry
        .canonicalize()
        .into_diagnostic()
        .wrap_err("failed to canonicalize entry path")?;

    Ok(ModuleGraph {
        modules,
        entry: entry_canonical,
        external_imports: all_external_imports,
    })
}

fn create_resolver(project_root: &Path) -> Resolver {
    let tsconfig_path = project_root.join("tsconfig.json");
    let tsconfig = if tsconfig_path.is_file() {
        Some(TsconfigDiscovery::Manual(TsconfigOptions {
            config_file: tsconfig_path,
            references: TsconfigReferences::Auto,
        }))
    } else {
        None
    };

    Resolver::new(ResolveOptions {
        extensions: vec![
            ".ts".into(),
            ".tsx".into(),
            ".mts".into(),
            ".cts".into(),
            ".js".into(),
            ".mjs".into(),
            ".cjs".into(),
        ],
        extension_alias: vec![
            (".js".into(), vec![".ts".into(), ".js".into()]),
            (".mjs".into(), vec![".mts".into(), ".mjs".into()]),
            (".cjs".into(), vec![".cts".into(), ".cjs".into()]),
        ],
        condition_names: vec!["import".into(), "types".into()],
        tsconfig,
        ..Default::default()
    })
}

/// Manually try resolving a local import with common TS extensions.
fn try_manual_resolve(parent_dir: &Path, specifier: &str) -> Option<PathBuf> {
    let base = parent_dir.join(specifier);

    // Try direct file with extensions
    for ext in &[".ts", ".tsx", ".mts", ".cts"] {
        let path = PathBuf::from(format!("{}{ext}", base.display()));
        if path.is_file() {
            return path.canonicalize().ok();
        }
    }

    // Try index file in directory
    for ext in &[".ts", ".tsx"] {
        let path = base.join(format!("index{ext}"));
        if path.is_file() {
            return path.canonicalize().ok();
        }
    }

    None
}
