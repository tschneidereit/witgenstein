// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Integration tests for witgenstein-rs.
//!
//! These tests run the binary against fixture Rust crates and verify the
//! generated WIT output.

use std::path::PathBuf;
use std::process::Command;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn binary_path() -> PathBuf {
    // In a workspace, binaries are built in the workspace root's target directory.
    project_root().join("../../target/debug/witgenstein-rs")
}

fn fixture_dir(name: &str) -> PathBuf {
    project_root().join("tests").join("fixtures").join(name)
}

/// Run witgenstein-rs on a fixture crate with the given package identity.
fn run_on_fixture(name: &str, package: &str) -> (String, String, bool) {
    let bin = binary_path();
    let dir = fixture_dir(name);

    let output = Command::new(&bin)
        .arg("generate")
        .arg(&dir)
        .arg("--package")
        .arg(package)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", bin.display()));

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

#[test]
fn basic_fixture_generates_wit() {
    let (stdout, stderr, success) = run_on_fixture("basic-crate", "test:basic-crate");
    assert!(
        success,
        "witgenstein-rs failed on basic-crate.\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.is_empty(),
        "expected WIT output but got nothing.\nstderr:\n{stderr}"
    );
    // Should contain the package declaration.
    assert!(
        stdout.contains("package test:basic-crate;"),
        "missing package declaration in:\n{stdout}"
    );
}

#[test]
fn basic_fixture_has_exported_functions() {
    let (stdout, _stderr, success) = run_on_fixture("basic-crate", "test:basic-crate");
    assert!(success);

    // Check for function signatures.
    assert!(
        stdout.contains("add: func("),
        "missing 'add' function in:\n{stdout}"
    );
    assert!(
        stdout.contains("greet: func("),
        "missing 'greet' function in:\n{stdout}"
    );
    assert!(
        stdout.contains("sum-list: func("),
        "missing 'sum-list' function in:\n{stdout}"
    );
    assert!(
        stdout.contains("try-parse: func("),
        "missing 'try-parse' function in:\n{stdout}"
    );
    assert!(
        stdout.contains("maybe-greet: func("),
        "missing 'maybe-greet' function in:\n{stdout}"
    );
}

#[test]
fn basic_fixture_add_signature() {
    let (stdout, _, success) = run_on_fixture("basic-crate", "test:basic-crate");
    assert!(success);

    // `add(a: u32, b: u32) -> u32` should map to WIT types.
    assert!(
        stdout.contains("u32"),
        "expected u32 type in WIT output:\n{stdout}"
    );
}

#[test]
fn basic_fixture_string_types() {
    let (stdout, _, success) = run_on_fixture("basic-crate", "test:basic-crate");
    assert!(success);

    // `greet(name: String) -> String` should use WIT `string` type.
    assert!(
        stdout.contains("string"),
        "expected string type in WIT output:\n{stdout}"
    );
}

#[test]
fn basic_fixture_list_types() {
    let (stdout, _, success) = run_on_fixture("basic-crate", "test:basic-crate");
    assert!(success);

    // `sum_list(values: Vec<f64>) -> f64` should produce `list<f64>`.
    assert!(
        stdout.contains("list<f64>"),
        "expected list<f64> in WIT output:\n{stdout}"
    );
}

#[test]
fn basic_fixture_result_types() {
    let (stdout, _, success) = run_on_fixture("basic-crate", "test:basic-crate");
    assert!(success);

    // `try_parse(input: String) -> Result<u32, String>` should produce `result<u32, string>`.
    assert!(
        stdout.contains("result<u32, string>"),
        "expected result<u32, string> in WIT output:\n{stdout}"
    );
}

#[test]
fn basic_fixture_option_types() {
    let (stdout, _, success) = run_on_fixture("basic-crate", "test:basic-crate");
    assert!(success);

    // `maybe_greet(name: Option<String>) -> Option<String>` should produce `option<string>`.
    assert!(
        stdout.contains("option<string>"),
        "expected option<string> in WIT output:\n{stdout}"
    );
}

#[test]
fn basic_fixture_has_resource() {
    let (stdout, _stderr, success) = run_on_fixture("basic-crate", "test:basic-crate");
    assert!(success);

    // The `Counter` resource should appear.
    assert!(
        stdout.contains("resource counter"),
        "missing 'resource counter' in:\n{stdout}"
    );
}

#[test]
fn basic_fixture_resource_methods() {
    let (stdout, _, success) = run_on_fixture("basic-crate", "test:basic-crate");
    assert!(success);

    // Constructor: `constructor(initial: u32)`
    assert!(
        stdout.contains("constructor("),
        "missing constructor in:\n{stdout}"
    );

    // Methods: get, increment, add
    assert!(
        stdout.contains("get: func("),
        "missing 'get' method in:\n{stdout}"
    );
    assert!(
        stdout.contains("increment: func("),
        "missing 'increment' method in:\n{stdout}"
    );
}

#[test]
fn explicit_package_with_version() {
    let (stdout, _stderr, success) = run_on_fixture("basic-crate", "myorg:my-package@1.0.0");
    assert!(success);

    assert!(
        stdout.contains("package myorg:my-package@1.0.0;"),
        "missing versioned package declaration in:\n{stdout}"
    );
}

#[test]
fn basic_fixture_has_world() {
    let (stdout, _, success) = run_on_fixture("basic-crate", "test:basic-crate");
    assert!(success);

    assert!(
        stdout.contains("world "),
        "missing world declaration in:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// build subcommand tests
// ---------------------------------------------------------------------------

/// Run `witgenstein build` on a fixture crate.
fn run_build_on_fixture(name: &str, package: &str) -> (String, String, bool) {
    let bin = binary_path();
    let dir = fixture_dir(name);

    let output = Command::new(&bin)
        .arg("build")
        .arg(&dir)
        .arg("--package")
        .arg(package)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", bin.display()));

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

#[test]
fn build_produces_wasm_component() {
    let (_stdout, stderr, success) = run_build_on_fixture("basic-crate", "test:basic-crate");
    assert!(
        success,
        "witgenstein build failed on basic-crate.\nstderr:\n{stderr}"
    );
    // The output path is printed to stderr.
    assert!(
        stderr.contains(".wasm"),
        "expected .wasm path in stderr:\n{stderr}"
    );
}

#[test]
fn build_component_file_exists() {
    let (_stdout, stderr, success) = run_build_on_fixture("basic-crate", "test:basic-crate");
    assert!(success, "build failed:\n{stderr}");

    // Extract the wasm path from stderr.
    let prefix = "Component: ";
    let wasm_line = stderr
        .lines()
        .find(|l| l.starts_with(prefix))
        .expect("missing 'Component: ' line in stderr");
    let wasm_path = wasm_line
        .trim()
        .strip_prefix(prefix)
        .unwrap();
    assert!(
        std::path::Path::new(wasm_path).is_file(),
        "component file does not exist: {wasm_path}"
    );
}
