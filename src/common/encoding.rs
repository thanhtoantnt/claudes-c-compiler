//! Encoding utilities for handling non-UTF-8 C source files.
//!
//! C source files may contain non-UTF-8 bytes in string/character literals
//! (e.g., EUC-JP, Latin-1 encoded files). Since Rust strings require valid
//! UTF-8, we encode non-UTF-8 bytes using Unicode Private Use Area (PUA)
//! code points, then decode them back to raw bytes in the lexer.
//!
//! Encoding scheme: byte 0x80+n → U+E080+n (PUA range U+E080..U+E0FF)

/// Base PUA code point for encoding non-UTF-8 bytes.
/// Byte 0x80 maps to U+E080, byte 0xFF maps to U+E0FF.
const PUA_BASE: u32 = 0xE080;

/// Convert raw bytes to a valid UTF-8 String.
///
/// If the bytes are valid UTF-8, returns them as-is.
/// Otherwise, processes byte-by-byte: valid UTF-8 sequences are preserved,
/// and invalid bytes 0x80-0xFF are encoded as PUA code points U+E080-U+E0FF.
///
/// A UTF-8 BOM (EF BB BF) at the start of the input is stripped, matching
/// the behavior of GCC and Clang.
pub fn bytes_to_string(bytes: Vec<u8>) -> String {
    // Strip UTF-8 BOM if present at the start of the file
    let bytes = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes[3..].to_vec()
    } else {
        bytes
    };
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => encode_non_utf8(e.into_bytes()),
    }
}

/// Encode a byte slice that contains non-UTF-8 data into a valid String
/// using PUA encoding for non-ASCII bytes that aren't part of valid UTF-8.
fn encode_non_utf8(bytes: Vec<u8>) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            // ASCII byte - pass through directly
            result.push(b as char);
            i += 1;
        } else {
            // Try to decode a valid UTF-8 multi-byte sequence
            let seq_len = utf8_sequence_length(b);
            if seq_len > 1 && i + seq_len <= bytes.len() {
                if let Ok(s) = std::str::from_utf8(&bytes[i..i + seq_len]) {
                    result.push_str(s);
                    i += seq_len;
                    continue;
                }
            }
            // Not a valid UTF-8 sequence - encode as PUA
            result.push(char::from_u32(PUA_BASE + (b - 0x80) as u32).unwrap());
            i += 1;
        }
    }
    result
}

/// Determine the expected length of a UTF-8 sequence from its first byte.
fn utf8_sequence_length(b: u8) -> usize {
    if b < 0xC0 { 1 } // ASCII or continuation byte (invalid as start)
    else if b < 0xE0 { 2 }
    else if b < 0xF0 { 3 }
    else { 4 }
}

