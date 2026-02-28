# witgenstein-ts / witgenstein-rs — Generate WIT from TypeScript or Rust

Using the ComponentizeJS tool, it's possible to generate bindings for WIT interfaces, presented as imports or requiring content to export items.

It's currently not possible to go the other way around: write TypeScript code and generate WIT from it. `witgenstein-ts` changes that: it can read a TypeScript project and generate a WIT file representing the entry module's exported interface as well as the required external imports.

Where the TypeScript types by themselves aren't precise enough, **branded types** and **decorators** can be used to provide the intended mappings:
- **Branded types** (for functions, type aliases, interfaces): import types like `u32`, `s64` from the `witgenstein-ts` package, or define local branded types matching the pattern `type u32 = number & { __brand: 'u32' }`.
- **Decorators** (for classes): use `@wit.resource`, `@wit.flags` etc. on class declarations.

The tool will print out any missing mappings with scaffolded code that can be added and filled in to make WIT generation succeed.

## Implementation

`witgenstein-ts` is a Rust CLI tool, built on the OXC set of JS/TS tools, such as https://crates.io/crates/oxc_parser. It's highly efficient by virtue of minimizing allocations, duplication of parsing and other high-effort processes, and using streaming processing wherever possible, e.g. through the use of `oxc_parser`'s visitor pattern support.

## Usage

`witgenstein-ts` has a simple CLI interface:
```sh
witgenstein-ts [-o|--output path-to-output.wit] [--package namespace:name@version] [--strict] [<input-path>]
```

- `-o|--output`: path to the output `.wit` file. If omitted, the generated WIT is printed to stdout.
- `--package`: WIT package identifier (e.g. `myorg:my-pkg@1.0.0`). If omitted, derived from `package.json` (`@scope/name@version` → `scope:name@version`).
- `--strict`: error on ambiguous types (e.g. bare `number`) instead of using defaults.
- `<input-path>`: if omitted, the TS project in the CWD will be used. If a directory, the TS project in that directory will be used. If a file, that file is used as the top-level TS source file.

## Entry Point Resolution

When the input is a directory (or CWD):
1. Check `package.json` for `main`, `exports`, or `types` fields
2. Check `tsconfig.json` for `files` or `include`
3. Fall back to `index.ts` or `src/index.ts`

## Module Following

The tool follows local imports (relative paths like `./foo`, `../bar`) from the entry file to resolve all type definitions. External imports (bare specifiers) that are used in the public API surface are auto-detected as WIT imports — no explicit annotation needed.

## Type Mappings

### Direct Mappings
| TypeScript | WIT | Notes |
|---|---|---|
| `string` | `string` | |
| `boolean` | `bool` | |
| `number` | `f64` | Default; error in `--strict` mode |
| `bigint` | `s64` | Default |
| `void` | *(no return type)* | |
| `T[]` / `Array<T>` | `list<T>` | |
| `[T1, T2]` | `tuple<T1, T2>` | |
| `T \| null` / `T \| undefined` / `T?` | `option<T>` | |
| `interface` / object literal type | `record` | |
| `class` | `resource` | With `@wit.resource` decorator |
| `enum` (string/no-value) | `enum` | |
| `enum` with `@wit.flags` | `flags` | |
| discriminated union | `variant` | |
| `Promise<T>` | `future<T>` | |
| `async function` | `async func` | WASIp3 async |

### Typed Array Mappings
| TypeScript | WIT |
|---|---|
| `Uint8Array` | `list<u8>` |
| `Uint16Array` | `list<u16>` |
| `Uint32Array` | `list<u32>` |
| `Int8Array` | `list<s8>` |
| `Int16Array` | `list<s16>` |
| `Int32Array` | `list<s32>` |
| `Float32Array` | `list<f32>` |
| `Float64Array` | `list<f64>` |
| `BigInt64Array` | `list<s64>` |
| `BigUint64Array` | `list<u64>` |

