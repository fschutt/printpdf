/// ToUnicode CMap parsing
use std::collections::BTreeMap;

use lopdf::{Dictionary, Document, Object};

use crate::text::CMap;

/// The mapping from a CID to one or more Unicode code points.
#[derive(Debug)]
pub struct ToUnicodeCMap {
    pub mappings: BTreeMap<u32, Vec<u32>>,
}

impl ToUnicodeCMap {
    /// Parses a ToUnicode CMap from the given input string.
    pub fn parse(input: &str) -> Result<ToUnicodeCMap, String> {
        let mut mappings = BTreeMap::new();
        let mut lines = input.lines().map(|l| l.trim()).filter(|l| !l.is_empty());
        while let Some(line) = lines.next() {
            if line.contains("beginbfchar") {
                // Process each bfchar mapping line until "endbfchar"
                while let Some(l) = lines.next() {
                    if l.contains("endbfchar") {
                        break;
                    }
                    // Expect a line like: "<0041> <0041>"
                    let tokens: Vec<&str> = l.split_whitespace().collect();
                    if tokens.len() < 2 {
                        continue; // skip bad lines
                    }
                    let cid = parse_hex_token(tokens[0])?;
                    let uni = parse_hex_token(tokens[1])?;
                    mappings.insert(cid, vec![uni]);
                }
            } else if line.contains("beginbfrange") {
                // Process each bfrange mapping line until "endbfrange"
                while let Some(l) = lines.next() {
                    if l.contains("endbfrange") {
                        break;
                    }
                    // There are two forms:
                    //   form1: <start> <end> <startUnicode>
                    //   form2: <start> <end> [ <unicode1> <unicode2> ... ]
                    let tokens: Vec<&str> = l.split_whitespace().collect();
                    if tokens.len() < 3 {
                        continue;
                    }
                    let start = parse_hex_token(tokens[0])?;
                    let end = parse_hex_token(tokens[1])?;
                    if tokens[2].starts_with('[') {
                        // form2: rebuild the array of tokens.
                        let mut arr_tokens = Vec::new();
                        // Remove the leading '[' from the first token.
                        let first = tokens[2].trim_start_matches('[');
                        arr_tokens.push(first);
                        // Process the rest tokens until one ends with ']'.
                        for token in tokens.iter().skip(3) {
                            if token.ends_with(']') {
                                arr_tokens.push(token.trim_end_matches(']'));
                                break;
                            } else {
                                arr_tokens.push(token);
                            }
                        }
                        let expected = end - start + 1;
                        if arr_tokens.len() != expected as usize {
                            return Err(format!(
                                "bfrange array length mismatch: expected {} but got {}",
                                expected,
                                arr_tokens.len()
                            ));
                        }
                        for (i, token) in arr_tokens.iter().enumerate() {
                            let uni = parse_hex_token(token)?;
                            mappings.insert(start + i as u32, vec![uni]);
                        }
                    } else {
                        // form1: a single starting unicode value.
                        let start_uni = parse_hex_token(tokens[2])?;
                        let mut cur = start_uni;
                        for cid in start..=end {
                            mappings.insert(cid, vec![cur]);
                            cur += 1;
                        }
                    }
                }
            }
            // (Other lines, e.g. codespacerange, can be skipped for now.)
        }
        Ok(ToUnicodeCMap { mappings })
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
        let mut grouped_by_high_byte: BTreeMap<u8, Vec<(u32, u32)>> = BTreeMap::new();

        for (&cid, unicode_values) in &self.mappings {
            if let Some(&unicode) = unicode_values.first() {
                let high_byte = ((cid >> 8) & 0xFF) as u8;
                grouped_by_high_byte
                    .entry(high_byte)
                    .or_insert_with(Vec::new)
                    .push((cid, unicode));
            }
        }

        // Generate bfchar blocks with at most 100 entries each
        for (_high_byte, mut entries) in grouped_by_high_byte {
            // Sort by CID for deterministic output
            entries.sort_by_key(|&(cid, _)| cid);

            // Process in chunks of 100
            for chunk in entries.chunks(100) {
                result.push_str(&format!("{} beginbfchar\n", chunk.len()));
                for &(cid, unicode) in chunk {
                    result.push_str(&format!("<{:04X}> <{:04X}>\n", cid + 1, unicode));
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

/// Helper: Parse a hex token of the form "<...>" and return the number.
fn parse_hex_token(token: &str) -> Result<u32, String> {
    let token = token.trim();
    if token.len() < 2 {
        return Err("Hex token too short".into());
    }
    if token.starts_with('<') && token.ends_with('>') {
        let inner = &token[1..token.len() - 1];
        u32::from_str_radix(inner, 16)
            .map_err(|e| format!("Failed to parse hex token {}: {}", token, e))
    } else {
        Err(format!("Expected token enclosed in <>: {}", token))
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
