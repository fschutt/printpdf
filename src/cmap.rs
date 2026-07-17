/// ToUnicode CMap parsing
use std::collections::BTreeMap;

use lopdf::{Dictionary, Document, Object};

use crate::text::CMap;

/// A single bfrange line may cover at most this many CIDs. Real CMaps map 1–4 byte
/// codes, so no legitimate range spans more than 64k entries — but the CMap stream of
/// a parsed PDF is attacker-controlled, and one hostile 3-token line like
/// `<00000000> <FFFFFFFF> <0041>` would otherwise expand to 2^32 map insertions.
const MAX_BFRANGE_ENTRIES: u64 = 65_536;

/// The mapping from a CID to one or more Unicode code points.
#[derive(Debug)]
pub struct ToUnicodeCMap {
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
///
/// Tokens of up to 4 hex digits (the overwhelmingly common case) decode exactly
/// like the old single-number path.
fn hex_to_unicode_scalars(hex: &str) -> Result<Vec<u32>, String> {
    if hex.is_empty() {
        return Err("empty hex token".to_string());
    }
    if hex.len() <= 4 {
        // single UTF-16 code unit (or fewer digits, e.g. `<20>`)
        return Ok(vec![hex_to_u32(hex)?]);
    }
    if hex.len() % 4 != 0 {
        // Not a whole number of UTF-16BE units; historic behavior parsed the
        // token as one number, keep that as a fallback for short odd tokens.
        return hex_to_u32(hex).map(|v| vec![v]);
    }
    let mut units = Vec::with_capacity(hex.len() / 4);
    for i in (0..hex.len()).step_by(4) {
        units.push(
            u16::from_str_radix(&hex[i..i + 4], 16)
                .map_err(|e| format!("Failed to parse hex token <{}>: {}", hex, e))?,
        );
    }
    let scalars: Vec<u32> = char::decode_utf16(units.iter().copied())
        .map(|r| r.map(|c| c as u32).unwrap_or(0xFFFD))
        .collect();
    Ok(scalars)
}

impl ToUnicodeCMap {
    /// Parses a ToUnicode CMap from the given input string.
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
                                // Reject reversed and oversized ranges instead of expanding
                                // them: `end - start + 1` underflows (panics with overflow
                                // checks on) when end < start, and an unbounded span is a
                                // decompression-bomb-style DoS. The caller
                                // (extract_to_unicode_cmap) downgrades the Err to a warning,
                                // so a hostile CMap costs the font its ToUnicode map but
                                // never the whole document.
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

    /// Look up a single code/CID, returning its Unicode scalar values.
    pub fn lookup(&self, cid: u32) -> Option<&Vec<u32>> {
        self.mappings.get(&cid)
    }

    /// Map a single code/CID to a String (empty when unmapped).
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

    /// Generates a CMap string representation suitable for embedding in a PDF.
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
                .or_insert_with(Vec::new)
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

/// Implement the CMap trait on our ToUnicodeCMap.
impl CMap for ToUnicodeCMap {
    fn map_bytes(&self, bytes: &[u8]) -> String {
        // For simplicity, assume that the byte sequence represents CIDs in big-endian,
        // and that each CID is 2 bytes long.
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

/// Looks for a `ToUnicode` CMap entry on the dictionary, resolves it to a dictionary
/// and parses the `ToUnicodeCMap`.
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
            .and_then(|obj| obj.as_stream().map(|s| s.clone()))
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
        // PostScript does not require newlines; a line-based parser lost these.
        let cmap = "begincmap 1 beginbfchar <0003> <0020> endbfchar endcmap";
        let parsed = ToUnicodeCMap::parse(cmap).unwrap();
        assert_eq!(parsed.mappings.get(&3), Some(&vec![0x20]));
    }

    #[test]
    fn multi_codepoint_bfchar_target() {
        // <00660066> is the two-character target "ff". The old parser tried to
        // read it as one u32 (0x00660066) and produced garbage, and 12-digit
        // targets made the whole CMap fail to parse.
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
        // Guard: keep rejecting reversed ranges (underflow / DoS protection).
        let cmap = "beginbfrange\n<0002> <0001> [<0041>]\nendbfrange";
        assert!(ToUnicodeCMap::parse(cmap).is_err());
    }

    #[test]
    fn bfrange_huge_range_is_bounded() {
        // Guard: a hostile few-byte bfrange must not allocate 2^21 entries.
        let cmap = "beginbfrange\n<000000> <1FFFFF> <0041>\nendbfrange";
        match ToUnicodeCMap::parse(cmap) {
            Ok(m) => assert!(m.mappings.len() <= 65_536),
            Err(_) => {}
        }
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