### Branded Type Overrides
Users disambiguate numeric types via branded types, either imported from the `witgenstein-ts` package or defined locally:
```typescript
import { u32, s16 } from 'witgenstein-ts';
export function process(count: u32): s16 { ... }
```
Or locally:
```typescript
type u32 = number & { __brand: 'u32' };
```
Recognized branded names: `u8`, `u16`, `u32`, `u64`, `s8`, `s16`, `s32`, `s64`, `f32`, `f64`, `char`.

### Result Pattern
The tool recognizes `Result<T, E>` patterns and maps them to WIT `result<T, E>`. Both branded `Result` types and union patterns like `{ ok: T } | { err: E }` are supported.

### Generics
Only built-in generic mappings are supported:
- `Array<T>` → `list<T>`
- `Promise<T>` → `future<T>`

User-defined generic types produce an error.

### Identifier Convention
All identifiers are automatically converted from `camelCase` to `kebab-case` for WIT.

### Error Types
The following TypeScript types produce errors (no WIT equivalent):
- `any`, `unknown`
- Function types (no first-class functions in WIT)
- User-defined generic types

## Diagnostics

When the tool encounters an unmappable type, it reports the source location and provides scaffolded fix code showing how to add the appropriate branded type or decorator.

---

# witgenstein-rs — Generate WIT from Rust

`witgenstein-rs` is a companion tool that generates WIT interface definitions from Rust source code. It uses `#[export]` annotations from the `witgenstein-rs-macros` crate to mark which items should be exposed in the WIT interface.

## Usage

`witgenstein-rs` has two subcommands: `generate` for WIT generation, and `build` for compiling a wasm32-wasip2 component.

### `generate` — Generate WIT

```sh
witgenstein-rs generate [-o|--output path-to-output.wit] [--package namespace:name@version] [--strict] [<input-path>]
```

- `-o|--output`: path to the output `.wit` file. If omitted, the generated WIT is printed to stdout.
- `--package`: WIT package identifier (e.g. `myorg:my-pkg@1.0.0`). If omitted, derived from `Cargo.toml` (crate name and version).
- `--strict`: error on ambiguous types instead of using defaults.
- `<input-path>`: crate root directory (must contain `Cargo.toml`). Defaults to CWD.

### `build` — Build a WASM Component

```sh
witgenstein-rs build [--package namespace:name@version] [--release] [<input-path>]
```

- `--package`: WIT package identifier (e.g. `myorg:my-pkg@1.0.0`). If omitted, derived from `Cargo.toml`.
- `--release`: build in release mode.
- `<input-path>`: crate root directory (must contain `Cargo.toml` and `src/lib.rs`). Defaults to CWD.

The `build` command is a single self-contained invocation that:
1. Discovers `#[export]` annotations
2. Resolves types via rustdoc JSON
3. Generates the WIT interface
4. Creates a wrapper crate at `target/witgenstein/component/`
5. Builds it with `cargo build --target wasm32-wasip2`
6. Produces a `.wasm` component at `target/witgenstein/build/wasm32-wasip2/{debug|release}/`

**Requirements:** The `wasm32-wasip2` target must be installed (`rustup target add wasm32-wasip2`).

## Architecture

The tool works in four phases:

### Phase 1: Discovery (`discover.rs`)
Scans `.rs` source files using `syn` to find items annotated with `#[export]`:
- `#[export] fn foo()` — standalone exported functions
- `#[export] impl MyType { ... }` — resource types with constructors, methods, and static functions

Follows `mod` declarations to scan submodules.

### Phase 2: Resolution (`resolve.rs`)
Invokes `cargo rustdoc --output-format json` (with `RUSTC_BOOTSTRAP=1` to work on stable/beta Rust) on the target crate and parses the rustdoc JSON output using the `rustdoc-types` crate. Resolves full type information for each exported item:
- Function signatures (parameters, return types, async)
- Impl blocks (constructor, methods, static functions)
- Referenced types (structs → records, enums → WIT enums/variants)

### Phase 3: Mapping (`mapper.rs`)
Converts resolved Rust types to `wit-encoder` types:

