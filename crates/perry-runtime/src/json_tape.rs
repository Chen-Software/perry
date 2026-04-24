//! Issue #179 Step 2 Phase 1: tape representation for JSON values.
//!
//! A *tape* is a flat `Vec<TapeEntry>` recording the structural
//! positions of every significant token in a JSON blob: object/array
//! starts and ends, object key positions, and scalar value positions.
//! Each entry carries a byte-offset into the original blob and a
//! lightweight kind tag. Parsing a JSON document to a tape is a
//! single pass with bounded memory (tape size is O(token count),
//! not O(tree size) — closer to 8-16 bytes per token versus the
//! ~80+ bytes per JSValue object the tree representation costs).
//!
//! This module is the foundation for:
//!   Phase 2 — `JSON.parse(x).length` reads tape's top-level array
//!     length directly, no tree materialization
//!   Phase 3 — indexed/property access on a tape-backed value
//!     materializes only the touched subtree
//!   Phase 4 — `JSON.stringify(taped)` on an unmutated tape memcpys
//!     the original blob bytes instead of walking a tree
//!
//! This Phase 1 commit ships the tape builder + a materializer that
//! produces the same `JSValue` tree as the existing `DirectParser`.
//! It is opt-in via the `PERRY_JSON_TAPE=1` env var so production
//! behavior is unchanged. Correctness is verified by running all
//! existing `JSON.parse` tests through both the direct and
//! tape-materialize paths and comparing their `JSON.stringify`
//! output byte-for-byte.
//!
//! The tape+materialize path intentionally performs no better than
//! the direct path (it does strictly more work). The value lands
//! when Phase 2+ intercept access and skip materialization.

use crate::value::JSValue;

/// One tape entry. Kind + byte offset + (for container kinds) a
/// parent/sibling pointer that lets materialization skip over
/// already-traversed subtrees.
#[derive(Debug, Clone, Copy)]
pub struct TapeEntry {
    /// Byte offset into the source blob where this token starts.
    pub offset: u32,
    /// One of the `KIND_*` constants.
    pub kind: u8,
    /// For container kinds (`KIND_OBJ_START` / `KIND_ARR_START`): the
    /// tape index of the matching end marker. Enables O(1) skip-over
    /// during lazy subtree materialization. Zero for leaf kinds.
    pub link: u32,
}

// Tape kinds. 8 bits; ample room for extension (lazy sentinel, hole,
// etc. can be added without widening the struct).
pub const KIND_OBJ_START: u8 = 1;
pub const KIND_OBJ_END: u8 = 2;
pub const KIND_ARR_START: u8 = 3;
pub const KIND_ARR_END: u8 = 4;
pub const KIND_KEY: u8 = 5;
pub const KIND_STRING: u8 = 6;
pub const KIND_NUMBER: u8 = 7;
pub const KIND_TRUE: u8 = 8;
pub const KIND_FALSE: u8 = 9;
pub const KIND_NULL: u8 = 10;

/// The built tape for one JSON document.
pub struct Tape {
    pub entries: Vec<TapeEntry>,
}

