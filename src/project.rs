// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Project resolution: find the TypeScript entry point, read package.json,
//! and derive output path and WIT package name.

use std::path::{Path, PathBuf};

use miette::{Context, IntoDiagnostic, Result};
use serde::Deserialize;

/// Metadata extracted from a TypeScript project.
#[derive(Debug)]
pub struct ProjectInfo {
    /// The resolved entry TypeScript source file.
    pub entry_file: PathBuf,
    /// The project root directory (containing package.json or tsconfig.json).
    pub project_root: PathBuf,
    /// Package metadata from package.json, if found.
    pub package_meta: Option<PackageMeta>,
}

/// Relevant fields from package.json.
#[derive(Debug, Deserialize)]
pub struct PackageMeta {
    pub name: Option<String>,
    pub version: Option<String>,
    pub main: Option<String>,
    pub types: Option<String>,
    pub typings: Option<String>,
    #[serde(default)]
    pub exports: serde_json::Value,
}

/// Relevant fields from tsconfig.json.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsConfig {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    compiler_options: Option<CompilerOptions>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompilerOptions {
    root_dir: Option<String>,
    out_dir: Option<String>,
}

/// Resolve the TypeScript project from the given input path.
///
/// - If `input` is `None`, uses the current working directory.
/// - If `input` is a file, uses it directly as the entry file.
/// - If `input` is a directory, searches for the entry file within it.
pub fn resolve_project(input: Option<&Path>) -> Result<ProjectInfo> {
    let input_path = match input {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()
            .into_diagnostic()
            .wrap_err("failed to get current working directory")?,
    };

    if input_path.is_file() {
        let project_root = input_path
            .parent()
            .map(find_project_root)
            .unwrap_or_else(|| input_path.clone());
        let package_meta = read_package_json(&project_root);
        return Ok(ProjectInfo {
            entry_file: input_path,
            project_root,
            package_meta,
        });
    }

    if !input_path.is_dir() {
        miette::bail!("input path does not exist: {}", input_path.display());
    }

    let package_meta = read_package_json(&input_path);

    // Try to find entry from package.json
    if let Some(meta) = &package_meta
        && let Some(entry) = entry_from_package_meta(meta, &input_path)
    {
        return Ok(ProjectInfo {
            entry_file: entry,
            project_root: input_path,
            package_meta,
        });
    }

    // Try to find entry from tsconfig.json
    if let Some(entry) = entry_from_tsconfig(&input_path) {
        return Ok(ProjectInfo {
            entry_file: entry,
            project_root: input_path,
            package_meta,
        });
    }

    // Fall back to well-known entry points
    for candidate in &["index.ts", "index.tsx", "src/index.ts", "src/index.tsx"] {
        let path = input_path.join(candidate);
        if path.is_file() {
            return Ok(ProjectInfo {
                entry_file: path,
                project_root: input_path,
                package_meta,
            });
        }
    }

    miette::bail!(
        "could not find a TypeScript entry point in {}",
        input_path.display()
    );
}

/// Walk up from a file's directory to find the project root (directory
/// containing `package.json` or `tsconfig.json`).
fn find_project_root(start: &Path) -> PathBuf {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join("package.json").is_file() || dir.join("tsconfig.json").is_file() {
            return dir;
        }
        if !dir.pop() {
            return start.to_path_buf();
        }
    }
}

/// Try reading package.json from a directory.
fn read_package_json(dir: &Path) -> Option<PackageMeta> {
    let path = dir.join("package.json");
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Try to derive the entry file from package.json metadata.
fn entry_from_package_meta(meta: &PackageMeta, project_root: &Path) -> Option<PathBuf> {
    // Prefer `types`/`typings` field (points to TS source or declarations)
    for path_str in [&meta.types, &meta.typings].into_iter().flatten() {
        let ts_source = try_resolve_ts_source(path_str, project_root);
        if let Some(entry) = ts_source {
            return Some(entry);
        }
    }

    // Try `main` field, converting .js extension to .ts
    if let Some(main) = &meta.main {
        let ts_source = try_resolve_ts_source(main, project_root);
        if let Some(entry) = ts_source {
            return Some(entry);
        }
    }

    // Try `exports` field (string or object with "." entry)
    match &meta.exports {
        serde_json::Value::String(s) => {
            return try_resolve_ts_source(s, project_root);
        }
        serde_json::Value::Object(obj) => {
            if let Some(serde_json::Value::String(s)) = obj.get(".") {
                return try_resolve_ts_source(s, project_root);
            }
        }
        _ => {}
    }

    None
}

/// Attempt to resolve a path that might reference a .js file to its .ts source.
fn try_resolve_ts_source(path_str: &str, project_root: &Path) -> Option<PathBuf> {
    let path = project_root.join(path_str);

    // Try the path as-is
    if path.is_file() && has_ts_extension(&path) {
        return Some(path);
    }

    // Try replacing .js/.mjs/.cjs with .ts/.mts/.cts
    let ts_path = swap_js_to_ts(&path);
    if let Some(ts) = ts_path
        && ts.is_file()
    {
        return Some(ts.clone());
    }

    // Try replacing .d.ts/.d.mts with .ts/.mts
    let stem = path_str
        .strip_suffix(".d.ts")
        .or(path_str.strip_suffix(".d.mts"));
    if let Some(stem) = stem {
        let ts = project_root.join(format!("{stem}.ts"));
        if ts.is_file() {
            return Some(ts);
        }
    }

    None
}

fn has_ts_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "tsx" | "mts" | "cts")
    )
}

