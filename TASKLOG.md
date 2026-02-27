# Task Log

## ts-auto-wit Implementation

- [x] **Design implementation** (2026-02-27): Designed the full implementation plan with phases, module structure, type mapping decisions, and WIT output strategy. See SPECIFICATION.md for design decisions.
- [x] **Implement core modules** (2026-02-27): Created all source modules: cli.rs, types.rs, project.rs, module_graph.rs, branded.rs, collect.rs, mapper.rs, emit.rs, diagnostic.rs, main.rs.
- [x] **Fix OXC 0.115.0 API compilation errors** (2026-02-27): Fixed 15+ compilation errors caused by OXC API differences (PropertyKey instead of TSPropertySignatureKey, BindingPattern enum instead of struct, TSEnumDeclaration.body.members, type_arguments not type_parameters, SourceType::ts(), TsconfigDiscovery wrapping, Rust 2024 ref pattern changes, etc.)
- [x] **Pass cargo fmt + clippy** (2026-02-27): Applied clippy auto-fixes (collapsible_if, useless_format, single_match, unnecessary_filter_map). All remaining warnings are dead_code for fields/variants reserved for future use.
- [x] **Create test fixtures** (2026-02-27): Created 5 test fixture projects: basic, branded, resource, async_result, multi_module — each with TypeScript source and package.json.
- [x] **Write and pass integration tests** (2026-02-27): Wrote 9 integration tests covering basic functions/records/enums, branded types, resources/classes, async/result types, multi-module, explicit --package flag, and WIT syntax validation. All 12 tests pass (3 unit + 9 integration).
- [x] **Default output to stdout** (2026-02-27): Changed default behavior to print WIT to stdout when `--output` is not specified. Writing to a file now requires explicit `--output <path>`.

## rs-auto-wit Implementation

- [x] **Restructure workspace** (2026-02-27): Converted from single-crate to Cargo workspace with 4 members: ts-auto-wit, rs-auto-wit, rs-auto-wit-macros, wit-common. Extracted `to_kebab_case` into wit-common shared crate.
- [x] **Implement core modules** (2026-02-27): Created all rs-auto-wit source modules: cli.rs (clap CLI), discover.rs (syn-based #[export] scanning), resolve.rs (rustdoc JSON type resolution), mapper.rs (Rust→WIT type mapping via wit-encoder), emit.rs (WIT package emission), diagnostic.rs, main.rs (pipeline).
- [x] **Implement rs-auto-wit-macros** (2026-02-27): Created identity proc-macro `#[export]` attribute with zero runtime cost.
- [x] **Fix compilation errors** (2026-02-27): Fixed ~40 compilation errors from rustdoc-types and wit-encoder API mismatches. Researched exact APIs by reading source code. Key fixes: `FunctionSignature` not `FnDecl`, `path.path` not `path.name`, `StandaloneFunc::new(name, async_)`, `Resource::empty()`, `TypeDef::new(name, kind)`, `Ident::new` requires `'static`/owned strings, `Package` implements `Display`.
- [x] **Pass cargo fmt + clippy** (2026-02-27): Fixed clippy warnings (collapsible if-let chains, `&Path` instead of `&PathBuf`).
- [x] **Create test fixture** (2026-02-27): Created basic-crate fixture with functions (add, greet, sum_list, try_parse, maybe_greet) and Counter resource with constructor/methods.
- [x] **Write and pass integration tests** (2026-02-27): 14 tests passing (3 unit + 11 integration) covering package declaration, exported functions, type mappings (u32, string, list, option, result), resource with constructor/methods, versioned packages, world generation.
- [x] **Update documentation** (2026-02-27): Updated SPECIFICATION.md with rs-auto-wit section, created README.md for combined workspace, updated TASKLOG.md.
- [x] **Use RUSTC_BOOTSTRAP=1 for stable/beta support** (2026-02-27): Replaced `cargo +nightly rustdoc` with `cargo rustdoc` + `RUSTC_BOOTSTRAP=1` env var, following the cargo-semver-checks approach. No longer requires nightly toolchain.