/// Build a tape from JSON bytes in one pass. Returns `None` on
/// malformed input (caller should fall through to the direct parser
/// which has richer error reporting).
///
/// The builder walks the input left-to-right and pushes tape entries
/// for every structural token. It does NOT decode strings or numbers
/// — those are deferred to materialization, which lets the tape build
/// pass be byte-scan-only (SIMD-friendly in future revisions) and
/// avoids allocating for values that lazy access will never read.
pub fn build_tape(bytes: &[u8]) -> Option<Tape> {
    // Pre-size: worst case is one tape entry per ~4 bytes of input
    // (single-digit integers in an array), though typical JSON is
    // closer to one per 15-20 bytes. Pre-allocating to len/8 is a
    // reasonable middle.
    let mut entries: Vec<TapeEntry> = Vec::with_capacity(bytes.len() / 8 + 8);
    // Parallel stack of (tape index of the matching OBJ/ARR start).
    // On end-of-container, we pop and backfill the start entry's
    // `link` field with the end's tape index.
    let mut stack: Vec<u32> = Vec::new();
    let mut pos = 0usize;

    // Helper: skip whitespace.
    #[inline(always)]
    fn skip_ws(bytes: &[u8], pos: &mut usize) {
        while *pos < bytes.len() {
            match bytes[*pos] {
                b' ' | b'\t' | b'\n' | b'\r' => *pos += 1,
                _ => break,
            }
        }
    }

    // Helper: skip a JSON string in place (past the closing quote).
    // Returns `true` on success, `false` on EOF before closing quote.
    // Honors `\"`, `\\`, and other escapes by swallowing the character
    // after a backslash. Does NOT decode — just finds the boundary.
    #[inline(always)]
    fn skip_string(bytes: &[u8], pos: &mut usize) -> bool {
        debug_assert_eq!(bytes[*pos], b'"');
        *pos += 1;
        while *pos < bytes.len() {
            let c = bytes[*pos];
            if c == b'"' {
                *pos += 1;
                return true;
            }
            if c == b'\\' {
                *pos += 1;
                if *pos >= bytes.len() { return false; }
                *pos += 1;
            } else {
                *pos += 1;
            }
        }
        false
    }

    // Helper: skip a JSON number (past its last digit/exponent).
    #[inline(always)]
    fn skip_number(bytes: &[u8], pos: &mut usize) {
        if *pos < bytes.len() && bytes[*pos] == b'-' { *pos += 1; }
        while *pos < bytes.len() && bytes[*pos].is_ascii_digit() { *pos += 1; }
        if *pos < bytes.len() && bytes[*pos] == b'.' {
            *pos += 1;
            while *pos < bytes.len() && bytes[*pos].is_ascii_digit() { *pos += 1; }
        }
        if *pos < bytes.len() && (bytes[*pos] == b'e' || bytes[*pos] == b'E') {
            *pos += 1;
            if *pos < bytes.len() && (bytes[*pos] == b'+' || bytes[*pos] == b'-') {
                *pos += 1;
            }
            while *pos < bytes.len() && bytes[*pos].is_ascii_digit() { *pos += 1; }
        }
    }

    // Driver: expecting-value state. After emitting a value, the
    // caller handles the trailing `,` or container end.
    enum State { Value, AfterValue }
    let mut state = State::Value;

    loop {
        skip_ws(bytes, &mut pos);
        if pos >= bytes.len() { break; }
        match state {
            State::Value => {
                let tok_off = pos as u32;
                match bytes[pos] {
                    b'{' => {
                        let idx = entries.len() as u32;
                        entries.push(TapeEntry { offset: tok_off, kind: KIND_OBJ_START, link: 0 });
                        stack.push(idx);
                        pos += 1;
                        skip_ws(bytes, &mut pos);
                        if pos < bytes.len() && bytes[pos] == b'}' {
                            let end_idx = entries.len() as u32;
                            entries.push(TapeEntry { offset: pos as u32, kind: KIND_OBJ_END, link: idx });
                            entries[idx as usize].link = end_idx;
                            stack.pop();
                            pos += 1;
                            state = State::AfterValue;
                        } else {
                            // Expect "key":value,...
                            // Handled by the AfterStart branch below.
                            state = State::Value;
                            // Immediately parse the key.
                            if pos >= bytes.len() || bytes[pos] != b'"' { return None; }
                            let key_off = pos as u32;
                            if !skip_string(bytes, &mut pos) { return None; }
                            entries.push(TapeEntry { offset: key_off, kind: KIND_KEY, link: 0 });
                            skip_ws(bytes, &mut pos);
                            if pos >= bytes.len() || bytes[pos] != b':' { return None; }
                            pos += 1;
                        }
                    }
                    b'[' => {
                        let idx = entries.len() as u32;
                        entries.push(TapeEntry { offset: tok_off, kind: KIND_ARR_START, link: 0 });
                        stack.push(idx);
                        pos += 1;
                        skip_ws(bytes, &mut pos);
                        if pos < bytes.len() && bytes[pos] == b']' {
                            let end_idx = entries.len() as u32;
                            entries.push(TapeEntry { offset: pos as u32, kind: KIND_ARR_END, link: idx });
                            entries[idx as usize].link = end_idx;
                            stack.pop();
                            pos += 1;
                            state = State::AfterValue;
                        } else {
                            state = State::Value;
                        }
                    }
                    b'"' => {
                        if !skip_string(bytes, &mut pos) { return None; }
                        entries.push(TapeEntry { offset: tok_off, kind: KIND_STRING, link: 0 });
                        state = State::AfterValue;
                    }
                    b't' => {
                        if pos + 4 > bytes.len() || &bytes[pos..pos + 4] != b"true" { return None; }
                        entries.push(TapeEntry { offset: tok_off, kind: KIND_TRUE, link: 0 });
                        pos += 4;
                        state = State::AfterValue;
                    }
                    b'f' => {
                        if pos + 5 > bytes.len() || &bytes[pos..pos + 5] != b"false" { return None; }
                        entries.push(TapeEntry { offset: tok_off, kind: KIND_FALSE, link: 0 });
                        pos += 5;
                        state = State::AfterValue;
                    }
                    b'n' => {
                        if pos + 4 > bytes.len() || &bytes[pos..pos + 4] != b"null" { return None; }
                        entries.push(TapeEntry { offset: tok_off, kind: KIND_NULL, link: 0 });
                        pos += 4;
                        state = State::AfterValue;
                    }
                    c if c == b'-' || c.is_ascii_digit() => {
                        skip_number(bytes, &mut pos);
                        entries.push(TapeEntry { offset: tok_off, kind: KIND_NUMBER, link: 0 });
                        state = State::AfterValue;
                    }
                    _ => return None,
                }
            }
            State::AfterValue => {
                if stack.is_empty() {
                    // Top-level value consumed; trailing whitespace is OK.
                    break;
                }
                // Look at which container we're in.
                let top_idx = *stack.last().unwrap();
                let top_kind = entries[top_idx as usize].kind;
                match bytes[pos] {
                    b',' => {
                        pos += 1;
                        if top_kind == KIND_OBJ_START {
                            // Expect next key.
                            skip_ws(bytes, &mut pos);
                            if pos >= bytes.len() || bytes[pos] != b'"' { return None; }
                            let key_off = pos as u32;
                            if !skip_string(bytes, &mut pos) { return None; }
                            entries.push(TapeEntry { offset: key_off, kind: KIND_KEY, link: 0 });
                            skip_ws(bytes, &mut pos);
                            if pos >= bytes.len() || bytes[pos] != b':' { return None; }
                            pos += 1;
                        }
                        state = State::Value;
                    }
                    b'}' if top_kind == KIND_OBJ_START => {
                        let end_idx = entries.len() as u32;
                        entries.push(TapeEntry { offset: pos as u32, kind: KIND_OBJ_END, link: top_idx });
                        entries[top_idx as usize].link = end_idx;
                        stack.pop();
                        pos += 1;
                        state = State::AfterValue;
                    }
                    b']' if top_kind == KIND_ARR_START => {
                        let end_idx = entries.len() as u32;
                        entries.push(TapeEntry { offset: pos as u32, kind: KIND_ARR_END, link: top_idx });
                        entries[top_idx as usize].link = end_idx;
                        stack.pop();
                        pos += 1;
                        state = State::AfterValue;
                    }
                    _ => return None,
                }
            }
        }
    }

    if !stack.is_empty() { return None; } // unclosed container
    if entries.is_empty() { return None; }
    Some(Tape { entries })
}

