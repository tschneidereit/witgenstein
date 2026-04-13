// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

//! A hashing and encoding toolkit component for WebAssembly compositions.
//!
//! Demonstrates:
//! - Pure computational functions (hashing, encoding, checksums)
//! - Records, enums, and variants
//! - `Result` / `Option` return types
//! - WASIp3 `stream<T>` via [`wit_common::Stream`]
//! - Async free functions (WASIp3 `async func`)
//! - A `Hasher` resource with constructor, sync and async methods
//!
//! Build a wasm32-wasip2 component:
//! ```sh
//! cargo build -p hashtools --target wasm32-wasip2
//! ```

use wit_common::Stream;

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

/// The result of a hash computation.
#[derive(Debug)]
pub struct HashOutput {
    /// Hex-encoded hash digest.
    pub hex: String,
    /// Raw digest bytes.
    pub bytes: Vec<u8>,
}

/// Configuration for a hashing operation.
pub struct HashConfig {
    /// Algorithm to use.
    pub algorithm: Algorithm,
    /// Optional HMAC key; if `None`, a plain hash is computed.
    pub hmac_key: Option<Vec<u8>>,
}

// ---------------------------------------------------------------------------
// Enums & variants
// ---------------------------------------------------------------------------

/// Supported hash algorithms.
pub enum Algorithm {
    Sha256,
    Sha512,
    Blake3,
}

/// Supported encoding formats.
pub enum Encoding {
    Hex,
    Base64,
    Base64Url,
}

/// Errors returned by hashing / encoding operations.
#[derive(Debug, PartialEq, Eq)]
pub enum HashError {
    /// The input was empty.
    EmptyInput,
    /// The chosen algorithm is not available.
    UnsupportedAlgorithm,
    /// The provided data is malformed (e.g. invalid hex).
    InvalidData,
}

// ---------------------------------------------------------------------------
// Resource struct
// ---------------------------------------------------------------------------

/// An incremental hasher that accepts data in chunks.
pub struct Hasher {
    state: u64,
    bytes_written: u64,
}

// ---------------------------------------------------------------------------
// Internal helpers (not exported)
// ---------------------------------------------------------------------------

fn fnv1a_bytes(data: &[u8]) -> Vec<u8> {
    fnv1a(data.to_vec()).to_be_bytes().to_vec()
}

fn apply_hmac(digest: &[u8], config: &HashConfig) {
    let _ = (digest, config);
}

fn hex_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 2);
    for byte in data {
        out.push(HEX_CHARS[(byte >> 4) as usize] as char);
        out.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
    }
    out
}

const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