/// Decode a byte from the lexer's input, converting PUA-encoded bytes back
/// to their original values. Returns the decoded byte and how many input
/// bytes were consumed.
///
/// If the input at the given position contains a PUA-encoded byte
/// (U+E080..U+E0FF, which is the 3-byte UTF-8 sequence EE 82 80..EE 83 BF),
/// returns the original byte 0x80-0xFF and consumes 3 input bytes.
/// Otherwise, returns the input byte as-is and consumes 1 byte.
pub fn decode_pua_byte(input: &[u8], pos: usize) -> (u8, usize) {
    if pos + 2 < input.len() && input[pos] == 0xEE {
        // PUA U+E080-U+E0FF is encoded in UTF-8 as:
        // U+E080 = EE 82 80
        // U+E0BF = EE 82 BF
        // U+E0C0 = EE 83 80
        // U+E0FF = EE 83 BF
        let b1 = input[pos + 1];
        let b2 = input[pos + 2];
        if b1 == 0x82 && (0x80..=0xBF).contains(&b2) {
            // U+E080..U+E0BF → original byte 0x80..0xBF
            let orig = b2; // 0x80 + (b2 - 0x80) = b2
            return (orig, 3);
        } else if b1 == 0x83 && (0x80..=0xBF).contains(&b2) {
            // U+E0C0..U+E0FF → original byte 0xC0..0xFF
            let orig = 0xC0 + (b2 - 0x80);
            return (orig, 3);
        }
    }
    (input[pos], 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn decode_all_pua_bytes(encoded: &str) -> Vec<u8> {
        let input = encoded.as_bytes();
        let mut output = Vec::with_capacity(input.len());
        let mut pos = 0;
        while pos < input.len() {
            let (byte, consumed) = decode_pua_byte(input, pos);
            output.push(byte);
            pos += consumed;
        }
        output
    }

    fn contains_literal_pua_utf8(bytes: &[u8]) -> bool {
        bytes.windows(3).any(|w| {
            w[0] == 0xEE
                && ((w[1] == 0x82 && (0x80..=0xBF).contains(&w[2]))
                    || (w[1] == 0x83 && (0x80..=0xBF).contains(&w[2])))
        })
    }

    fn reference_decode_pua_byte(input: &[u8], pos: usize) -> (u8, usize) {
        if pos + 2 < input.len() && input[pos] == 0xEE {
            let b1 = input[pos + 1];
            let b2 = input[pos + 2];
            if b1 == 0x82 && (0x80..=0xBF).contains(&b2) {
                return (b2, 3);
            } else if b1 == 0x83 && (0x80..=0xBF).contains(&b2) {
                return (0xC0 + (b2 - 0x80), 3);
            }
        }
        (input[pos], 1)
    }

    #[test]
    fn test_ascii_passthrough() {
        let bytes = b"hello world".to_vec();
        let result = bytes_to_string(bytes);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_valid_utf8_passthrough() {
        let bytes = "こんにちは".as_bytes().to_vec();
        let result = bytes_to_string(bytes);
        assert_eq!(result, "こんにちは");
    }

    #[test]
    fn test_non_utf8_encoding() {
        // EUC-JP byte sequence \xa4\xa2 (hiragana "a")
        let bytes = vec![0xA4, 0xA2];
        let encoded = bytes_to_string(bytes);
        // Should be encoded as PUA characters
        assert!(encoded.is_char_boundary(0));
        assert_eq!(encoded.len(), 6); // Two 3-byte PUA chars

        // Decode back
        let input: Vec<u8> = encoded.bytes().collect();
        let (b0, len0) = decode_pua_byte(&input, 0);
        assert_eq!(b0, 0xA4);
        assert_eq!(len0, 3);
        let (b1, len1) = decode_pua_byte(&input, 3);
        assert_eq!(b1, 0xA2);
        assert_eq!(len1, 3);
    }

    #[test]
    fn test_mixed_ascii_and_non_utf8() {
        let bytes = vec![b'h', b'i', 0xA4, 0xA2, b'!'];
        let encoded = bytes_to_string(bytes);
        let input: Vec<u8> = encoded.bytes().collect();
        // 'h' 'i' PUA PUA '!'
        let (b, l) = decode_pua_byte(&input, 0);
        assert_eq!((b, l), (b'h', 1));
        let (b, l) = decode_pua_byte(&input, 1);
        assert_eq!((b, l), (b'i', 1));
        let (b, l) = decode_pua_byte(&input, 2);
        assert_eq!((b, l), (0xA4, 3));
        let (b, l) = decode_pua_byte(&input, 5);
        assert_eq!((b, l), (0xA2, 3));
        let (b, l) = decode_pua_byte(&input, 8);
        assert_eq!((b, l), (b'!', 1));
    }

    #[test]
    fn test_bom_stripping() {
        // UTF-8 BOM followed by ASCII content
        let bytes = vec![0xEF, 0xBB, 0xBF, b'#', b'i', b'n', b'c'];
        let result = bytes_to_string(bytes);
        assert_eq!(result, "#inc");

        // BOM-only file
        let bytes = vec![0xEF, 0xBB, 0xBF];
        let result = bytes_to_string(bytes);
        assert_eq!(result, "");

        // No BOM - should be unchanged
        let bytes = vec![b'#', b'i', b'n', b'c'];
        let result = bytes_to_string(bytes);
        assert_eq!(result, "#inc");
    }

    #[test]
    fn test_roundtrip_all_bytes() {
        // Test that all byte values 0x80-0xFF round-trip correctly
        for b in 0x80u8..=0xFF {
            let encoded = bytes_to_string(vec![b]);
            let input: Vec<u8> = encoded.bytes().collect();
            let (decoded, _) = decode_pua_byte(&input, 0);
            assert_eq!(decoded, b, "Byte 0x{:02X} failed round-trip", b);
        }
    }

    proptest! {
        // Oracle: Algebraic/Reference — every PUA-encoded byte sequence must
        // decode back to the original byte and consume exactly three bytes.
        #[test]
        fn embedded_pua_sequences_decode_to_original_byte(
            byte in 0x80u8..=0xFF,
            prefix in prop::collection::vec(any::<u8>(), 0..=32),
            suffix in prop::collection::vec(any::<u8>(), 0..=32)
        ) {
            let mut input = prefix;
            let pos = input.len();
            let encoded = char::from_u32(PUA_BASE + (byte - 0x80) as u32).unwrap().to_string();
            input.extend_from_slice(encoded.as_bytes());
            input.extend_from_slice(&suffix);

            let (decoded, consumed) = decode_pua_byte(&input, pos);
            prop_assert_eq!((decoded, consumed), (byte, 3));
        }

        // Oracle: Negative/error contract — byte triples that start with EE but
        // do not match the documented PUA continuation bytes must be treated as
        // ordinary raw bytes.
        #[test]
        fn malformed_ee_sequences_are_passthrough(
            (b1, b2) in (any::<u8>(), any::<u8>())
                .prop_filter("exclude valid PUA encodings", |&(b1, b2)| {
                    !((b1 == 0x82 || b1 == 0x83) && (0x80..=0xBF).contains(&b2))
                }),
            prefix in prop::collection::vec(any::<u8>(), 0..=32),
            suffix in prop::collection::vec(any::<u8>(), 0..=32)
        ) {
            let mut input = prefix;
            let pos = input.len();
            input.extend_from_slice(&[0xEE, b1, b2]);
            input.extend_from_slice(&suffix);

            let (decoded, consumed) = decode_pua_byte(&input, pos);
            prop_assert_eq!((decoded, consumed), (0xEE, 1));
        }

        // Oracle: Reference — the decoder must behave exactly like the documented
        // byte-by-byte specification for arbitrary input windows.
        #[test]
        fn decode_pua_byte_matches_reference_decoder(
            (input, pos) in prop::collection::vec(any::<u8>(), 1..=128)
                .prop_flat_map(|input| {
                    let len = input.len();
                    (Just(input), 0..len)
                })
        ) {
            prop_assert_eq!(
                decode_pua_byte(&input, pos),
                reference_decode_pua_byte(&input, pos)
            );
        }

        // Oracle: State/progress — decoding an entire byte stream from start
        // to finish must always make progress (consumed ∈ {1, 3}) and terminate
        // exactly at input.len(), never overshooting or stalling.
        #[test]
        fn full_stream_decode_terminates_exactly(
            input in prop::collection::vec(any::<u8>(), 0..=256)
        ) {
            let mut pos = 0usize;
            let len = input.len();
            while pos < len {
                let (_, consumed) = decode_pua_byte(&input, pos);
                prop_assert!(
                    consumed == 1 || consumed == 3,
                    "consumed must be 1 or 3, got {}", consumed
                );
                prop_assert!(
                    pos + consumed <= len,
                    "overshoot: pos={} consumed={} len={}", pos, consumed, len
                );
                pos += consumed;
            }
            prop_assert_eq!(pos, len);
        }

        // Oracle: Boundary — a complete PUA sequence occupying exactly the last
        // three bytes of the input (len == pos + 3) must still be recognised,
        // exercising the `pos + 2 < input.len()` guard at its tightest.
        #[test]
        fn complete_pua_sequence_at_exact_end(
            byte in 0x80u8..=0xFF,
            prefix in prop::collection::vec(any::<u8>(), 0..=32)
        ) {
            let mut input = prefix;
            let pos = input.len();
            let enc = char::from_u32(PUA_BASE + (byte - 0x80) as u32)
                .unwrap()
                .to_string();
            input.extend_from_slice(enc.as_bytes());
            prop_assert_eq!(input.len(), pos + 3);
            let (decoded, consumed) = decode_pua_byte(&input, pos);
            prop_assert_eq!((decoded, consumed), (byte, 3));
        }

        // Oracle: Negative contract — any byte other than 0xEE at the cursor
        // must pass through unchanged consuming exactly one byte, regardless of
        // the surrounding bytes.
        #[test]
        fn non_ee_first_byte_always_passthrough(
            first in (any::<u8>()).prop_filter("exclude 0xEE", |&b| b != 0xEE),
            rest in prop::collection::vec(any::<u8>(), 0..=32)
        ) {
            let mut input = vec![first];
            input.extend_from_slice(&rest);
            let (decoded, consumed) = decode_pua_byte(&input, 0);
            prop_assert_eq!((decoded, consumed), (first, 1));
        }

        // Oracle: Algebraic — the consumed length fully determines the shape of
        // the output: consumed==1 ⇒ passthrough (decoded == input[pos]);
        // consumed==3 ⇒ PUA decode (decoded ∈ 0x80..=0xFF).
        #[test]
        fn consumed_length_implies_output_shape(
            (input, pos) in prop::collection::vec(any::<u8>(), 1..=128)
                .prop_flat_map(|input| {
                    let len = input.len();
                    (Just(input), 0..len)
                })
        ) {
            let (decoded, consumed) = decode_pua_byte(&input, pos);
            match consumed {
                1 => prop_assert_eq!(decoded, input[pos]),
                3 => prop_assert!(
                    (0x80u8..=0xFF).contains(&decoded),
                    "PUA-decoded byte must be >= 0x80, got 0x{:02X}", decoded
                ),
                other => prop_assert!(false, "unexpected consumed {}", other),
            }
        }

        // Oracle: Round-trip / negative contract — encode->decode must return
        // the ORIGINAL bytes. The generator deterministically seeds a raw
        // 3-byte UTF-8 sequence in the PUA range U+E080..U+E0FF (which is itself
        // valid UTF-8), surrounded by ASCII. This EXPECTS CORRECT OUTPUT and
        // FAILS, demonstrating the encoder/decoder PUA-range ambiguity bug:
        // `bytes_to_string` passes the valid-UTF-8 PUA triple through unchanged,
        // but `decode_pua_byte` reinterprets it as a PUA-encoded single byte,
        // collapsing 3 bytes into 1.
        // See pbt-out/bug_reports/decode_pua_byte_pua_range_ambiguity.md
        #[test]
        fn raw_pua_range_utf8_does_not_roundtrip(
            cp in 0xE080u32..=0xE0FF,
            prefix in prop::collection::vec(0x00u8..=0x7F, 0..=32),
            suffix in prop::collection::vec(0x00u8..=0x7F, 0..=32)
        ) {
            let pua_bytes = char::from_u32(cp).unwrap().to_string();
            let mut bytes = prefix;
            bytes.extend_from_slice(pua_bytes.as_bytes());
            bytes.extend_from_slice(&suffix);

            let encoded = bytes_to_string(bytes.clone());
            let input: Vec<u8> = encoded.bytes().collect();
            // Encoder passes the valid-UTF-8 PUA triple through verbatim
            // (input == bytes), so any divergence is the decoder's fault.
            assert_eq!(input, bytes, "encoder altered valid-UTF-8 PUA input");

            let mut decoded = Vec::new();
            let mut pos = 0;
            while pos < input.len() {
                let (b, c) = decode_pua_byte(&input, pos);
                decoded.push(b);
                pos += c;
            }
            // Failing assertion: decode collapses EE 82/83 xx into a single
            // byte, losing the other two.
            prop_assert_eq!(decoded, bytes);
        }

        // Oracle: Negative contract — decode must not panic for any position
        // in 0..=len. This EXPECTS no-panic and FAILS, demonstrating that
        // `decode_pua_byte` indexes input[pos] unguarded in its fallback
        // branch and panics at pos == len (e.g. empty input, pos 0).
        // See pbt-out/bug_reports/decode_pua_byte_panics_on_empty_input.md
        #[test]
        fn decode_does_not_panic_at_end_position(
            (bytes, pos) in prop::collection::vec(any::<u8>(), 0..=32)
                .prop_flat_map(|b| {
                    let len = b.len() as u32;
                    (Just(b), 0..=len)
                })
        ) {
            let result = std::panic::catch_unwind(|| decode_pua_byte(&bytes, pos as usize));
            prop_assert!(
                result.is_ok(),
                "decode_pua_byte panicked at pos={} of len={}", pos, bytes.len()
            );
        }
    }
}
