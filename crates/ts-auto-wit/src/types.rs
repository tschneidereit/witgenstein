// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! WIT intermediate representation types.
//!
//! These types represent the WIT constructs that `ts-auto-wit` can generate.
//! They are populated by the type mapper and consumed by the WIT emitter.

use std::fmt;

/// A WIT type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitType {
    // Unsigned integers
    U8,
    U16,
    U32,
    U64,
    // Signed integers
    S8,
    S16,
    S32,
    S64,
    // Floats
    F32,
    F64,
    // Other primitives
    Bool,
    Char,
    WitString,
    // Compound types
    List(Box<WitType>),
    Option(Box<WitType>),
    Result {
        ok: Option<Box<WitType>>,
        err: Option<Box<WitType>>,
    },
    Tuple(Vec<WitType>),
    Future(Option<Box<WitType>>),
    Stream(Option<Box<WitType>>),
    // Reference to a named user-defined type
    Named(String),
    // Handle types
    Own(String),
    Borrow(String),
}

impl fmt::Display for WitType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::U8 => write!(f, "u8"),
            Self::U16 => write!(f, "u16"),
            Self::U32 => write!(f, "u32"),
            Self::U64 => write!(f, "u64"),
            Self::S8 => write!(f, "s8"),
            Self::S16 => write!(f, "s16"),
            Self::S32 => write!(f, "s32"),
            Self::S64 => write!(f, "s64"),
            Self::F32 => write!(f, "f32"),
            Self::F64 => write!(f, "f64"),
            Self::Bool => write!(f, "bool"),
            Self::Char => write!(f, "char"),
            Self::WitString => write!(f, "string"),
            Self::List(inner) => write!(f, "list<{inner}>"),
            Self::Option(inner) => write!(f, "option<{inner}>"),
            Self::Result { ok, err } => match (ok, err) {
                (Some(ok), Some(err)) => write!(f, "result<{ok}, {err}>"),
                (Some(ok), None) => write!(f, "result<{ok}>"),
                (None, Some(err)) => write!(f, "result<_, {err}>"),
                (None, None) => write!(f, "result"),
            },
            Self::Tuple(elems) => {
                write!(f, "tuple<")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem}")?;
                }
                write!(f, ">")
            }
            Self::Future(inner) => match inner {
                Some(t) => write!(f, "future<{t}>"),
                None => write!(f, "future"),
            },
            Self::Stream(inner) => match inner {
                Some(t) => write!(f, "stream<{t}>"),
                None => write!(f, "stream"),
            },
            Self::Named(name) => write!(f, "{name}"),
            Self::Own(name) => write!(f, "own<{name}>"),
            Self::Borrow(name) => write!(f, "borrow<{name}>"),
        }
    }
}

/// A WIT function signature.
#[derive(Debug, Clone)]
pub struct WitFunc {
    pub name: String,
    pub params: Vec<(String, WitType)>,
    pub result: Option<WitType>,
    pub is_async: bool,
}

/// A WIT record (named bag of fields).
#[derive(Debug, Clone)]
pub struct WitRecord {
    pub name: String,
    pub fields: Vec<(String, WitType)>,
}

/// A WIT resource (entity with lifetime, passed by handle).
#[derive(Debug, Clone)]
pub struct WitResource {
    pub name: String,
    pub constructor_params: Option<Vec<(String, WitType)>>,
    pub methods: Vec<WitFunc>,
    pub static_funcs: Vec<WitFunc>,
}

/// A WIT enum (variant with no payloads).
#[derive(Debug, Clone)]
pub struct WitEnum {
    pub name: String,
    pub cases: Vec<String>,
}

/// A WIT variant (tagged union with optional payloads).
#[derive(Debug, Clone)]
pub struct WitVariant {
    pub name: String,
    pub cases: Vec<(String, Option<WitType>)>,
}

/// A WIT flags type (bag-of-bools / bitset).
#[derive(Debug, Clone)]
pub struct WitFlags {
    pub name: String,
    pub flags: Vec<String>,
}

/// A WIT type alias.
#[derive(Debug, Clone)]
pub struct WitTypeAlias {
    pub name: String,
    pub target: WitType,
}

/// Any top-level WIT type definition.
#[derive(Debug, Clone)]
pub enum WitTypeDef {
    Record(WitRecord),
    Resource(WitResource),
    Enum(WitEnum),
    Variant(WitVariant),
    Flags(WitFlags),
    TypeAlias(WitTypeAlias),
}

impl WitTypeDef {
    pub fn name(&self) -> &str {
        match self {
            Self::Record(r) => &r.name,
            Self::Resource(r) => &r.name,
            Self::Enum(e) => &e.name,
            Self::Variant(v) => &v.name,
            Self::Flags(f) => &f.name,
            Self::TypeAlias(a) => &a.name,
        }
    }
}

/// A WIT interface containing type definitions and functions.
#[derive(Debug, Clone)]
pub struct WitInterface {
    pub name: String,
    pub type_defs: Vec<WitTypeDef>,
    pub functions: Vec<WitFunc>,
}

/// A WIT world describing a component's imports and exports.
#[derive(Debug, Clone)]
pub struct WitWorld {
    pub name: String,
    /// Exported interface (types + functions).
    pub exports: WitInterface,
    /// Imported interfaces (from external dependencies).
    pub imports: Vec<WitInterface>,
}

/// A complete WIT package.
#[derive(Debug, Clone)]
pub struct WitPackage {
    pub namespace: String,
    pub name: String,
    pub version: Option<String>,
    pub world: WitWorld,
}