/// Materialize a tape into a `JSValue` tree identical to what the
/// direct parser would produce. Walks the tape from index 0 (the
/// root value) and recursively builds the tree.
///
/// Uses the same runtime allocators as `DirectParser` so the result
/// is GC-tracked + shape-cached identically. The materializer does
/// NOT use the typed-parse shape hint (that's Step 1b's path) —
/// it's the lazy-parse dual: correctness-preserving and order-
/// agnostic.
///
/// Returns `JSValue::null()` on empty tape (caller shouldn't invoke
/// materialize on None tapes, but this keeps the function total).
pub unsafe fn materialize(tape: &Tape, bytes: &[u8]) -> JSValue {
    let mut idx: usize = 0;
    materialize_value(tape, bytes, &mut idx)
}

#[inline]
unsafe fn materialize_value(tape: &Tape, bytes: &[u8], idx: &mut usize) -> JSValue {
    if *idx >= tape.entries.len() {
        return JSValue::null();
    }
    let entry = tape.entries[*idx];
    match entry.kind {
        KIND_OBJ_START => {
            let end_idx = entry.link as usize;
            *idx += 1;
            materialize_object(tape, bytes, idx, end_idx)
        }
        KIND_ARR_START => {
            let end_idx = entry.link as usize;
            *idx += 1;
            materialize_array(tape, bytes, idx, end_idx)
        }
        KIND_STRING => {
            *idx += 1;
            materialize_string_value(bytes, entry.offset as usize)
        }
        KIND_NUMBER => {
            *idx += 1;
            materialize_number(bytes, entry.offset as usize)
        }
        KIND_TRUE => { *idx += 1; JSValue::bool(true) }
        KIND_FALSE => { *idx += 1; JSValue::bool(false) }
        KIND_NULL => { *idx += 1; JSValue::null() }
        _ => JSValue::null(),
    }
}

