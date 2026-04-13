// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Integration tests for witgenstein-rs-macros.
//!
//! These tests build fixture crates for `wasm32-wasip2` and validate the
//! generated components using `wasm-tools` and `wasmtime`.

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn target_dir() -> PathBuf {
    workspace_root().join("target/wasm32-wasip2/debug")
}

fn build_fixture(name: &str) -> String {
    let output = Command::new("cargo")
        .arg("build")
        .arg("-p")
        .arg(name)
        .arg("--target")
        .arg("wasm32-wasip2")
        .current_dir(workspace_root())
        .output()
        .unwrap_or_else(|e| panic!("failed to run cargo build for {name}: {e}"));

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        output.status.success(),
        "cargo build -p {name} failed:\n{stderr}"
    );
    stderr
}

fn wasm_path(crate_name: &str) -> PathBuf {
    // Cargo uses underscores in output filenames.
    target_dir().join(format!("{}.wasm", crate_name.replace('-', "_")))
}

fn component_wit(crate_name: &str) -> String {
    let path = wasm_path(crate_name);
    let output = Command::new("wasm-tools")
        .arg("component")
        .arg("wit")
        .arg(&path)
        .output()
        .expect("wasm-tools not found; install via `cargo install wasm-tools`");

    assert!(
        output.status.success(),
        "wasm-tools failed on {}:\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn wasmtime_invoke(crate_name: &str, func_call: &str) -> String {
    let path = wasm_path(crate_name);
    let output = Command::new("wasmtime")
        .arg("run")
        .arg("-W")
        .arg("component-model-async=y")
        .arg("--invoke")
        .arg(func_call)
        .arg(&path)
        .output()
        .expect("wasmtime not found; install from https://wasmtime.dev");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        output.status.success(),
        "wasmtime invoke {func_call} failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    stdout.trim().to_string()
}

// ---------------------------------------------------------------------------
// component-basic fixture
// ---------------------------------------------------------------------------

#[test]
fn basic_builds_for_wasm() {
    build_fixture("component-basic");
    assert!(wasm_path("component-basic").is_file());
}

#[test]
fn basic_exports_interface() {
    build_fixture("component-basic");
    let wit = component_wit("component-basic");
    assert!(
        wit.contains("export component:pkg/exports;"),
        "missing default export:\n{wit}"
    );
}

#[test]
fn basic_has_functions() {
    build_fixture("component-basic");
    let wit = component_wit("component-basic");
    assert!(
        wit.contains("add: func(a: u32, b: u32) -> u32;"),
        "missing add:\n{wit}"
    );
    assert!(
        wit.contains("greet: func(name: string) -> string;"),
        "missing greet:\n{wit}"
    );
}

#[test]
fn basic_runtime_add() {
    build_fixture("component-basic");
    assert_eq!(wasmtime_invoke("component-basic", "add(3, 4)"), "7");
}

#[test]
fn basic_runtime_greet() {
    build_fixture("component-basic");
    assert_eq!(
        wasmtime_invoke("component-basic", "greet(\"World\")"),
        "\"Hello, World!\""
    );
}

// ---------------------------------------------------------------------------
// component-types fixture — auto-discovery tests
// ---------------------------------------------------------------------------

#[test]
fn types_builds_for_wasm() {
    build_fixture("component-types");
    assert!(wasm_path("component-types").is_file());
}

#[test]
fn types_auto_discovers_record_from_signature() {
    // Point has no #[export] — it appears in WIT because distance() uses it.
    build_fixture("component-types");
    let wit = component_wit("component-types");
    assert!(wit.contains("record point"), "missing record point:\n{wit}");
}

#[test]
fn types_auto_discovers_enum_from_signature() {
    // Color has no #[export] — it appears in WIT because color_name() uses it.
    build_fixture("component-types");
    let wit = component_wit("component-types");
    assert!(wit.contains("enum color"), "missing enum color:\n{wit}");
}

#[test]
fn types_has_resource() {
    build_fixture("component-types");
    let wit = component_wit("component-types");
    assert!(
        wit.contains("resource counter"),
        "missing resource counter:\n{wit}"
    );
}

#[test]
fn types_runtime_color_name() {
    build_fixture("component-types");
    assert_eq!(
        wasmtime_invoke("component-types", "color-name(%red)"),
        "\"red\""
    );
}

#[test]
fn types_runtime_try_parse() {
    build_fixture("component-types");
    assert_eq!(
        wasmtime_invoke("component-types", "try-parse(\"42\")"),
        "ok(42)"
    );
}

#[test]
fn types_runtime_maybe_double() {
    build_fixture("component-types");
    assert_eq!(
        wasmtime_invoke("component-types", "maybe-double(some(5))"),
        "some(10)"
    );
}

// ---------------------------------------------------------------------------
// hashtools sample
// ---------------------------------------------------------------------------

#[test]
fn hashtools_builds_for_wasm() {
    build_fixture("hashtools");
    assert!(wasm_path("hashtools").is_file());
}

#[test]
fn hashtools_exports_custom_interface() {
    build_fixture("hashtools");
    let wit = component_wit("hashtools");
    assert!(
        wit.contains("export hashtools:hashtools/hashtools@0.1.0;"),
        "missing custom export:\n{wit}"
    );
}

#[test]
fn hashtools_has_resource() {
    build_fixture("hashtools");
    let wit = component_wit("hashtools");
    assert!(
        wit.contains("resource hasher"),
        "missing hasher resource:\n{wit}"
    );
}

#[test]
fn hashtools_has_records_and_enums() {
    build_fixture("hashtools");
    let wit = component_wit("hashtools");
    assert!(
        wit.contains("record hash-output"),
        "missing record hash-output:\n{wit}"
    );
    assert!(
        wit.contains("enum algorithm"),
        "missing enum algorithm:\n{wit}"
    );
    assert!(
        wit.contains("enum encoding"),
        "missing enum encoding:\n{wit}"
    );
}

#[test]
fn hashtools_runtime_crc32() {
    build_fixture("hashtools");
    assert_eq!(
        wasmtime_invoke("hashtools", "crc32([104, 101, 108, 108, 111])"),
        "907060870"
    );
}

#[test]
fn hashtools_runtime_encode_hex() {
    build_fixture("hashtools");
    assert_eq!(
        wasmtime_invoke("hashtools", "encode([72, 101, 108, 108, 111], %hex)"),
        "\"48656c6c6f\""
    );
}

#[test]
fn hashtools_runtime_decode_hex() {
    build_fixture("hashtools");
    assert_eq!(
        wasmtime_invoke("hashtools", "decode(\"48656c6c6f\", %hex)"),
        "ok([72, 101, 108, 108, 111])"
    );
}

#[test]
fn hashtools_runtime_verify_crc32() {
    build_fixture("hashtools");
    assert_eq!(
        wasmtime_invoke(
            "hashtools",
            "verify-crc32([104, 101, 108, 108, 111], 907060870)"
        ),
        "true"
    );
}

// ---------------------------------------------------------------------------
// textkit sample
// ---------------------------------------------------------------------------

#[test]
fn textkit_builds_for_wasm() {
    build_fixture("textkit");
    assert!(wasm_path("textkit").is_file());
}

#[test]
fn textkit_exports_custom_interface() {
    build_fixture("textkit");
    let wit = component_wit("textkit");
    assert!(
        wit.contains("export textkit:textkit/textkit@0.1.0;"),
        "missing custom export:\n{wit}"
    );
}

#[test]
fn textkit_has_resource() {
    build_fixture("textkit");
    let wit = component_wit("textkit");
    assert!(
        wit.contains("resource string-builder"),
        "missing string-builder resource:\n{wit}"
    );
}

#[test]
fn textkit_has_records_and_enums() {
    build_fixture("textkit");
    let wit = component_wit("textkit");
    assert!(
        wit.contains("record pad-config"),
        "missing pad-config:\n{wit}"
    );
    assert!(
        wit.contains("record split-result"),
        "missing split-result:\n{wit}"
    );
    assert!(
        wit.contains("enum case-style"),
        "missing case-style:\n{wit}"
    );
    assert!(wit.contains("enum pad-side"), "missing pad-side:\n{wit}");
    assert!(
        wit.contains("enum text-error"),
        "missing text-error:\n{wit}"
    );
}

#[test]
fn textkit_transitive_type_discovery() {
    // PadSide has no #[export] and is not directly used in any exported function
    // signature. It appears in WIT because PadConfig (used by pad()) contains
    // a PadSide field — demonstrating transitive auto-discovery.
    build_fixture("textkit");
    let wit = component_wit("textkit");
    assert!(
        wit.contains("enum pad-side"),
        "PadSide should be transitively discovered via PadConfig:\n{wit}"
    );
    // Verify the record references the enum.
    assert!(
        wit.contains("side: pad-side"),
        "pad-config should reference pad-side:\n{wit}"
    );
}

#[test]
fn textkit_runtime_reverse() {
    build_fixture("textkit");
    assert_eq!(
        wasmtime_invoke("textkit", "reverse(\"hello\")"),
        "\"olleh\""
    );
}

#[test]
fn textkit_runtime_convert_case() {
    build_fixture("textkit");
    assert_eq!(
        wasmtime_invoke("textkit", "convert-case(\"hello world\", %title)"),
        "\"Hello World\""
    );
}

#[test]
fn textkit_runtime_char_count() {
    build_fixture("textkit");
    assert_eq!(wasmtime_invoke("textkit", "char-count(\"hello\")"), "5");
}

#[test]
fn textkit_runtime_contains() {
    build_fixture("textkit");
    assert_eq!(
        wasmtime_invoke("textkit", "contains(\"hello world\", \"world\")"),
        "true"
    );
}

#[test]
fn textkit_runtime_trim() {
    build_fixture("textkit");
    assert_eq!(
        wasmtime_invoke("textkit", "trim(\"  hello  \")"),
        "\"hello\""
    );
}