| Rust | WIT |
|---|---|
| `bool` | `bool` |
| `u8`, `u16`, `u32`, `u64` | `u8`, `u16`, `u32`, `u64` |
| `i8`, `i16`, `i32`, `i64` | `s8`, `s16`, `s32`, `s64` |
| `f32`, `f64` | `f32`, `f64` |
| `char` | `char` |
| `String`, `&str` | `string` |
| `Vec<T>`, `&[T]` | `list<T>` |
| `Option<T>` | `option<T>` |
| `Result<T, E>` | `result<T, E>` |
| `(T1, T2, ...)` | `tuple<T1, T2, ...>` |
| `Box<T>`, `Arc<T>`, `Rc<T>` | `T` (unwrapped) |
| `HashMap<K, V>` | `list<tuple<K, V>>` |
| `HashSet<T>` | `list<T>` |
| struct with named fields | `record` |
| C-like enum | `enum` |
| enum with data variants | `variant` |
| `#[export] impl Type { ... }` | `resource` |

### Phase 4: Emission (`emit.rs`)
Assembles mapped types into a complete WIT package using `wit-encoder`:
- All exports (standalone functions, resources, and type definitions) go into a single interface named after the crate (e.g. `basic-crate` for a crate named `basic-crate`, regardless of the `--package` flag)
- The world is named `{crate-name}-world` (e.g. `basic-crate-world`) to avoid a naming conflict with the interface
- The world re-exports the interface

This ensures that when compiled to a component, all exports are namespaced under the user's package identity (e.g. `myorg:my-crate/basic-crate@1.0.0`) rather than appearing as anonymous top-level exports under the synthetic `root:component` package that the component model binary format produces.

### Identifier Convention
All identifiers are automatically converted from `snake_case` to `kebab-case` for WIT.

### Phase 5: Code Generation (`codegen.rs`) — `build` only
Creates a wrapper crate at `target/witgenstein/component/` that bridges the user's library to a WASM component:

1. **Cargo.toml**: depends on the user's crate and `wit-bindgen = "0.40"`, with `crate-type = ["cdylib"]` and an empty `[workspace]` to prevent parent-workspace membership.
2. **wit/world.wit**: the generated WIT from Phase 4.
3. **src/lib.rs**: uses `wit_bindgen::generate!` to produce guest bindings, then implements the generated `Guest` trait for the interface by delegating to the user's crate. All delegations (standalone functions and resource type associations) are in a single `impl` block for the interface-level `Guest` trait.

#### Interior Mutability for Resources
WIT resource methods always produce `&self` in the wit-bindgen generated Rust traits, regardless of the original method's mutability. The codegen layer wraps each user resource in a `RefCell` to bridge this:

```rust
struct CounterWrapper(std::cell::RefCell<user_crate::Counter>);

impl GuestCounter for CounterWrapper {
    fn new(initial: u32) -> Self {
        Self(std::cell::RefCell::new(user_crate::Counter::new(initial)))
    }
    fn get(&self) -> u32 {
        user_crate::Counter::get(&*self.0.borrow())
    }
    fn increment(&self) {
        user_crate::Counter::increment(&mut *self.0.borrow_mut())
    }
}
```

The `is_mut_self` field on `ResolvedFunction` (detected via `Type::BorrowedRef { is_mutable }` in rustdoc-types) determines whether `borrow()` or `borrow_mut()` is used.

## The `#[export]` Macro

The `witgenstein-macros` crate provides the `#[export]` proc-macro attribute. It is an identity transform — it passes through the annotated item unchanged with zero runtime cost. Its sole purpose is to mark items for WIT generation.

```rust
use witgenstein_macros::export;

#[export]
pub fn add(a: u32, b: u32) -> u32 {
    a + b
}

pub struct Counter { value: u32 }

#[export]
impl Counter {
    pub fn new(initial: u32) -> Self { Self { value: initial } }
    pub fn get(&self) -> u32 { self.value }
    pub fn increment(&mut self) { self.value += 1; }
}
```

## Workspace Structure

The project is organized as a Cargo workspace:
- `crates/witgenstein-ts/` — TypeScript → WIT tool
- `crates/witgenstein/` — Rust → WIT tool
- `crates/witgenstein-macros/` — `#[export]` proc-macro
- `crates/wit-common/` — shared utilities (e.g. `to_kebab_case`)
