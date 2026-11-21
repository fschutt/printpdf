//! CMaps: tables that map the bytes of a PDF string to glyphs and text.
//!
//! A CMap ("character map") is a small PostScript-like text stream. Two kinds matter for fonts:
//!
//! - A **ToUnicode CMap** is the `/ToUnicode` entry of a font. It maps each character code the
//!   content stream uses (for embedded fonts, the glyph id) to the Unicode text it stands for.
//!   Viewers use it for copy, paste and search; without it the text of an embedded font is
//!   unreadable. See [`ToUnicodeCMap`], which also writes one for the fonts printpdf embeds.
//! - A **CID CMap** is the `/Encoding` of a composite (Type0) font. It splits the bytes of a
//!   string into codes and maps each code to a CID, the index of a glyph in the font. printpdf
//!   writes `Identity-H` (two-byte codes, code equal to CID); [`CidCMap`] reads that and CMaps
//!   embedded in a PDF.
//!
//! # Examples
//!
//! ```
//! use printpdf::ToUnicodeCMap;
//!
//! let cmap = ToUnicodeCMap::parse("1 beginbfchar <0003> <0020> endbfchar").unwrap();
//! assert_eq!(cmap.lookup_string(3).as_deref(), Some(" "));
//! ```
use std::collections::BTreeMap;

use lopdf::{Dictionary, Document, Object};

use crate::text::CMap;

/// The most codes a single `bfrange` may cover. Real CMaps map codes of 1 to 4 bytes, so no
/// legitimate range spans more than 64k entries. The stream comes from the file, and a line
/// such as `<00000000> <FFFFFFFF> <0041>` would otherwise expand to 2^32 entries.
const MAX_BFRANGE_ENTRIES: u64 = 65_536;

/// A ToUnicode CMap: character codes (CIDs) mapped to the Unicode text they stand for.
///
/// A mapping targets one or more Unicode scalar values, several for a ligature such as "fi".
#[derive(Debug)]
pub struct ToUnicodeCMap {
    /// Code to Unicode scalar values.
    pub mappings: BTreeMap<u32, Vec<u32>>,
}

/// One whitespace-separated CMap token.
#[derive(Debug, Clone, PartialEq)]
enum CMapToken {
    /// `<...>` hex string (raw hex digits, without the delimiters)
    Hex(String),
    /// `[`
    ArrayOpen,
    /// `]`
    ArrayClose,
    /// any other token (keywords, numbers, names, ...)
    Word(String),
}

