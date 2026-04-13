// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! A text processing toolkit component for WebAssembly compositions.
//!
//! Demonstrates:
//! - Pure string manipulation functions
//! - Records and enums
//! - `Result` / `Option` return types
//! - A `StringBuilder` resource with incremental building
//!
//! Build a wasm32-wasip2 component:
//! ```sh
//! cargo build -p textkit --target wasm32-wasip2
//! ```

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Which side to pad a string on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PadSide {
    Left,
    Right,
    Both,
}

/// Case conversion style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseStyle {
    Upper,
    Lower,
    Title,
}

/// Errors that text operations can produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextError {
    InvalidIndex,
    PatternNotFound,
    EmptyInput,
}

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

/// The result of a `split` operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitResult {
    pub parts: Vec<String>,
    pub count: u32,
}

/// Configuration for padding.
pub struct PadConfig {
    pub width: u32,
    pub fill: char,
    pub side: PadSide,
}

// ---------------------------------------------------------------------------
// Resource struct
// ---------------------------------------------------------------------------

/// An incremental string builder.
pub struct StringBuilder {
    buf: String,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn title_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for c in s.chars() {
        if c.is_whitespace() {
            capitalize_next = true;
            result.push(c);
        } else if capitalize_next {
            result.extend(c.to_uppercase());
            capitalize_next = false;
        } else {
            result.extend(c.to_lowercase());
            capitalize_next = false;
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Exported functions and resource impl
// ---------------------------------------------------------------------------

witgenstein_rs_macros::component! {
    #![package("textkit:textkit@0.1.0")]
    #![interface("textkit")]

    /// Reverse a string.
    #[export]
    pub fn reverse(input: String) -> String {
        input.chars().rev().collect()
    }

    /// Convert a string to the specified case.
    #[export]
    pub fn convert_case(input: String, style: CaseStyle) -> String {
        match style {
            CaseStyle::Upper => input.to_uppercase(),
            CaseStyle::Lower => input.to_lowercase(),
            CaseStyle::Title => title_case(&input),
        }
    }

    /// Truncate a string to `max_len` characters, appending `suffix` if
    /// truncated.
    #[export]
    pub fn truncate(input: String, max_len: u32, suffix: String) -> String {
        let max = max_len as usize;
        if input.chars().count() <= max {
            return input;
        }
        let truncated: String = input.chars().take(max).collect();
        format!("{truncated}{suffix}")
    }

    /// Pad a string according to `config`.
    #[export]
    pub fn pad(input: String, config: PadConfig) -> String {
        let width = config.width as usize;
        let len = input.chars().count();
        if len >= width {
            return input;
        }
        let fill = config.fill;
        let diff = width - len;
        match config.side {
            PadSide::Left => {
                let padding: String = std::iter::repeat_n(fill, diff).collect();
                format!("{padding}{input}")
            }
            PadSide::Right => {
                let padding: String = std::iter::repeat_n(fill, diff).collect();
                format!("{input}{padding}")
            }
            PadSide::Both => {
                let left = diff / 2;
                let right = diff - left;
                let lp: String = std::iter::repeat_n(fill, left).collect();
                let rp: String = std::iter::repeat_n(fill, right).collect();
                format!("{lp}{input}{rp}")
            }
        }
    }

    /// Split a string by a delimiter.
    #[export]
    pub fn split(input: String, delimiter: String) -> SplitResult {
        let parts: Vec<String> = input.split(&delimiter).map(String::from).collect();
        let count = parts.len() as u32;
        SplitResult { parts, count }
    }

    /// Join a list of strings with a separator.
    #[export]
    pub fn join(parts: Vec<String>, separator: String) -> String {
        parts.join(&separator)
    }

    /// Check whether `input` contains `pattern`.
    #[export]
    pub fn contains(input: String, pattern: String) -> bool {
        input.contains(&pattern)
    }

    /// Replace all occurrences of `pattern` with `replacement` in `input`.
    #[export]
    pub fn replace_all(input: String, pattern: String, replacement: String) -> String {
        input.replace(&pattern, &replacement)
    }

    /// Extract a substring. Returns an error if the indices are out of bounds.
    #[export]
    pub fn substring(input: String, start: u32, end: u32) -> Result<String, TextError> {
        let chars: Vec<char> = input.chars().collect();
        let s = start as usize;
        let e = end as usize;
        if s > chars.len() || e > chars.len() || s > e {
            return Err(TextError::InvalidIndex);
        }
        Ok(chars[s..e].iter().collect())
    }

    /// Count occurrences of `pattern` in `input`.
    #[export]
    pub fn count(input: String, pattern: String) -> u32 {
        if pattern.is_empty() {
            return 0;
        }
        input.matches(&pattern).count() as u32
    }

    /// Trim whitespace from both ends.
    #[export]
    pub fn trim(input: String) -> String {
        input.trim().to_string()
    }

    /// Return the character length of a string.
    #[export]
    pub fn char_count(input: String) -> u32 {
        input.chars().count() as u32
    }

    #[export]
    #[allow(clippy::new_without_default)]
    impl StringBuilder {
        pub fn new() -> Self {
            Self { buf: String::new() }
        }

        /// Append a string.
        pub fn append(&mut self, text: String) {
            self.buf.push_str(&text);
        }

        /// Append a string followed by a newline.
        pub fn append_line(&mut self, text: String) {
            self.buf.push_str(&text);
            self.buf.push('\n');
        }

        /// Insert text at a character position. Returns error on invalid index.
        pub fn insert(&mut self, index: u32, text: String) -> Result<(), TextError> {
            let idx = index as usize;
            let byte_pos = self
                .buf
                .char_indices()
                .nth(idx)
                .map(|(i, _)| i)
                .unwrap_or(self.buf.len());
            if idx > self.buf.chars().count() {
                return Err(TextError::InvalidIndex);
            }
            self.buf.insert_str(byte_pos, &text);
            Ok(())
        }

        /// Clear the builder.
        pub fn clear(&mut self) {
            self.buf.clear();
        }

        /// Return the current length in characters.
        pub fn len(&self) -> u32 {
            self.buf.chars().count() as u32
        }

        /// Check if the builder is empty.
        pub fn is_empty(&self) -> bool {
            self.buf.is_empty()
        }

        /// Return the built string.
        pub fn build(&self) -> String {
            self.buf.clone()
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests — run on native target via `cargo test -p textkit`
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverse() {
        assert_eq!(reverse("hello".into()), "olleh");
    }

    #[test]
    fn test_reverse_empty() {
        assert_eq!(reverse(String::new()), "");
    }

    #[test]
    fn test_reverse_unicode() {
        assert_eq!(reverse("café".into()), "éfac");
    }

    #[test]
    fn test_convert_case_upper() {
        assert_eq!(convert_case("hello".into(), CaseStyle::Upper), "HELLO");
    }

    #[test]
    fn test_convert_case_lower() {
        assert_eq!(convert_case("HELLO".into(), CaseStyle::Lower), "hello");
    }

    #[test]
    fn test_convert_case_title() {
        assert_eq!(
            convert_case("hello world".into(), CaseStyle::Title),
            "Hello World"
        );
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hi".into(), 10, "...".into()), "hi");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("hello world".into(), 5, "...".into()), "hello...");
    }

    #[test]
    fn test_pad_left() {
        let cfg = PadConfig {
            width: 8,
            fill: '0',
            side: PadSide::Left,
        };
        assert_eq!(pad("42".into(), cfg), "00000042");
    }

    #[test]
    fn test_pad_right() {
        let cfg = PadConfig {
            width: 6,
            fill: '.',
            side: PadSide::Right,
        };
        assert_eq!(pad("hi".into(), cfg), "hi....");
    }

    #[test]
    fn test_pad_both() {
        let cfg = PadConfig {
            width: 7,
            fill: '-',
            side: PadSide::Both,
        };
        assert_eq!(pad("hi".into(), cfg), "--hi---");
    }

    #[test]
    fn test_split() {
        let r = split("a,b,c".into(), ",".into());
        assert_eq!(r.parts, vec!["a", "b", "c"]);
        assert_eq!(r.count, 3);
    }

    #[test]
    fn test_join() {
        let parts = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(join(parts, ", ".into()), "a, b, c");
    }

    #[test]
    fn test_contains_true() {
        assert!(contains("hello world".into(), "world".into()));
    }

    #[test]
    fn test_contains_false() {
        assert!(!contains("hello".into(), "xyz".into()));
    }

    #[test]
    fn test_replace_all() {
        assert_eq!(replace_all("aabaa".into(), "a".into(), "x".into()), "xxbxx");
    }

    #[test]
    fn test_substring_ok() {
        assert_eq!(substring("hello".into(), 1, 4), Ok("ell".into()));
    }

    #[test]
    fn test_substring_err() {
        assert!(substring("hi".into(), 0, 10).is_err());
    }

    #[test]
    fn test_count_pattern() {
        assert_eq!(count("banana".into(), "an".into()), 2);
    }

    #[test]
    fn test_count_empty_pattern() {
        assert_eq!(count("hello".into(), String::new()), 0);
    }

    #[test]
    fn test_trim() {
        assert_eq!(trim("  hello  ".into()), "hello");
    }

    #[test]
    fn test_char_count() {
        assert_eq!(char_count("café".into()), 4);
    }

    #[test]
    fn test_string_builder() {
        let mut sb = StringBuilder::new();
        sb.append("hello".into());
        sb.append(" ".into());
        sb.append("world".into());
        assert_eq!(sb.len(), 11);
        assert_eq!(sb.build(), "hello world");
    }

    #[test]
    fn test_string_builder_insert() {
        let mut sb = StringBuilder::new();
        sb.append("helloworld".into());
        sb.insert(5, " ".into()).unwrap();
        assert_eq!(sb.build(), "hello world");
    }

    #[test]
    fn test_string_builder_insert_invalid() {
        let mut sb = StringBuilder::new();
        sb.append("hi".into());
        assert!(sb.insert(100, "x".into()).is_err());
    }

    #[test]
    fn test_string_builder_clear() {
        let mut sb = StringBuilder::new();
        sb.append("stuff".into());
        sb.clear();
        assert_eq!(sb.len(), 0);
        assert_eq!(sb.build(), "");
    }

    #[test]
    fn test_string_builder_append_line() {
        let mut sb = StringBuilder::new();
        sb.append_line("line1".into());
        sb.append_line("line2".into());
        assert_eq!(sb.build(), "line1\nline2\n");
    }
}