fn swap_js_to_ts(path: &Path) -> Option<PathBuf> {
    let s = path.to_str()?;
    for (js_ext, ts_ext) in &[(".js", ".ts"), (".mjs", ".mts"), (".cjs", ".cts")] {
        if let Some(stem) = s.strip_suffix(js_ext) {
            return Some(PathBuf::from(format!("{stem}{ts_ext}")));
        }
    }
    None
}

/// Try to find the entry file from tsconfig.json.
fn entry_from_tsconfig(project_root: &Path) -> Option<PathBuf> {
    let tsconfig_path = project_root.join("tsconfig.json");
    let content = std::fs::read_to_string(&tsconfig_path).ok()?;
    let config: TsConfig = serde_json::from_str(&content).ok()?;

    // Use the first file in "files" that has a TS extension
    for file in &config.files {
        let path = project_root.join(file);
        if path.is_file() && has_ts_extension(&path) {
            return Some(path);
        }
    }

    // Use the first match from "include" patterns
    // For simplicity, check if any include patterns point to specific files
    for pattern in &config.include {
        // If it's a direct file reference (no glob chars), try it
        if !pattern.contains('*') && !pattern.contains('?') {
            let path = project_root.join(pattern);
            if path.is_file() && has_ts_extension(&path) {
                return Some(path);
            }
        }
    }

    None
}

/// Derive the output .wit file path from the package name.
///
/// E.g. `@myorg/my-pkg` → `my-pkg.wit` in the project root.
pub fn derive_output_path(project: &ProjectInfo) -> PathBuf {
    if let Some(meta) = &project.package_meta
        && let Some(name) = &meta.name
    {
        let wit_name = package_name_to_wit_name(name);
        return project.project_root.join(format!("{wit_name}.wit"));
    }
    project.project_root.join("output.wit")
}

/// Derive the WIT package identifier from package.json metadata.
///
/// `@myorg/my-pkg@1.0.0` → `myorg:my-pkg@1.0.0`
pub fn derive_wit_package(meta: &PackageMeta) -> Option<(String, String, Option<String>)> {
    let name = meta.name.as_deref()?;
    let (namespace, pkg_name) = if let Some(scoped) = name.strip_prefix('@') {
        let (scope, rest) = scoped.split_once('/')?;
        (scope.to_string(), rest.to_string())
    } else {
        // No scope: use "local" as namespace
        ("local".to_string(), name.to_string())
    };

    Some((namespace, pkg_name, meta.version.clone()))
}

/// Extract the short package name from an npm package name, stripping the scope.
fn package_name_to_wit_name(npm_name: &str) -> String {
    if let Some(scoped) = npm_name.strip_prefix('@')
        && let Some((_, rest)) = scoped.split_once('/')
    {
        return rest.to_string();
    }
    npm_name.to_string()
}

/// Parse a WIT package identifier string like `myorg:my-pkg@1.0.0`.
pub fn parse_wit_package_id(s: &str) -> Result<(String, String, Option<String>)> {
    let (name_part, version) = if let Some((name, ver)) = s.split_once('@') {
        (name, Some(ver.to_string()))
    } else {
        (s, None)
    };

    let (namespace, name) = name_part
        .split_once(':')
        .ok_or_else(|| miette::miette!("invalid WIT package id, expected 'namespace:name': {s}"))?;

    Ok((namespace.to_string(), name.to_string(), version))
}
