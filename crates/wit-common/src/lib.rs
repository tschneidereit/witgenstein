// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! Shared utilities for WIT generation tools.
//!
//! This crate provides common functions used by both `ts-auto-wit` and
//! `rs-auto-wit`, such as identifier case conversion and WIT naming validation.

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
}
