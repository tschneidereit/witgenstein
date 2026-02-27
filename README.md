# auto-wit — Generate WIT from TypeScript or Rust

This workspace provides two CLI tools for automatically generating [WIT](https://component-model.bytecodealliance.org/design/wit.html) (WebAssembly Interface Type) definitions from source code:

- **ts-auto-wit** — Generate WIT from TypeScript
- **rs-auto-wit** — Generate WIT from Rust

## rs-auto-wit

Generate WIT interface definitions from Rust source code by annotating items with `#[export]`.

### Quick Start

1. Add the macros crate to your project:
   ```toml
   [dependencies]
   rs-auto-wit-macros = { path = "path/to/rs-auto-wit-macros" }
   ```

2. Annotate your exports:
   ```rust
   use rs_auto_wit_macros::export;

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

3. Run the tool:
   ```sh
   rs-auto-wit --package myorg:my-crate@1.0.0 path/to/crate
   ```

### Generated Output

```wit
package myorg:my-crate@1.0.0;

interface types {
  resource counter {
    constructor(initial: u32);
    get: func() -> u32;
    increment: func();
  }
}

world my-crate {
  export types;
  export add: func(a: u32, b: u32) -> u32;
}
```

### CLI Usage

```sh
rs-auto-wit [-o|--output path.wit] [--package namespace:name@version] [--strict] [<input-path>]
```

| Flag | Description |
|---|---|
| `-o, --output` | Output file path. Defaults to stdout. |
| `--package` | WIT package ID (e.g. `myorg:my-pkg@1.0.0`). Defaults to crate name/version. |
| `--strict` | Error on ambiguous types instead of using defaults. |
| `<input-path>` | Crate root directory (must contain `Cargo.toml`). Defaults to CWD. |

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
| struct (named fields) | `record` |
| C-like enum | `enum` |
| enum with data | `variant` |
| `#[export] impl Type` | `resource` |

Smart pointers (`Box`, `Arc`, `Rc`) are transparently unwrapped. `HashMap<K,V>` maps to `list<tuple<K,V>>`.

### Requirements

- Rust toolchain

## ts-auto-wit

Generate WIT from TypeScript source code. See [SPECIFICATION.md](SPECIFICATION.md) for full details.

### CLI Usage

```sh
ts-auto-wit [-o|--output path.wit] [--package namespace:name@version] [--strict] [<input-path>]
```

Uses branded types and decorators to guide type mappings where TypeScript's type system is ambiguous.

## Workspace Structure

```
crates/
  ts-auto-wit/          # TypeScript → WIT tool
  rs-auto-wit/          # Rust → WIT tool
  rs-auto-wit-macros/   # #[export] proc-macro attribute
  wit-common/           # Shared utilities (e.g. to_kebab_case)
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