unsafe fn materialize_object(
    tape: &Tape, bytes: &[u8], idx: &mut usize, end_idx: usize,
) -> JSValue {
    let obj = crate::object::js_object_alloc(0, 0);
    while *idx < end_idx {
        let key_entry = tape.entries[*idx];
        debug_assert_eq!(key_entry.kind, KIND_KEY);
        *idx += 1;
        // Extract and (possibly decode) the key bytes.
        let key_ptr = decode_key_to_interned_string(bytes, key_entry.offset as usize);
        let value = materialize_value(tape, bytes, idx);
        if !key_ptr.is_null() {
            crate::object::js_object_set_field_by_name(
                obj, key_ptr, f64::from_bits(value.bits()),
            );
        }
    }
    // Skip past the OBJ_END marker.
    *idx = end_idx + 1;
    JSValue::object_ptr(obj as *mut u8)
}

unsafe fn materialize_array(
    tape: &Tape, bytes: &[u8], idx: &mut usize, end_idx: usize,
) -> JSValue {
    let mut arr = crate::array::js_array_alloc(16);
    while *idx < end_idx {
        let value = materialize_value(tape, bytes, idx);
        arr = crate::array::js_array_push(arr, value);
    }
    *idx = end_idx + 1;
    JSValue::object_ptr(arr as *mut u8)
}

/// Decode the string literal starting at `offset` (the opening `"`)
/// into an interned `*mut StringHeader` — same contract as
/// `js_string_intern_from_bytes` provides elsewhere. Returns null on
/// malformed input (shouldn't happen for a tape built from valid
/// JSON, but defensive).
unsafe fn decode_key_to_interned_string(
    bytes: &[u8], offset: usize,
) -> *mut crate::StringHeader {
    let bytes_at_key = &bytes[offset..];
    match parse_string_bytes_static(bytes_at_key) {
        Some(ParsedStr::Borrowed(slice)) => {
            crate::string::js_string_from_bytes_longlived(slice.as_ptr(), slice.len() as u32)
        }
        Some(ParsedStr::Owned(vec)) => {
            crate::string::js_string_from_bytes_longlived(vec.as_ptr(), vec.len() as u32)
        }
        None => std::ptr::null_mut(),
    }
}

unsafe fn materialize_string_value(bytes: &[u8], offset: usize) -> JSValue {
    let bytes_at_val = &bytes[offset..];
    match parse_string_bytes_static(bytes_at_val) {
        Some(ParsedStr::Borrowed(slice)) => {
            let ptr = crate::string::js_string_from_bytes(slice.as_ptr(), slice.len() as u32);
            JSValue::string_ptr(ptr)
        }
        Some(ParsedStr::Owned(vec)) => {
            let ptr = crate::string::js_string_from_bytes(vec.as_ptr(), vec.len() as u32);
            JSValue::string_ptr(ptr)
        }
        None => JSValue::null(),
    }
}

