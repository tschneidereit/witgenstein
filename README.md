# auto-wit — Generate WIT from TypeScript or Rust

This workspace provides two CLI tools for automatically generating [WIT](https://component-model.bytecodealliance.org/design/wit.html) (WebAssembly Interface Type) definitions from source code:

- **witgenstein-rs** — Generate WIT from Rust, and build WASM components
- **witgenstein-ts** — Generate WIT from TypeScript

## witgenstein-rs

Generate WIT interface definitions from Rust source code by annotating items with `#[export]`.

### Quick Start

1. Add the macros crate to your project:
   ```toml
   [dependencies]
   witgenstein-rs-macros = { path = "path/to/witgenstein-rs-macros" }
   ```

2. Annotate your exports:
   ```rust
   use witgenstein_rs_macros::export;

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

3. Generate WIT:
   ```sh
   witgenstein-rs generate --package myorg:my-crate@1.0.0 path/to/crate
   ```

   Or build a wasm32-wasip2 component in a single step:
   ```sh
   witgenstein-rs build --package myorg:my-crate@1.0.0 path/to/crate
   ```

### Generated Output

```wit
package myorg:my-crate@1.0.0;

interface my-crate {
  resource counter {
    constructor(initial: u32);
    get: func() -> u32;
    increment: func();
  }

  add: func(a: u32, b: u32) -> u32;
}

world my-crate-world {
  export my-crate;
}
```

### CLI Usage

#### `generate` — Generate WIT

```sh
witgenstein-rs generate [-o|--output path.wit] [--package namespace:name@version] [--strict] [-v|--verbose] [<input-path>]
```

| Flag | Description |
|---|---|
| `-o, --output` | Output file path. Defaults to stdout. |
| `--package` | WIT package ID (e.g. `myorg:my-pkg@1.0.0`). Defaults to crate name/version. |
| `--strict` | Error on ambiguous types instead of using defaults. |
| `-v, --verbose` | Enable verbose output (e.g. list discovered exports). |
| `<input-path>` | Crate root directory (must contain `Cargo.toml`). Defaults to CWD. |

#### `build` — Build a WASM Component

```sh
witgenstein-rs build [--package namespace:name@version] [--release] [-v|--verbose] [<input-path>]
```

| Flag | Description |
|---|---|
| `--package` | WIT package ID (e.g. `myorg:my-pkg@1.0.0`). Defaults to crate name/version. |
| `--release` | Build in release mode. |
| `-v, --verbose` | Enable verbose output (e.g. list discovered exports). |
| `<input-path>` | Crate root directory (must contain `Cargo.toml` and `src/lib.rs`). Defaults to CWD. |

Discovers `#[export]` annotations, generates WIT, creates a wrapper crate, and compiles a `wasm32-wasip2` component — all in a single invocation. Output is placed at `target/witgenstein/build/wasm32-wasip2/{debug|release}/`.

Requires the `wasm32-wasip2` target: `rustup target add wasm32-wasip2`.

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
| struct | `record` |
| C-like enum | `enum` |
| enum with data | `variant` |
| `#[export] impl Type` | `resource` |

Smart pointers (`Box`, `Arc`, `Rc`) are transparently unwrapped. `HashMap<K,V>` maps to `list<tuple<K,V>>`.

### Requirements

- Rust toolchain

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
  witgenstein-rs/          # Rust → WIT tool
  witgenstein-rs-macros/   # #[export] proc-macro attribute
  wit-common/              # Shared utilities (e.g. to_kebab_case, parse_wit_package_id)
```

## Building

```sh
cargo build --workspace
```

## Testing

```sh
cargo test --workspace
```

## License

Apache-2.0 WITH LLVM-exception
