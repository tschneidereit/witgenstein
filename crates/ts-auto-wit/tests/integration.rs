// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Integration tests for ts-auto-wit.
//!
//! These tests run the binary against fixture TypeScript projects and verify
//! the generated WIT output.

use std::path::PathBuf;
use std::process::Command;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn binary_path() -> PathBuf {
    // In a workspace, binaries are built in the workspace root's target directory.
    let mut path = project_root();
    path.push("../../target");
    path.push("debug");
    path.push("ts-auto-wit");
    path
}

fn fixture_dir(name: &str) -> PathBuf {
    project_root().join("tests").join("fixtures").join(name)
}

/// Run ts-auto-wit on a fixture directory, outputting to stdout (default).
fn run_on_fixture(name: &str) -> (String, String, bool) {
    let bin = binary_path();
    let dir = fixture_dir(name);

    let output = Command::new(&bin)
        .arg(&dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", bin.display()));

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

/// Run ts-auto-wit with explicit package flag.
fn run_with_package(name: &str, package: &str) -> (String, String, bool) {
    let bin = binary_path();
    let dir = fixture_dir(name);

    let output = Command::new(&bin)
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
fn basic_fixture_generates_valid_wit() {
    let (stdout, stderr, success) = run_on_fixture("basic");
    assert!(success, "ts-auto-wit failed on basic fixture:\n{stderr}");

    // Should contain a package declaration
    assert!(
        stdout.contains("package test:basic"),
        "missing package declaration in:\n{stdout}"
    );

    // Should contain the greet function
    assert!(
        stdout.contains("greet"),
        "missing greet function in:\n{stdout}"
    );

    // Should contain the add function
    assert!(stdout.contains("add"), "missing add function in:\n{stdout}");

    // Should contain the Point record
    assert!(
        stdout.contains("record point"),
        "missing point record in:\n{stdout}"
    );

    // Should contain the Color enum
    assert!(
        stdout.contains("enum color"),
        "missing color enum in:\n{stdout}"
    );

    // Should contain world declaration
    assert!(
        stdout.contains("world "),
        "missing world declaration in:\n{stdout}"
    );
}

#[test]
fn basic_fixture_function_signatures() {
    let (stdout, _stderr, success) = run_on_fixture("basic");
    assert!(success, "ts-auto-wit failed");

    // greet: func(name: string) -> string
    assert!(
        stdout.contains("string") && stdout.contains("greet"),
        "greet function signature not found in:\n{stdout}"
    );

    // add should have f64 params (number defaults to f64)
    assert!(
        stdout.contains("f64") && stdout.contains("add"),
        "add function with f64 not found in:\n{stdout}"
    );
}

#[test]
fn basic_fixture_point_record_fields() {
    let (stdout, _stderr, success) = run_on_fixture("basic");
    assert!(success, "ts-auto-wit failed");

    // Point record should have x and y fields of type f64
    // (number maps to f64 by default)
    assert!(
        stdout.contains("x: f64") || stdout.contains("x: float64"),
        "point.x field not found in:\n{stdout}"
    );
    assert!(
        stdout.contains("y: f64") || stdout.contains("y: float64"),
        "point.y field not found in:\n{stdout}"
    );
}

#[test]
fn branded_types_use_correct_wit_types() {
    let (stdout, stderr, success) = run_on_fixture("branded");
    assert!(success, "ts-auto-wit failed on branded fixture:\n{stderr}");

    // Should use u32 for branded u32 types
    assert!(
        stdout.contains("u32"),
        "missing u32 type in branded output:\n{stdout}"
    );

    // Should use f32 for branded f32 types
    assert!(
        stdout.contains("f32"),
        "missing f32 type in branded output:\n{stdout}"
    );

    // Should contain the package
    assert!(
        stdout.contains("package test:branded"),
        "missing package in branded output:\n{stdout}"
    );
}

#[test]
fn resource_class_generates_resource() {
    let (stdout, stderr, success) = run_on_fixture("resource");
    assert!(success, "ts-auto-wit failed on resource fixture:\n{stderr}");

    // Should contain a resource declaration for Counter
    assert!(
        stdout.contains("resource counter"),
        "missing resource counter in:\n{stdout}"
    );

    // Should contain constructor
    assert!(
        stdout.contains("constructor"),
        "missing constructor in:\n{stdout}"
    );

    // Should contain increment method
    assert!(
        stdout.contains("increment"),
        "missing increment method in:\n{stdout}"
    );

    // Should contain get-count method (kebab-case)
    assert!(
        stdout.contains("get-count"),
        "missing get-count method in:\n{stdout}"
    );
}

#[test]
fn async_result_types() {
    let (stdout, stderr, success) = run_on_fixture("async_result");
    assert!(
        success,
        "ts-auto-wit failed on async_result fixture:\n{stderr}"
    );

    // Should contain package declaration
    assert!(
        stdout.contains("package test:async-result"),
        "missing package in async output:\n{stdout}"
    );

    // Should contain the fetch-data function
    assert!(
        stdout.contains("fetch-data"),
        "missing fetch-data function in:\n{stdout}"
    );

    // Should handle optional type (string | undefined -> option<string>)
    assert!(
        stdout.contains("option<string>") || stdout.contains("option"),
        "missing option type in:\n{stdout}"
    );
}

#[test]
fn explicit_package_flag_overrides() {
    let (stdout, stderr, success) = run_with_package("basic", "my-ns:my-pkg@2.0.0");
    assert!(success, "ts-auto-wit failed with --package:\n{stderr}");

    // Should use the explicit package name
    assert!(
        stdout.contains("package my-ns:my-pkg@2.0.0"),
        "explicit package not used in:\n{stdout}"
    );
}

#[test]
fn multi_module_processes_entry() {
    let (stdout, stderr, success) = run_on_fixture("multi_module");
    assert!(
        success,
        "ts-auto-wit failed on multi_module fixture:\n{stderr}"
    );

    // Should contain the initialize function
    assert!(
        stdout.contains("initialize"),
        "missing initialize function in:\n{stdout}"
    );

    // Should contain the package
    assert!(
        stdout.contains("package test:multi-module"),
        "missing package in multi_module output:\n{stdout}"
    );
}

#[test]
fn output_is_valid_wit_syntax() {
    let (stdout, _stderr, success) = run_on_fixture("basic");
    assert!(success, "ts-auto-wit failed");

    // Basic WIT syntax checks
    assert!(
        stdout.contains("package "),
        "missing package keyword:\n{stdout}"
    );
    assert!(
        stdout.contains("world "),
        "missing world keyword:\n{stdout}"
    );
    assert!(
        stdout.ends_with("}\n"),
        "WIT should end with closing brace:\n{stdout}"
    );

    // Verify semicolons after package declaration
    let first_line = stdout.lines().next().unwrap();
    assert!(
        first_line.ends_with(';'),
        "package declaration should end with semicolon: {first_line}"
    );
}