unsafe fn materialize_number(bytes: &[u8], offset: usize) -> JSValue {
    // Find the number's end using the same rules as skip_number in
    // the tape builder. Slice then parse.
    let mut end = offset;
    if end < bytes.len() && bytes[end] == b'-' { end += 1; }
    while end < bytes.len() && bytes[end].is_ascii_digit() { end += 1; }
    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() { end += 1; }
    }
    if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        end += 1;
        if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') { end += 1; }
        while end < bytes.len() && bytes[end].is_ascii_digit() { end += 1; }
    }
    let num_str = std::str::from_utf8_unchecked(&bytes[offset..end]);
    let value: f64 = num_str.parse().unwrap_or(0.0);
    JSValue::number(value)
}

/// Parsed string slot: zero-copy borrow when no escapes, owned when
/// escapes required decoding. Mirrors `DirectParser::ParsedStr`.
enum ParsedStr<'a> {
    Borrowed(&'a [u8]),
    Owned(Vec<u8>),
}

/// Parse a `"…"` literal starting at `bytes[0]` (the opening quote).
/// Standalone because the materializer doesn't have a live
/// `DirectParser` instance. Same semantics as
/// `DirectParser::parse_string_bytes`.
fn parse_string_bytes_static(bytes: &[u8]) -> Option<ParsedStr<'_>> {
    if bytes.is_empty() || bytes[0] != b'"' { return None; }
    let mut pos = 1usize;
    let start = pos;
    while pos < bytes.len() {
        let c = bytes[pos];
        if c == b'"' {
            return Some(ParsedStr::Borrowed(&bytes[start..pos]));
        }
        if c == b'\\' {
            // Fall through to slow path from here.
            return parse_string_bytes_slow(bytes, pos, start);
        }
        pos += 1;
    }
    None
}

