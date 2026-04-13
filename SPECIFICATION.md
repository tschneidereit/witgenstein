# witgenstein-ts / witgenstein-rs-macros — Generate WIT from TypeScript or Rust

## witgenstein-ts

Using the ComponentizeJS tool, it's possible to generate bindings for WIT interfaces, presented as imports or requiring content to export items.

It's currently not possible to go the other way around: write TypeScript code and generate WIT from it. `witgenstein-ts` changes that: it can read a TypeScript project and generate a WIT file representing the entry module's exported interface as well as the required external imports.

Where the TypeScript types by themselves aren't precise enough, **branded types** and **decorators** can be used to provide the intended mappings:
- **Branded types** (for functions, type aliases, interfaces): import types like `u32`, `s64` from the `witgenstein-ts` package, or define local branded types matching the pattern `type u32 = number & { __brand: 'u32' }`.
- **Decorators** (for classes): use `@wit.resource`, `@wit.flags` etc. on class declarations.

The tool will print out any missing mappings with scaffolded code that can be added and filled in to make WIT generation succeed.

### Implementation

`witgenstein-ts` is a Rust CLI tool, built on the OXC set of JS/TS tools, such as https://crates.io/crates/oxc_parser. It's highly efficient by virtue of minimizing allocations, duplication of parsing and other high-effort processes, and using streaming processing wherever possible, e.g. through the use of `oxc_parser`'s visitor pattern support.

### Usage

```sh
witgenstein-ts [-o|--output path-to-output.wit] [--package namespace:name@version] [--strict] [<input-path>]
```

- `-o|--output`: path to the output `.wit` file. If omitted, the generated WIT is printed to stdout.
- `--package`: WIT package identifier (e.g. `myorg:my-pkg@1.0.0`). If omitted, derived from `package.json` (`@scope/name@version` → `scope:name@version`).
- `--strict`: error on ambiguous types (e.g. bare `number`) instead of using defaults.
- `<input-path>`: if omitted, the TS project in the CWD will be used. If a directory, the TS project in that directory will be used. If a file, that file is used as the top-level TS source file.

### Entry Point Resolution

When the input is a directory (or CWD):
1. Check `package.json` for `main`, `exports`, or `types` fields
2. Check `tsconfig.json` for `files` or `include`
3. Fall back to `index.ts` or `src/index.ts`

### Module Following

The tool follows local imports (relative paths like `./foo`, `../bar`) from the entry file to resolve all type definitions. External imports (bare specifiers) that are used in the public API surface are auto-detected as WIT imports — no explicit annotation needed.

### Type Mappings

#### Direct Mappings
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
| `AsyncIterable<T>` | `stream<T>` | WASIp3 stream |
| `ReadableStream<T>` | `stream<T>` | WASIp3 stream |
| `async function` | `async func` | WASIp3 async |

#### Typed Array Mappings
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

#### Branded Type Overrides
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

#### Result Pattern
The tool recognizes `Result<T, E>` patterns and maps them to WIT `result<T, E>`. Both branded `Result` types and union patterns like `{ ok: T } | { err: E }` are supported.

#### Generics
Only built-in generic mappings are supported:
- `Array<T>` → `list<T>`
- `Promise<T>` → `future<T>`
- `AsyncIterable<T>` → `stream<T>`
- `ReadableStream<T>` → `stream<T>`

User-defined generic types produce an error.

#### Identifier Convention
All identifiers are automatically converted from `camelCase` to `kebab-case` for WIT.

#### Error Types
The following TypeScript types produce errors (no WIT equivalent):
- `any`, `unknown`
- Function types (no first-class functions in WIT)
- User-defined generic types

### Diagnostics

When the tool encounters an unmappable type, it reports the source location and provides scaffolded fix code showing how to add the appropriate branded type or decorator.

---

## witgenstein-rs-macros — Generate WIT Components from Rust via Proc Macro

`witgenstein-rs-macros` provides the `component!` proc macro that generates WIT and `wit_bindgen` glue code at compile time. Users wrap their exported items in `component! { ... }` and mark them with `#[export]`, then build with `cargo build --target wasm32-wasip2`.

### Usage

```rust
witgenstein_rs_macros::component! {
    #![package("myorg:my-crate@1.0.0")]
    #![interface("my-api")]

    #[export]
    pub fn add(a: u32, b: u32) -> u32 { a + b }
}
```

Types (structs, enums) live **outside** the macro — the macro scans source files
and discovers them automatically:

```rust
pub struct Counter { value: u32 }

witgenstein_rs_macros::component! {
    #[export]
    impl Counter {
        pub fn new(initial: u32) -> Self { Self { value: initial } }
        pub fn get(&self) -> u32 { self.value }
        pub fn increment(&mut self) { self.value += 1; }
    }
}
```

### Configuration

Inner attributes configure the WIT package and interface:

- `#![package("ns:name@version")]` — WIT package identity. Default: `component:pkg`.
- `#![interface("name")]` — WIT interface name. Default: `exports`.

Both are optional. The world is always named `component-world`.

### Architecture

The macro works in four phases at compile time:

#### Phase 1: Extraction (`extract.rs`)
Parses items inside the `component!` body using `syn` to discover:
- `#[export] fn name(...)` — standalone exported functions
- `#[export] impl Type { ... }` — resource types with constructors, methods, and static functions

**Source-scanning type discovery:** Before processing the macro body, the macro reads the crate's source files (via `CARGO_MANIFEST_DIR/src/`) and collects all struct and enum definitions. Types therefore do **not** need to be inside `component!` — they can live anywhere in the crate and are automatically included in the WIT interface when referenced (directly or transitively) by an exported function's signature. Types placed inside the macro still work and take precedence over external definitions of the same name.

Type resolution is purely AST-based (syn parsing of source files, no I/O beyond reading `.rs` files).

#### Phase 2: WIT Generation (`wit.rs`)
Generates an inline WIT string from the extracted types:
- Package declaration from `#![package(...)]` config
- Single interface from `#![interface(...)]` config
- All type definitions, resources, and functions in that interface
- A `component-world` that exports the interface

#### Phase 3: Glue Code Generation (`glue.rs`)
Generates Rust code that bridges user types to `wit_bindgen`:
- `wit_bindgen::generate!({ inline: "...", world: "component-world" })` invocation
- Bidirectional conversion functions between user types and wit-bindgen bindings types
- `Guest` trait implementation delegating to user functions
- `GuestResource` trait implementations for resources
- `RefCell` wrappers for interior mutability (WIT methods always take `&self`)

#### Phase 4: Emission (`lib.rs`)
Re-emits original items (with `#[export]` stripped) plus the glue wrapped in:
```rust
#[cfg(target_arch = "wasm32")]
mod __witgenstein_glue { ... }
```
This ensures native `cargo test` works without `wit_bindgen` runtime dependencies.

### Type Mappings

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
| `wit_common::Stream<T>` | `stream<T>` |
| `wit_common::Future<T>` | `future<T>` |
| `async fn` | `async func` |
| struct with named fields | `record` |
| C-like enum | `enum` |
| enum with data variants | `variant` |
| `#[export] impl Type { ... }` | `resource` |

### Named Type Conversions
wit-bindgen generates its own Rust types for records, enums, and variants in the bindings module (e.g., `bindings::exports::...::Algorithm`). These are distinct from the user types (e.g., `Algorithm`). The macro automatically generates bidirectional conversion functions for every named type that appears in function signatures:
- **Parameters**: bindings type → user type (before calling user code)
- **Returns**: user type → bindings type (after receiving the result)
- Conversions are recursive: a `Result<HashOutput, HashError>` wraps `.map()` and `.map_err()` with nested conversions.

### Async Functions (WASIp3)
Functions marked `async` produce `async func` in the WIT. The glue generates `async fn` signatures in the `Guest` trait impl and appends `.await` to user function calls. wit-bindgen 0.55+ handles the async ABI lowering automatically.

### Stream Return Types (WASIp3)
Functions returning `wit_common::Stream<T>` produce `stream<T>` in WIT. The glue creates a stream pair via `wit_stream::new()` and returns the reader half.

### Interior Mutability for Resources
WIT resource methods always produce `&self` in the wit-bindgen generated Rust traits. The macro wraps each user resource in a `RefCell` to bridge this:
- `&self` methods use `borrow()`
- `&mut self` methods use `borrow_mut()`

### The `#[export]` Attribute

The `#[export]` attribute marks items for WIT export. It is consumed by `component!` — on its own it is an identity transform.

Supported targets:
- `#[export] pub fn name(...)` — standalone functions
- `#[export] impl Type { ... }` — resources

### Identifier Convention
All identifiers are automatically converted from `snake_case` to `kebab-case` for WIT.

## Workspace Structure

- `crates/witgenstein-ts/` — TypeScript → WIT CLI tool
- `crates/witgenstein-rs-macros/` — `component!` proc-macro
- `crates/wit-common/` — shared utilities + `Stream<T>` / `Future<T>` marker types
- `samples/rust/hashtools/` — sample Rust component