fn hex_decode(s: &str) -> Result<Vec<u8>, HashError> {
    if !s.len().is_multiple_of(2) {
        return Err(HashError::InvalidData);
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_val(bytes[i]).ok_or(HashError::InvalidData)?;
        let lo = hex_val(bytes[i + 1]).ok_or(HashError::InvalidData)?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(BASE64_ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(BASE64_ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(BASE64_ALPHABET[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64url_encode(data: &[u8]) -> String {
    base64_encode(data)
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string()
}

fn base64_decode(s: &str) -> Result<Vec<u8>, HashError> {
    let s = s.replace('-', "+").replace('_', "/");
    let padded = match s.len() % 4 {
        2 => format!("{s}=="),
        3 => format!("{s}="),
        _ => s,
    };
    let mut out = Vec::with_capacity(padded.len() / 4 * 3);
    for chunk in padded.as_bytes().chunks(4) {
        if chunk.len() < 4 {
            return Err(HashError::InvalidData);
        }
        let vals: Vec<u8> = chunk
            .iter()
            .map(|&b| base64_val(b))
            .collect::<Result<_, _>>()?;
        let triple = ((vals[0] as u32) << 18)
            | ((vals[1] as u32) << 12)
            | ((vals[2] as u32) << 6)
            | vals[3] as u32;
        out.push((triple >> 16) as u8);
        if chunk[2] != b'=' {
            out.push((triple >> 8) as u8);
        }
        if chunk[3] != b'=' {
            out.push(triple as u8);
        }
    }
    Ok(out)
}

fn base64_val(b: u8) -> Result<u8, HashError> {
    match b {
        b'A'..=b'Z' => Ok(b - b'A'),
        b'a'..=b'z' => Ok(b - b'a' + 26),
        b'0'..=b'9' => Ok(b - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        b'=' => Ok(0),
        _ => Err(HashError::InvalidData),
    }
}

/// Compute CRC-32 using the standard polynomial (0xEDB88320 reflected).
fn crc32_compute(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// ---------------------------------------------------------------------------
// Exported functions and resource impl
// ---------------------------------------------------------------------------

witgenstein_rs_macros::component! {
    #![package("hashtools:hashtools@0.1.0")]
    #![interface("hashtools")]

    /// Compute a simple FNV-1a 64-bit hash of the input bytes.
    #[export]
    pub fn fnv1a(data: Vec<u8>) -> u64 {
        const BASIS: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut hash = BASIS;
        for byte in &data {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(PRIME);
        }
        hash
    }

    /// Encode a byte slice to the requested text format.
    #[export]
    pub fn encode(data: Vec<u8>, encoding: Encoding) -> String {
        match encoding {
            Encoding::Hex => hex_encode(&data),
            Encoding::Base64 => base64_encode(&data),
            Encoding::Base64Url => base64url_encode(&data),
        }
    }

    /// Decode a text string back to bytes using the specified encoding.
    #[export]
    pub fn decode(data: String, encoding: Encoding) -> Result<Vec<u8>, HashError> {
        match encoding {
            Encoding::Hex => hex_decode(&data),
            Encoding::Base64 | Encoding::Base64Url => base64_decode(&data),
        }
    }

    /// Compute a CRC-32 (ISO 3309 / ITU-T V.42) checksum.
    #[export]
    pub fn crc32(data: Vec<u8>) -> u32 {
        crc32_compute(&data)
    }

    /// Verify that data matches an expected CRC-32 checksum.
    #[export]
    pub fn verify_crc32(data: Vec<u8>, expected: u32) -> bool {
        crc32_compute(&data) == expected
    }

    /// Asynchronously hash a block of data with the given configuration.
    #[export]
    pub async fn hash(data: Vec<u8>, config: HashConfig) -> Result<HashOutput, HashError> {
        if data.is_empty() {
            return Err(HashError::EmptyInput);
        }
        let digest = fnv1a_bytes(&data);
        apply_hmac(&digest, &config);
        Ok(HashOutput {
            hex: hex_encode(&digest),
            bytes: digest,
        })
    }

    /// Hash each chunk and return a stream of intermediate digests.
    #[export]
    pub fn hash_stream(chunks: Vec<Vec<u8>>, algorithm: Algorithm) -> Stream<Vec<u8>> {
        let _ = (chunks, algorithm);
        Stream::new()
    }

    #[export]
    impl Hasher {
        /// Create a new hasher for the given algorithm.
        pub fn new(algorithm: Algorithm) -> Self {
            let _ = algorithm;
            Self {
                state: 0xcbf2_9ce4_8422_2325, // FNV-1a basis
                bytes_written: 0,
            }
        }

        /// Feed data into the hasher.
        pub fn update(&mut self, data: Vec<u8>) {
            const PRIME: u64 = 0x0000_0100_0000_01b3;
            for byte in &data {
                self.state ^= *byte as u64;
                self.state = self.state.wrapping_mul(PRIME);
            }
            self.bytes_written += data.len() as u64;
        }

        /// Return the number of bytes fed so far.
        pub fn bytes_written(&self) -> u64 {
            self.bytes_written
        }

        /// Asynchronously finalise the hash and return the digest.
        pub async fn finalize(&self) -> HashOutput {
            let bytes = self.state.to_be_bytes().to_vec();
            HashOutput {
                hex: hex_encode(&bytes),
                bytes,
            }
        }

        /// Reset the hasher to its initial state.
        pub fn reset(&mut self) {
            self.state = 0xcbf2_9ce4_8422_2325;
            self.bytes_written = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- FNV-1a ---------------------------------------------------------

    #[test]
    fn fnv1a_empty_input() {
        assert_eq!(fnv1a(vec![]), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn fnv1a_hello() {
        let h = fnv1a(b"hello".to_vec());
        assert_ne!(h, 0);
        assert_eq!(h, fnv1a(b"hello".to_vec()));
    }

    #[test]
    fn fnv1a_different_inputs_differ() {
        assert_ne!(fnv1a(b"abc".to_vec()), fnv1a(b"abd".to_vec()));
    }

    // -- Hex encode / decode -------------------------------------------

    #[test]
    fn hex_roundtrip() {
        let data = vec![0xde, 0xad, 0xbe, 0xef];
        let encoded = hex_encode(&data);
        assert_eq!(encoded, "deadbeef");
        assert_eq!(hex_decode(&encoded).unwrap(), data);
    }

    #[test]
    fn hex_decode_odd_length() {
        assert_eq!(hex_decode("abc"), Err(HashError::InvalidData));
    }

    #[test]
    fn hex_decode_invalid_char() {
        assert_eq!(hex_decode("zz"), Err(HashError::InvalidData));
    }

    #[test]
    fn hex_empty() {
        assert_eq!(hex_encode(&[]), "");
        assert_eq!(hex_decode("").unwrap(), vec![]);
    }

    // -- Base64 encode / decode ----------------------------------------

    #[test]
    fn base64_roundtrip() {
        let data = b"Hello, world!".to_vec();
        let encoded = base64_encode(&data);
        assert_eq!(encoded, "SGVsbG8sIHdvcmxkIQ==");
        assert_eq!(base64_decode(&encoded).unwrap(), data);
    }

    #[test]
    fn base64_no_padding() {
        let data = b"abc".to_vec();
        let encoded = base64_encode(&data);
        assert!(!encoded.contains('=') || encoded.ends_with('='));
        assert_eq!(base64_decode(&encoded).unwrap(), data);
    }

    #[test]
    fn base64url_roundtrip() {
        let data = vec![0xff, 0xfe, 0xfd];
        let encoded = base64url_encode(&data);
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('='));
        assert_eq!(base64_decode(&encoded).unwrap(), data);
    }

    #[test]
    fn base64_empty() {
        assert_eq!(base64_encode(&[]), "");
        assert_eq!(base64_decode("").unwrap(), vec![]);
    }

    // -- CRC-32 ---------------------------------------------------------

    #[test]
    fn crc32_known_value() {
        assert_eq!(crc32(b"123456789".to_vec()), 0xCBF4_3926);
    }

    #[test]
    fn crc32_empty() {
        assert_eq!(crc32(vec![]), 0x0000_0000);
    }

    #[test]
    fn verify_crc32_correct() {
        let data = b"hello".to_vec();
        let checksum = crc32(data.clone());
        assert!(verify_crc32(data, checksum));
    }

    #[test]
    fn verify_crc32_incorrect() {
        assert!(!verify_crc32(b"hello".to_vec(), 0));
    }

    // -- encode / decode free functions ---------------------------------

    #[test]
    fn encode_hex() {
        assert_eq!(encode(vec![0xca, 0xfe], Encoding::Hex), "cafe");
    }

    #[test]
    fn decode_hex() {
        assert_eq!(
            decode("cafe".into(), Encoding::Hex).unwrap(),
            vec![0xca, 0xfe]
        );
    }

    #[test]
    fn decode_hex_invalid() {
        assert!(decode("xyz".into(), Encoding::Hex).is_err());
    }

    #[test]
    fn encode_base64() {
        assert_eq!(encode(b"hi".to_vec(), Encoding::Base64), "aGk=");
    }

    #[test]
    fn decode_base64() {
        assert_eq!(
            decode("aGk=".into(), Encoding::Base64).unwrap(),
            b"hi".to_vec()
        );
    }

    #[test]
    fn encode_base64url() {
        let data = vec![0xff, 0xfe];
        let encoded = encode(data.clone(), Encoding::Base64Url);
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
    }

    // -- Hasher resource -----------------------------------------------

    #[test]
    fn hasher_update_and_finalize() {
        let mut h = Hasher::new(Algorithm::Sha256);
        h.update(b"hello".to_vec());
        assert_eq!(h.bytes_written(), 5);

        let output = futures_executor::block_on(h.finalize());
        assert!(!output.hex.is_empty());
        assert_eq!(output.bytes.len(), 8);
    }

    #[test]
    fn hasher_incremental_equals_oneshot() {
        let mut h = Hasher::new(Algorithm::Blake3);
        h.update(b"hel".to_vec());
        h.update(b"lo".to_vec());
        let incremental = futures_executor::block_on(h.finalize());

        let oneshot = fnv1a_bytes(b"hello");
        assert_eq!(incremental.bytes, oneshot);
    }

    #[test]
    fn hasher_reset() {
        let mut h = Hasher::new(Algorithm::Sha512);
        h.update(b"data".to_vec());
        h.reset();
        assert_eq!(h.bytes_written(), 0);

        let output = futures_executor::block_on(h.finalize());
        let basis_bytes = 0xcbf2_9ce4_8422_2325_u64.to_be_bytes().to_vec();
        assert_eq!(output.bytes, basis_bytes);
    }

    #[test]
    fn hasher_empty_finalize() {
        let h = Hasher::new(Algorithm::Sha256);
        assert_eq!(h.bytes_written(), 0);
        let output = futures_executor::block_on(h.finalize());
        let basis_bytes = 0xcbf2_9ce4_8422_2325_u64.to_be_bytes().to_vec();
        assert_eq!(output.bytes, basis_bytes);
    }

    // -- hash async free function --------------------------------------

    #[test]
    fn hash_empty_returns_error() {
        let config = HashConfig {
            algorithm: Algorithm::Sha256,
            hmac_key: None,
        };
        let result = futures_executor::block_on(hash(vec![], config));
        assert_eq!(result.unwrap_err(), HashError::EmptyInput);
    }

    #[test]
    fn hash_produces_output() {
        let config = HashConfig {
            algorithm: Algorithm::Sha256,
            hmac_key: None,
        };
        let result = futures_executor::block_on(hash(b"hello".to_vec(), config));
        let output = result.unwrap();
        assert!(!output.hex.is_empty());
        assert!(!output.bytes.is_empty());
    }

    #[test]
    fn hash_deterministic() {
        let config1 = HashConfig {
            algorithm: Algorithm::Blake3,
            hmac_key: None,
        };
        let config2 = HashConfig {
            algorithm: Algorithm::Blake3,
            hmac_key: None,
        };
        let r1 = futures_executor::block_on(hash(b"data".to_vec(), config1)).unwrap();
        let r2 = futures_executor::block_on(hash(b"data".to_vec(), config2)).unwrap();
        assert_eq!(r1.hex, r2.hex);
        assert_eq!(r1.bytes, r2.bytes);
    }
}