fn parse_string_bytes_slow(bytes: &[u8], start_pos: usize, start: usize) -> Option<ParsedStr<'_>> {
    let mut result: Vec<u8> = Vec::from(&bytes[start..start_pos]);
    let mut pos = start_pos;
    loop {
        if pos >= bytes.len() { return None; }
        let c = bytes[pos];
        pos += 1;
        match c {
            b'"' => return Some(ParsedStr::Owned(result)),
            b'\\' => {
                if pos >= bytes.len() { return None; }
                let esc = bytes[pos];
                pos += 1;
                match esc {
                    b'"' => result.push(b'"'),
                    b'\\' => result.push(b'\\'),
                    b'/' => result.push(b'/'),
                    b'n' => result.push(b'\n'),
                    b'r' => result.push(b'\r'),
                    b't' => result.push(b'\t'),
                    b'b' => result.push(0x08),
                    b'f' => result.push(0x0C),
                    b'u' => {
                        if pos + 4 > bytes.len() { return None; }
                        let hex = std::str::from_utf8(&bytes[pos..pos + 4]).ok()?;
                        let code = u16::from_str_radix(hex, 16).ok()?;
                        pos += 4;
                        if (0xD800..=0xDBFF).contains(&code) {
                            if pos + 6 <= bytes.len()
                                && bytes[pos] == b'\\'
                                && bytes[pos + 1] == b'u'
                            {
                                let hex2 = std::str::from_utf8(&bytes[pos + 2..pos + 6]).ok()?;
                                let low = u16::from_str_radix(hex2, 16).ok()?;
                                pos += 6;
                                let codepoint = 0x10000
                                    + ((code as u32 - 0xD800) << 10)
                                    + (low as u32 - 0xDC00);
                                if let Some(ch) = char::from_u32(codepoint) {
                                    let mut buf = [0u8; 4];
                                    let s = ch.encode_utf8(&mut buf);
                                    result.extend_from_slice(s.as_bytes());
                                }
                            }
                        } else if let Some(ch) = char::from_u32(code as u32) {
                            let mut buf = [0u8; 4];
                            let s = ch.encode_utf8(&mut buf);
                            result.extend_from_slice(s.as_bytes());
                        }
                    }
                    _ => result.push(esc),
                }
            }
            _ => result.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tape structure invariants on a simple object — exercises the
    /// OBJ_START → KEY → scalar → OBJ_END chain and the backfilled
    /// `link` for skip-over.
    #[test]
    fn tape_simple_object() {
        let input = br#"{"a":1,"b":"x"}"#;
        let tape = build_tape(input).unwrap();
        let kinds: Vec<u8> = tape.entries.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![KIND_OBJ_START, KIND_KEY, KIND_NUMBER, KIND_KEY, KIND_STRING, KIND_OBJ_END]
        );
        // OBJ_START.link points at the matching OBJ_END (last entry).
        assert_eq!(tape.entries[0].link as usize, tape.entries.len() - 1);
        // OBJ_END.link points back at OBJ_START.
        assert_eq!(*tape.entries.last().unwrap(), TapeEntry {
            offset: tape.entries.last().unwrap().offset,
            kind: KIND_OBJ_END,
            link: 0
        });
    }

    /// Nested structure — an array of objects. Each inner OBJ_START
    /// must have its link pointing at its OWN OBJ_END, not the outer
    /// ARR_END. This is the invariant Phase 3 (lazy indexed access)
    /// relies on to skip past unwanted elements.
    #[test]
    fn tape_nested_array_of_objects() {
        let input = br#"[{"a":1},{"b":2},{"c":3}]"#;
        let tape = build_tape(input).unwrap();
        // ARR_START ... ARR_END outer
        assert_eq!(tape.entries[0].kind, KIND_ARR_START);
        assert_eq!(tape.entries.last().unwrap().kind, KIND_ARR_END);
        // Three object children — count OBJ_START entries.
        let n_objs = tape.entries.iter().filter(|e| e.kind == KIND_OBJ_START).count();
        assert_eq!(n_objs, 3);
        // Each OBJ_START's link points at an OBJ_END strictly before ARR_END.
        for (i, e) in tape.entries.iter().enumerate() {
            if e.kind == KIND_OBJ_START {
                let end = e.link as usize;
                assert!(end > i, "OBJ_START.link must point forward");
                assert!(end < tape.entries.len() - 1, "OBJ_END must precede outer ARR_END");
                assert_eq!(tape.entries[end].kind, KIND_OBJ_END);
                assert_eq!(tape.entries[end].link as usize, i, "OBJ_END.link must point back");
            }
        }
    }

    /// Escaped string in a key and value — tape should still emit
    /// one KEY and one STRING entry; string decoding is deferred to
    /// materialization and doesn't perturb the tape shape.
    #[test]
    fn tape_escaped_strings() {
        let input = br#"{"a\"b":"x\\y"}"#;
        let tape = build_tape(input).unwrap();
        assert_eq!(
            tape.entries.iter().map(|e| e.kind).collect::<Vec<_>>(),
            vec![KIND_OBJ_START, KIND_KEY, KIND_STRING, KIND_OBJ_END]
        );
    }

    /// Malformed inputs must return None (caller falls back to
    /// direct parser with richer error messages).
    #[test]
    fn tape_malformed_returns_none() {
        assert!(build_tape(b"{").is_none(), "unclosed object");
        assert!(build_tape(b"[").is_none(), "unclosed array");
        assert!(build_tape(b"{a:1}").is_none(), "unquoted key");
        assert!(build_tape(b"{\"a\"}").is_none(), "missing colon");
        assert!(build_tape(b"").is_none(), "empty input");
    }

    /// Top-level scalar (allowed by JSON spec).
    #[test]
    fn tape_top_level_scalars() {
        assert_eq!(build_tape(b"42").unwrap().entries.len(), 1);
        assert_eq!(build_tape(b"true").unwrap().entries.len(), 1);
        assert_eq!(build_tape(br#""hi""#).unwrap().entries.len(), 1);
        assert_eq!(build_tape(b"null").unwrap().entries.len(), 1);
    }

    /// `TapeEntry` is 12 bytes (u32 + u8 + padding + u32). Keeping
    /// this compact matters for tape-size parity with parse output:
    /// a 1 MB JSON blob with ~20k tokens should build a ~240 KB tape,
    /// not a megabyte.
    #[test]
    fn tape_entry_layout() {
        assert!(std::mem::size_of::<TapeEntry>() <= 12,
            "TapeEntry grew beyond 12 bytes — check padding");
    }
}

impl PartialEq for TapeEntry {
    fn eq(&self, other: &Self) -> bool {
        self.offset == other.offset && self.kind == other.kind && self.link == other.link
    }
}