/// Tokenize a CMap stream. PostScript is whitespace/delimiter separated, so
/// `1 beginbfchar<0003><0020>endbfchar` on a single line is legal — a line-based
/// parser silently loses those mappings. Comments (`%` to end of line) are skipped.
fn tokenize_cmap(input: &str) -> Vec<CMapToken> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some(&(i, c)) = chars.peek() {
        match c {
            '%' => {
                // comment until end of line
                while let Some(&(_, c2)) = chars.peek() {
                    if c2 == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            '<' => {
                chars.next();
                let start = i + 1;
                let mut end = start;
                while let Some(&(j, c2)) = chars.peek() {
                    if c2 == '>' {
                        chars.next();
                        break;
                    }
                    // `<<` starts a dictionary, not a hex string — treat like a word
                    end = j + c2.len_utf8();
                    chars.next();
                }
                tokens.push(CMapToken::Hex(
                    input[start..end].split_whitespace().collect::<String>(),
                ));
            }
            '[' => {
                chars.next();
                tokens.push(CMapToken::ArrayOpen);
            }
            ']' => {
                chars.next();
                tokens.push(CMapToken::ArrayClose);
            }
            c if c.is_whitespace() => {
                chars.next();
            }
            _ => {
                let start = i;
                let mut end = i;
                while let Some(&(j, c2)) = chars.peek() {
                    if c2.is_whitespace() || c2 == '<' || c2 == '[' || c2 == ']' || c2 == '%' {
                        break;
                    }
                    end = j + c2.len_utf8();
                    chars.next();
                }
                tokens.push(CMapToken::Word(input[start..end].to_string()));
            }
        }
    }

    tokens
}

/// Parse a hex string token used as a *code* (CID): a single big-endian number.
fn hex_to_u32(hex: &str) -> Result<u32, String> {
    if hex.is_empty() {
        return Err("empty hex token".to_string());
    }
    u32::from_str_radix(hex, 16).map_err(|e| format!("Failed to parse hex token <{}>: {}", hex, e))
}

/// Parse a hex string token used as a *target*: UTF-16BE code units, possibly
/// several characters (ligatures like `<004600660069>` = "ffi") and possibly
/// surrogate pairs (`<D835DC56>` = U+1D456). Returns Unicode scalar values.
fn hex_to_unicode_scalars(hex: &str) -> Result<Vec<u32>, String> {
    if hex.is_empty() {
        return Err("empty hex token".to_string());
    }
    if hex.len() <= 4 {
        // single UTF-16 code unit (or fewer digits, e.g. `<20>`)
        return Ok(vec![hex_to_u32(hex)?]);
    }
    if !hex.len().is_multiple_of(4) {
        // Not a whole number of UTF-16BE units: read the token as one number.
        return hex_to_u32(hex).map(|v| vec![v]);
    }
    let mut units = Vec::with_capacity(hex.len() / 4);
    for i in (0..hex.len()).step_by(4) {
        // `get`, not `[i..i + 4]`: the token comes from the file and may hold a multi-byte
        // character, and slicing a `str` inside one panics.
        let unit = hex
            .get(i..i + 4)
            .ok_or_else(|| format!("Failed to parse hex token <{hex}>: not hexadecimal"))?;
        units.push(
            u16::from_str_radix(unit, 16)
                .map_err(|e| format!("Failed to parse hex token <{}>: {}", hex, e))?,
        );
    }
    let scalars: Vec<u32> = char::decode_utf16(units.iter().copied())
        .map(|r| r.map(|c| c as u32).unwrap_or(0xFFFD))
        .collect();
    Ok(scalars)
}

impl ToUnicodeCMap {
    /// Parses a ToUnicode CMap from its text.
    ///
    /// Reads the `bfchar` and `bfrange` sections (both forms of `bfrange`: one start value
    /// that is incremented, or an array with one target per code); everything else is
    /// ignored. Malformed entries are skipped.
    ///
    /// # Errors
    ///
    /// Returns an error for a hex token that is not a number, a reversed `bfrange`, or one that
    /// spans more than 65536 codes.
    pub fn parse(input: &str) -> Result<ToUnicodeCMap, String> {
        let mut mappings = BTreeMap::new();
        let tokens = tokenize_cmap(input);
        let mut i = 0;

        while i < tokens.len() {
            match &tokens[i] {
                CMapToken::Word(w) if w == "beginbfchar" => {
                    i += 1;
                    // Each mapping is: <code> <target>
                    while i < tokens.len() {
                        match (&tokens[i], tokens.get(i + 1)) {
                            (CMapToken::Word(w), _) if w == "endbfchar" => break,
                            (CMapToken::Hex(code), Some(CMapToken::Hex(target))) => {
                                let cid = hex_to_u32(code)?;
                                let uni = hex_to_unicode_scalars(target)?;
                                mappings.insert(cid, uni);
                                i += 2;
                            }
                            _ => {
                                // skip malformed token
                                i += 1;
                            }
                        }
                    }
                }
                CMapToken::Word(w) if w == "beginbfrange" => {
                    i += 1;
                    while i < tokens.len() {
                        // Two forms:
                        //   form1: <start> <end> <startUnicode>
                        //   form2: <start> <end> [ <unicode1> <unicode2> ... ]
                        match &tokens[i] {
                            CMapToken::Word(w) if w == "endbfrange" => break,
                            CMapToken::Hex(start_tok) => {
                                let Some(CMapToken::Hex(end_tok)) = tokens.get(i + 1) else {
                                    i += 1;
                                    continue;
                                };
                                let start = hex_to_u32(start_tok)?;
                                let end = hex_to_u32(end_tok)?;
                                // Reject reversed and oversized ranges: `end - start + 1`
                                // would underflow, and a huge span would expand to millions
                                // of entries. The caller turns the error into a warning, so a
                                // hostile CMap costs the font its ToUnicode map and nothing
                                // else.
                                if end < start {
                                    return Err(format!(
                                        "bfrange: end {:04X} < start {:04X}",
                                        end, start
                                    ));
                                }
                                let span = end - start;
                                if span as u64 + 1 > MAX_BFRANGE_ENTRIES {
                                    return Err(format!(
                                        "bfrange spans {} CIDs (max {})",
                                        span as u64 + 1,
                                        MAX_BFRANGE_ENTRIES
                                    ));
                                }
                                match tokens.get(i + 2) {
                                    Some(CMapToken::ArrayOpen) => {
                                        // form2: one target per CID.
                                        i += 3;
                                        let mut offset = 0u32;
                                        while i < tokens.len() {
                                            match &tokens[i] {
                                                CMapToken::ArrayClose => {
                                                    i += 1;
                                                    break;
                                                }
                                                CMapToken::Hex(target) => {
                                                    // Ignore surplus entries beyond the
                                                    // declared range instead of erroring:
                                                    // partial data beats no data.
                                                    if offset <= span {
                                                        let uni = hex_to_unicode_scalars(target)?;
                                                        mappings.insert(start + offset, uni);
                                                    }
                                                    offset += 1;
                                                    i += 1;
                                                }
                                                _ => {
                                                    i += 1;
                                                }
                                            }
                                        }
                                    }
                                    Some(CMapToken::Hex(target)) => {
                                        // form1: single starting value, incremented over
                                        // the range. Increment applies to the *last*
                                        // character of the target string (ISO 32000-1,
                                        // 9.10.3).
                                        let unis = hex_to_unicode_scalars(target)?;
                                        let last = unis.last().copied().unwrap_or(0);
                                        if last.checked_add(span).is_none() {
                                            return Err(
                                                "bfrange: target unicode values overflow u32"
                                                    .to_string(),
                                            );
                                        }
                                        for off in 0..=span {
                                            let mut target = unis.clone();
                                            if let Some(l) = target.last_mut() {
                                                *l = last + off;
                                            }
                                            mappings.insert(start + off, target);
                                        }
                                        i += 3;
                                    }
                                    _ => {
                                        // malformed entry, skip the start token
                                        i += 1;
                                    }
                                }
                                continue;
                            }
                            _ => {
                                i += 1;
                            }
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }

        Ok(ToUnicodeCMap { mappings })
    }

    /// The Unicode scalar values a code maps to.
    pub fn lookup(&self, cid: u32) -> Option<&Vec<u32>> {
        self.mappings.get(&cid)
    }

    /// The text a code maps to, or `None` when the code is unmapped or maps to nothing valid.
    pub fn lookup_string(&self, cid: u32) -> Option<String> {
        let unis = self.mappings.get(&cid)?;
        let s: String = unis
            .iter()
            .filter_map(|&u| std::char::from_u32(u))
            .collect();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    /// Writes the CMap as the text of a `/ToUnicode` stream.
    ///
    /// `font_name` becomes the CMap's `/Ordering` and name. The codespace is `<0000> <FFFF>`
    /// (two-byte codes), and mappings are written as `bfchar` blocks of at most 100 entries,
    /// with targets in UTF-16BE. Codes with an empty target are left out.
    pub fn to_cmap_string(&self, font_name: &str) -> String {
        // Header section
        let mut result = format!(
            "/CIDInit /ProcSet findresource begin\n\n12 dict begin\n\nbegincmap\n\n%!PS-Adobe-3.0 \
             Resource-CMap\n%%DocumentNeededResources: procset CIDInit\n%%IncludeResource: \
             procset CIDInit\n\n/CIDSystemInfo 3 dict dup begin\n/Registry (FontSpecific) \
             def\n/Ordering ({}) def\n/Supplement 0 def\nend def\n\n/CMapName /FontSpecific-{} \
             def\n/CMapVersion 1 def\n/CMapType 2 def\n/WMode 0 def\n\n1 \
             begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
            font_name, font_name
        );

        // Group mappings by high byte for better organization
        let mut grouped_by_high_byte: BTreeMap<u8, Vec<(u32, &Vec<u32>)>> = BTreeMap::new();

        for (&cid, unicode_values) in &self.mappings {
            if unicode_values.is_empty() {
                continue;
            }
            let high_byte = ((cid >> 8) & 0xFF) as u8;
            grouped_by_high_byte
                .entry(high_byte)
                .or_default()
                .push((cid, unicode_values));
        }

        // Generate bfchar blocks with at most 100 entries each
        for (_high_byte, mut entries) in grouped_by_high_byte {
            // Sort by CID for deterministic output
            entries.sort_by_key(|&(cid, _)| cid);

            // Process in chunks of 100
            for chunk in entries.chunks(100) {
                result.push_str(&format!("{} beginbfchar\n", chunk.len()));
                for &(cid, unicode_values) in chunk {
                    // Encode the full target as UTF-16BE so multi-char mappings
                    // (ligatures) and non-BMP chars survive the round trip.
                    let mut target_hex = String::new();
                    for &u in unicode_values {
                        match std::char::from_u32(u) {
                            Some(c) => {
                                let mut buf = [0u16; 2];
                                for unit in c.encode_utf16(&mut buf) {
                                    target_hex.push_str(&format!("{:04X}", unit));
                                }
                            }
                            None => target_hex.push_str(&format!("{:04X}", u)),
                        }
                    }
                    result.push_str(&format!("<{:04X}> <{}>\n", cid, target_hex));
                }
                result.push_str("endbfchar\n");
            }
        }

        // Footer section
        result.push_str(
            "\
            endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n",
        );

        result
    }
}

/// A CMap from codes to CIDs: the `/Encoding` of a composite (Type0) font (ISO 32000-1,
/// 9.7.5).
///
/// Where [`ToUnicodeCMap`] maps codes to text, this maps the bytes of a content-stream string
/// to codes (through the codespace ranges; codes are 1 to 4 bytes and may differ in length)
/// and codes to CIDs (through the `cidrange` and `cidchar` entries).
///
/// Built from an embedded CMap stream, or from the two predefined CMaps that need no external
/// data, `Identity-H` and `Identity-V`. The other predefined CMaps (`90ms-RKSJ-H`,
/// `GBK-EUC-H`, `UniJIS-UCS2-H`, ...) need Adobe's CMap files, which printpdf does not
/// include; [`CidCMap::parse`] reports them as unresolved.
#[derive(Debug, Clone, PartialEq)]
pub struct CidCMap {
    /// Writing mode from `/WMode`: 0 is horizontal, 1 is vertical.
    pub wmode: u8,
    /// Codespace ranges as `(byte_len, lo_bytes, hi_bytes)`: a code of `byte_len` bytes lies in
    /// the range when each of its bytes lies between the corresponding lo and hi bytes
    /// (compared byte by byte, not as numbers).
    codespace: Vec<(u8, [u8; 4], [u8; 4])>,
    /// `(byte_len, lo, hi, dst_cid)` from cidrange: code → dst + (code - lo).
    ranges: Vec<(u8, u32, u32, u32)>,
    /// `(byte_len, code, cid)` from cidchar.
    singles: BTreeMap<(u8, u32), u32>,
    /// True for the identity CMaps: code == CID, no table lookups needed.
    identity: bool,
}

/// The most codes a single `cidrange` may cover; the stream comes from the file, see
/// [`MAX_BFRANGE_ENTRIES`].
const MAX_CIDRANGE_SPAN: u64 = 65_536;

impl CidCMap {
    /// The predefined `Identity-H` (`wmode` 0) or `Identity-V` (`wmode` 1) CMap: two-byte
    /// codes, code equal to CID.
    pub fn identity(wmode: u8) -> Self {
        CidCMap {
            wmode,
            codespace: vec![(2, [0, 0, 0, 0], [0xFF, 0xFF, 0, 0])],
            ranges: Vec::new(),
            singles: BTreeMap::new(),
            identity: true,
        }
    }

    /// Whether this is an identity CMap, where code equals CID and no table is consulted.
    pub fn is_identity(&self) -> bool {
        self.identity
    }

    /// Parses an embedded CMap stream.
    ///
    /// Returns the CMap and, when the stream says `usecmap` with a base other than
    /// `Identity-H` or `Identity-V`, that base's name: it cannot be resolved without Adobe's
    /// data files, and the caller should warn about it.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed hex token, a code that is not 1 to 4 bytes, or a
    /// `cidrange` spanning more than 65536 codes.
    pub fn parse(input: &str) -> Result<(CidCMap, Option<String>), String> {
        let tokens = tokenize_cmap(input);
        let mut cmap = CidCMap {
            wmode: 0,
            codespace: Vec::new(),
            ranges: Vec::new(),
            singles: BTreeMap::new(),
            identity: false,
        };
        let mut unresolved_usecmap = None;

        let hex_bytes = |hex: &str| -> Result<(u8, [u8; 4], u32), String> {
            let digits = hex.len();
            if digits == 0 || digits > 8 || !digits.is_multiple_of(2) {
                return Err(format!("codespace hex <{hex}> is not 1–4 bytes"));
            }
            let len = (digits / 2) as u8;
            let mut bytes = [0u8; 4];
            for (idx, i) in (0..digits).step_by(2).enumerate() {
                // `get`, not `[i..i + 2]`: a multi-byte character in the token would panic.
                let byte = hex
                    .get(i..i + 2)
                    .ok_or_else(|| format!("bad hex <{hex}>: not hexadecimal"))?;
                bytes[idx] =
                    u8::from_str_radix(byte, 16).map_err(|e| format!("bad hex <{hex}>: {e}"))?;
            }
            Ok((len, bytes, hex_to_u32(hex)?))
        };

        let mut i = 0;
        while i < tokens.len() {
            match &tokens[i] {
                CMapToken::Word(w) if w == "begincodespacerange" => {
                    i += 1;
                    while i < tokens.len() {
                        match (&tokens[i], tokens.get(i + 1)) {
                            (CMapToken::Word(w), _) if w == "endcodespacerange" => break,
                            (CMapToken::Hex(lo), Some(CMapToken::Hex(hi))) => {
                                let (llen, lb, _) = hex_bytes(lo)?;
                                let (hlen, hb, _) = hex_bytes(hi)?;
                                if llen == hlen {
                                    cmap.codespace.push((llen, lb, hb));
                                }
                                i += 2;
                            }
                            _ => i += 1,
                        }
                    }
                }
                CMapToken::Word(w) if w == "begincidrange" || w == "beginnotdefrange" => {
                    let end_kw = if w == "begincidrange" {
                        "endcidrange"
                    } else {
                        "endnotdefrange"
                    };
                    i += 1;
                    while i < tokens.len() {
                        match (&tokens[i], tokens.get(i + 1), tokens.get(i + 2)) {
                            (CMapToken::Word(w), _, _) if w == end_kw => break,
                            (
                                CMapToken::Hex(lo),
                                Some(CMapToken::Hex(hi)),
                                Some(CMapToken::Word(dst)),
                            ) => {
                                let (llen, _, lov) = hex_bytes(lo)?;
                                let (hlen, _, hiv) = hex_bytes(hi)?;
                                let dst: u32 = dst
                                    .parse()
                                    .map_err(|e| format!("cidrange dst {dst:?}: {e}"))?;
                                if llen == hlen && lov <= hiv {
                                    if (hiv - lov) as u64 + 1 > MAX_CIDRANGE_SPAN {
                                        return Err(format!(
                                            "cidrange spans {} codes (max {})",
                                            (hiv - lov) as u64 + 1,
                                            MAX_CIDRANGE_SPAN
                                        ));
                                    }
                                    cmap.ranges.push((llen, lov, hiv, dst));
                                }
                                i += 3;
                            }
                            _ => i += 1,
                        }
                    }
                }
                CMapToken::Word(w) if w == "begincidchar" => {
                    i += 1;
                    while i < tokens.len() {
                        match (&tokens[i], tokens.get(i + 1)) {
                            (CMapToken::Word(w), _) if w == "endcidchar" => break,
                            (CMapToken::Hex(code), Some(CMapToken::Word(cid))) => {
                                let (len, _, code_v) = hex_bytes(code)?;
                                if let Ok(cid) = cid.parse::<u32>() {
                                    cmap.singles.insert((len, code_v), cid);
                                }
                                i += 2;
                            }
                            _ => i += 1,
                        }
                    }
                }
                CMapToken::Word(w) if w == "usecmap" => {
                    // The base CMap name is the last /Name token before `usecmap`.
                    let base = tokens[..i].iter().rev().find_map(|t| match t {
                        CMapToken::Word(w) if w.starts_with('/') => Some(w[1..].to_string()),
                        _ => None,
                    });
                    match base.as_deref() {
                        Some("Identity-H") => cmap.identity = cmap.tables_are_empty(),
                        Some("Identity-V") => {
                            cmap.identity = cmap.tables_are_empty();
                            cmap.wmode = 1;
                        }
                        Some(other) => unresolved_usecmap = Some(other.to_string()),
                        None => {}
                    }
                }
                CMapToken::Word(w) if w == "/WMode" => {
                    if let Some(CMapToken::Word(v)) = tokens.get(i + 1) {
                        if v == "1" {
                            cmap.wmode = 1;
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }

        // A CMap with no codespace at all cannot split codes; assume the
        // ubiquitous 2-byte space rather than failing the whole font.
        if cmap.codespace.is_empty() {
            cmap.codespace.push((2, [0, 0, 0, 0], [0xFF, 0xFF, 0, 0]));
        }
        Ok((cmap, unresolved_usecmap))
    }

    fn tables_are_empty(&self) -> bool {
        self.ranges.is_empty() && self.singles.is_empty()
    }

    /// Splits a content-stream string into `(code, byte_len)` pairs by the codespace ranges
    /// (ISO 32000-1, 9.7.6.2). A code matches a range of length L when each of its L bytes lies
    /// within the range's bytes. Where nothing matches, the shortest range whose first byte
    /// matches is consumed (the code maps to notdef later), or else one byte.
    pub fn split_codes(&self, bytes: &[u8]) -> Vec<(u32, u8)> {
        let mut out = Vec::with_capacity(bytes.len() / 2);
        let mut pos = 0;
        while pos < bytes.len() {
            let rest = &bytes[pos..];
            let mut matched: Option<u8> = None;
            // Prefer the shortest matching range (ranges are tried by length).
            let mut lens: Vec<u8> = self.codespace.iter().map(|(l, ..)| *l).collect();
            lens.sort_unstable();
            lens.dedup();
            'outer: for len in lens {
                let len_usize = len as usize;
                if rest.len() < len_usize {
                    continue;
                }
                for (l, lo, hi) in &self.codespace {
                    if *l != len {
                        continue;
                    }
                    if (0..len_usize)
                        .all(|k| rest[k] >= lo[k] && rest[k] <= hi[k])
                    {
                        matched = Some(len);
                        break 'outer;
                    }
                }
            }
            let len = matched.unwrap_or_else(|| {
                // No full match: shortest range whose FIRST byte matches, else 1.
                self.codespace
                    .iter()
                    .filter(|(l, lo, hi)| {
                        *l >= 1 && rest[0] >= lo[0] && rest[0] <= hi[0]
                    })
                    .map(|(l, ..)| *l)
                    .min()
                    .unwrap_or(1)
            });
            let len_usize = (len as usize).min(rest.len());
            let mut code = 0u32;
            for &b in &rest[..len_usize] {
                code = (code << 8) | b as u32;
            }
            out.push((code, len_usize as u8));
            pos += len_usize;
        }
        out
    }

    /// The CID for a code of the given byte length. Unmapped codes give CID 0 (notdef),
    /// except in the identity CMaps, where the code is the CID.
    pub fn cid_for_code(&self, code: u32, byte_len: u8) -> u32 {
        if self.identity {
            return code;
        }
        if let Some(cid) = self.singles.get(&(byte_len, code)) {
            return *cid;
        }
        for (len, lo, hi, dst) in &self.ranges {
            if *len == byte_len && code >= *lo && code <= *hi {
                // A range whose destination runs past u32::MAX is corrupt: the code is
                // unmapped (notdef), not wrapped around to some other glyph.
                return dst.checked_add(code - lo).unwrap_or(0);
            }
        }
        0
    }
}

/// Decodes strings of two-byte codes, as `Identity-H` fonts use; a trailing odd byte and
/// unmapped codes are skipped.
impl CMap for ToUnicodeCMap {
    fn map_bytes(&self, bytes: &[u8]) -> String {
        let mut result = String::new();
        let mut i = 0;
        while i + 1 < bytes.len() {
            let cid = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as u32;
            if let Some(unis) = self.mappings.get(&cid) {
                for &u in unis {
                    if let Some(ch) = std::char::from_u32(u) {
                        result.push(ch);
                    }
                }
            }
            i += 2;
        }
        result
    }
}

/// Reads and parses the `/ToUnicode` CMap of a font dictionary.
///
/// # Errors
///
/// Returns an error when the entry is missing or is neither a stream nor a reference to one,
/// when the stream cannot be decompressed or is not UTF-8, or when the CMap does not parse.
pub fn get_to_unicode_cmap_from_font(
    font_dict: &Dictionary,
    doc: &Document,
) -> Result<ToUnicodeCMap, String> {
    let to_unicode_obj = font_dict
        .get(b"ToUnicode")
        .ok()
        .ok_or("No ToUnicode entry found")?;

    let stream = match to_unicode_obj {
        Object::Reference(r) => doc
            .get_object(*r)
            .and_then(|obj| obj.as_stream().cloned())
            .map_err(|e| format!("Error getting ToUnicode stream: {}", e))?,
        Object::Stream(s) => s.clone(),
        _ => return Err("Unexpected type for ToUnicode entry".into()),
    };

    let content = stream
        .decompressed_content()
        .map_err(|e| format!("Decompress error: {}", e))?;

    let cmap_str =
        String::from_utf8(content).map_err(|e| format!("UTF-8 conversion error: {}", e))?;

    ToUnicodeCMap::parse(&cmap_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_bfchar_is_parsed() {
        // PostScript does not require newlines between tokens.
        let cmap = "begincmap 1 beginbfchar <0003> <0020> endbfchar endcmap";
        let parsed = ToUnicodeCMap::parse(cmap).unwrap();
        assert_eq!(parsed.mappings.get(&3), Some(&vec![0x20]));
    }

    #[test]
    fn multi_codepoint_bfchar_target() {
        // <00660066> is the two-character target "ff", not one 32-bit number.
        let cmap = "beginbfchar\n<0001> <00660066>\n<0002> <004600660069>\nendbfchar";
        let parsed = ToUnicodeCMap::parse(cmap).unwrap();
        assert_eq!(parsed.mappings.get(&1), Some(&vec![0x66, 0x66]));
        assert_eq!(parsed.mappings.get(&2), Some(&vec![0x46, 0x66, 0x69]));
    }

    #[test]
    fn surrogate_pair_bfchar_target() {
        // <D835DC56> is U+1D456 (mathematical italic small i) as UTF-16BE.
        let cmap = "beginbfchar\n<0001> <D835DC56>\nendbfchar";
        let parsed = ToUnicodeCMap::parse(cmap).unwrap();
        assert_eq!(parsed.mappings.get(&1), Some(&vec![0x1D456]));
    }

    #[test]
    fn bfrange_form1_still_works() {
        let cmap = "beginbfrange\n<0041> <0043> <0061>\nendbfrange";
        let parsed = ToUnicodeCMap::parse(cmap).unwrap();
        assert_eq!(parsed.mappings.get(&0x41), Some(&vec![0x61]));
        assert_eq!(parsed.mappings.get(&0x42), Some(&vec![0x62]));
        assert_eq!(parsed.mappings.get(&0x43), Some(&vec![0x63]));
    }

    #[test]
    fn bfrange_form2_array_still_works() {
        let cmap = "beginbfrange\n<0001> <0002> [<0041> <0042>]\nendbfrange";
        let parsed = ToUnicodeCMap::parse(cmap).unwrap();
        assert_eq!(parsed.mappings.get(&1), Some(&vec![0x41]));
        assert_eq!(parsed.mappings.get(&2), Some(&vec![0x42]));
    }

    #[test]
    fn bfrange_reversed_is_error() {
        // A reversed range is an error (it would underflow).
        let cmap = "beginbfrange\n<0002> <0001> [<0041>]\nendbfrange";
        assert!(ToUnicodeCMap::parse(cmap).is_err());
    }

    #[test]
    fn bfrange_huge_range_is_bounded() {
        // A hostile few-byte bfrange must not expand to millions of entries.
        let cmap = "beginbfrange\n<000000> <1FFFFF> <0041>\nendbfrange";
        if let Ok(m) = ToUnicodeCMap::parse(cmap) { assert!(m.mappings.len() <= 65_536) }
    }

    #[test]
    fn roundtrip_through_cmap_string() {
        let mut mappings = BTreeMap::new();
        mappings.insert(1, vec![0x66, 0x66]); // "ff" ligature
        mappings.insert(2, vec![0x1D456]); // non-BMP char
        mappings.insert(3, vec![0x41]);
        let cmap = ToUnicodeCMap { mappings };
        let s = cmap.to_cmap_string("TESTFONT");
        let reparsed = ToUnicodeCMap::parse(&s).unwrap();
        assert_eq!(reparsed.mappings.get(&1), Some(&vec![0x66, 0x66]));
        assert_eq!(reparsed.mappings.get(&2), Some(&vec![0x1D456]));
        assert_eq!(reparsed.mappings.get(&3), Some(&vec![0x41]));
    }
}
