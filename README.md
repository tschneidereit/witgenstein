# WITGenstein — Automatic Component Interface Generation

This workspace provides tools for automatically generating [WIT](https://component-model.bytecodealliance.org/design/wit.html) (WebAssembly Interface Type) definitions from source code:

- **witgenstein-rs-macros** — Proc-macro for Rust: wrap your code in `component!` and `cargo build --target wasm32-wasip2` just works
- **witgenstein-ts** — CLI tool for TypeScript → WIT generation

## witgenstein-rs-macros

Generate WIT and compile WASM components from Rust with zero external tooling — just `cargo build`.

### Quick Start

1. Add the macros crate and wit-bindgen to your project:
   ```toml
   [lib]
   crate-type = ["cdylib", "rlib"]

   [dependencies]
   witgenstein-rs-macros = { path = "path/to/witgenstein-rs-macros" }
   wit-bindgen = "0.55"
   ```

2. Define your types normally, then wrap your exports in the `component!` macro:
   ```rust
   // Types live outside the macro — the macro scans source files
   // and automatically discovers them from function signatures.
   pub struct Point { pub x: f64, pub y: f64 }

   pub struct Counter { value: u32 }

   witgenstein_rs_macros::component! {
       #![package("myorg:my-crate@1.0.0")]
       #![interface("my-api")]

       #[export]
       pub fn origin() -> Point { Point { x: 0.0, y: 0.0 } }

       #[export]
       impl Counter {
           pub fn new(initial: u32) -> Self { Self { value: initial } }
           pub fn get(&self) -> u32 { self.value }
           pub fn increment(&mut self) { self.value += 1; }
       }
   }
   ```

3. Build the component:
   ```sh
   cargo build --target wasm32-wasip2
   ```

### Configuration

Use inner attributes inside `component!` to configure the WIT output:

| Attribute | Description | Default |
|---|---|---|
| `#![package("ns:name@ver")]` | WIT package identity | `component:pkg` |
| `#![interface("name")]` | WIT interface name | `exports` |

Both are optional.

### Generated WIT

For the example above, the macro generates inline WIT equivalent to:

```wit
package myorg:my-crate@1.0.0;

interface my-api {
  record point {
    x: f64,
    y: f64,
  }

  resource counter {
    constructor(initial: u32);
    get: func() -> u32;
    increment: func();
  }

  origin: func() -> point;
}

world component-world {
  export my-api;
}
```

Note that `point` appears in the interface automatically because it's used as
the return type of `origin` — no `#[export]` was needed on the struct.

### Type Mappings

| Rust | WIT |
|---|---|
| `bool` | `bool` |
| `u8`..`u64`, `i8`..`i64` | `u8`..`u64`, `s8`..`s64` |
| `f32`, `f64` | `f32`, `f64` |
| `char` | `char` |
| `String`, `&str` | `string` |
| `Vec<T>`, `&[T]` | `list<T>` |
| `Option<T>` | `option<T>` |
| `Result<T, E>` | `result<T, E>` |
| `(T1, T2)` | `tuple<T1, T2>` |
| `wit_common::Stream<T>` | `stream<T>` |
| `wit_common::Future<T>` | `future<T>` |
| `async fn` | `async func` |
| struct | `record` |
| C-like enum | `enum` |
| enum with data | `variant` |
| `#[export] impl Type` | `resource` |

### How It Works

The `component!` macro:
1. Parses items with `syn` to discover `#[export]` annotations
2. **Scans the crate's source files** to find struct and enum definitions used (directly or transitively) by exported function signatures — types need no annotation and can live anywhere in the crate
3. Generates inline WIT from the type information
4. Emits `wit_bindgen::generate!` with the inline WIT
5. Generates a `Guest` trait implementation bridging your code to the component ABI
6. Wraps the glue in `#[cfg(target_arch = "wasm32")]` so native `cargo test` works normally

### Requirements

- Rust toolchain with `wasm32-wasip2` target: `rustup target add wasm32-wasip2`
- `wit-bindgen = "0.55"` as a dependency

## witgenstein-ts

Generate WIT from TypeScript source code. See [SPECIFICATION.md](SPECIFICATION.md) for full details.

### CLI Usage

```sh
witgenstein-ts [-o|--output path.wit] [--package namespace:name@version] [--strict] [<input-path>]
```

Uses branded types and decorators to guide type mappings where TypeScript's type system is ambiguous.

## Workspace Structure

```
crates/
  witgenstein-ts/          # TypeScript → WIT tool
  witgenstein-rs-macros/   # component! proc-macro
  wit-common/              # Shared utilities + Stream<T>/Future<T> marker types
samples/
  rust/hashtools/          # Rust hashing/encoding toolkit (async, streams, WASIp3)
```

## Samples

### Rust — `samples/rust/hashtools`

A hashing and encoding toolkit with pure computational functions (FNV-1a,
CRC-32, hex, base64), async hashing, and a `Hasher` resource with incremental
updates. Demonstrates `stream<T>` return types via `wit_common::Stream<T>`.

```sh
# Build a wasm32-wasip2 component
cargo build -p hashtools --target wasm32-wasip2

# Run unit tests on native
cargo test -p hashtools
```

Sample generated WIT (abbreviated):
```wit
package hashtools:hashtools@0.1.0;

interface hashtools {
  enum algorithm { sha256, sha512, blake3 }
  enum encoding { hex, base64, base64-url }
  enum hash-error { empty-input, unsupported-algorithm, invalid-data }
  record hash-output { hex: string, bytes: list<u8> }
  record hash-config { algorithm: algorithm, hmac-key: option<list<u8>> }

  resource hasher {
    constructor(algorithm: algorithm);
    update: func(data: list<u8>);
    bytes-written: func() -> u64;
    finalize: async func() -> hash-output;
    reset: func();
  }

  fnv1a: func(data: list<u8>) -> u64;
  crc32: func(data: list<u8>) -> u32;
  encode: func(data: list<u8>, encoding: encoding) -> string;
  decode: func(data: string, encoding: encoding) -> result<list<u8>, hash-error>;
  hash: async func(data: list<u8>, config: hash-config) -> result<hash-output, hash-error>;
  hash-stream: func(chunks: list<list<u8>>, algorithm: algorithm) -> stream<list<u8>>;
}

world component-world { export hashtools; }
```

## Building

```sh
cargo build --workspace
```

## Testing

```sh
cargo test --workspace
```

Integration tests build fixture crates for `wasm32-wasip2` and validate them
with `wasm-tools` and `wasmtime` (runtime invocation of exported functions).

Prerequisites for the full test suite:
- `rustup target add wasm32-wasip2`
- [wasmtime](https://wasmtime.dev/) CLI
- [wasm-tools](https://github.com/bytecodealliance/wasm-tools) CLI

## CI

A GitHub Actions workflow (`.github/workflows/ci.yml`) runs on every push and
PR: `cargo fmt --check`, `cargo clippy`, and `cargo test --workspace`.

## License

Apache-2.0 WITH LLVM-exception
