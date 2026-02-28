// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Shared utilities for WIT generation tools.
//!
//! This crate provides common functions used by both `witgenstein-ts` and
//! `witgenstein`, such as identifier case conversion and WIT naming validation.

/// Convert a camelCase, PascalCase, or snake_case identifier to kebab-case.
///
/// This is the standard naming convention for WIT identifiers.
///
/// # Examples
///
/// ```
/// assert_eq!(wit_common::to_kebab_case("camelCase"), "camel-case");
/// assert_eq!(wit_common::to_kebab_case("PascalCase"), "pascal-case");
/// assert_eq!(wit_common::to_kebab_case("snake_case"), "snake-case");
/// assert_eq!(wit_common::to_kebab_case("HTMLParser"), "html-parser");
/// assert_eq!(wit_common::to_kebab_case("getHTTPResponse"), "get-http-response");
/// ```
pub fn to_kebab_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let mut chars = s.chars().peekable();
    let mut prev_was_upper = false;
    let mut prev_was_separator = false;

    while let Some(c) = chars.next() {
        if c == '_' || c == '-' {
            if !result.is_empty() && !prev_was_separator {
                result.push('-');
            }
            prev_was_separator = true;
            prev_was_upper = false;
            continue;
        }

        if c.is_uppercase() {
            let next_is_lower = chars.peek().is_some_and(|n| n.is_lowercase());

            if !result.is_empty() && !prev_was_separator {
                // Insert hyphen before the start of a new word.
                if !prev_was_upper || next_is_lower {
                    result.push('-');
                }
            }

            result.push(c.to_lowercase().next().unwrap());
            prev_was_upper = true;
        } else {
            result.push(c);
            prev_was_upper = false;
        }
        prev_was_separator = false;
    }

    result
}

/// A parsed WIT package identifier (namespace, name, optional version).
pub type WitPackageId = (String, String, Option<String>);

/// Parse a WIT package identifier string like `myorg:my-pkg@1.0.0`.
///
/// Returns `(namespace, name, optional_version)`.
///
/// # Examples
///
/// ```
/// let (ns, name, ver) = wit_common::parse_wit_package_id("myorg:my-pkg@1.0.0").unwrap();
/// assert_eq!(ns, "myorg");
/// assert_eq!(name, "my-pkg");
/// assert_eq!(ver.as_deref(), Some("1.0.0"));
/// ```
pub fn parse_wit_package_id(s: &str) -> Result<WitPackageId, String> {
    let (rest, version) = if let Some((rest, ver)) = s.rsplit_once('@') {
        (rest, Some(ver.to_string()))
    } else {
        (s, None)
    };

    let (namespace, name) = rest
        .split_once(':')
        .ok_or_else(|| format!("invalid WIT package ID: expected 'namespace:name', got '{s}'"))?;

    Ok((namespace.to_string(), name.to_string(), version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("camelCase"), "camel-case");
        assert_eq!(to_kebab_case("PascalCase"), "pascal-case");
        assert_eq!(to_kebab_case("simpleword"), "simpleword");
        assert_eq!(to_kebab_case("HTMLParser"), "html-parser");
        assert_eq!(to_kebab_case("getHTTPResponse"), "get-http-response");
        assert_eq!(to_kebab_case("snake_case"), "snake-case");
        assert_eq!(to_kebab_case("SCREAMING_SNAKE"), "screaming-snake");
        assert_eq!(to_kebab_case("already-kebab"), "already-kebab");
        assert_eq!(to_kebab_case("a"), "a");
        assert_eq!(to_kebab_case(""), "");
    }

    #[test]
    fn test_parse_wit_package_id_with_version() {
        let (ns, name, ver) = parse_wit_package_id("myorg:my-pkg@1.0.0").unwrap();
        assert_eq!(ns, "myorg");
        assert_eq!(name, "my-pkg");
        assert_eq!(ver.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn test_parse_wit_package_id_no_version() {
        let (ns, name, ver) = parse_wit_package_id("myorg:my-pkg").unwrap();
        assert_eq!(ns, "myorg");
        assert_eq!(name, "my-pkg");
        assert_eq!(ver, None);
    }

    #[test]
    fn test_parse_wit_package_id_invalid() {
        assert!(parse_wit_package_id("invalid").is_err());
    }
}
