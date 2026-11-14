use lopdf::{Object, StringFormat};
use serde_derive::{Deserialize, Serialize};

/// Represents a positioned glyph with optional CID mapping
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct Codepoint {
    /// Glyph ID in the font
    pub gid: u16,
    /// Horizontal offset in thousandths of an em
    pub offset: f32,
    /// Optional CID for CID-keyed fonts (used for ToUnicode mapping)
    pub cid: Option<String>,
}

impl Codepoint {
    pub fn new(gid: u16, offset: f32) -> Self {
        Self { gid, offset, cid: None }
    }
    
    pub fn with_cid(gid: u16, offset: f32, cid: String) -> Self {
        Self { gid, offset, cid: Some(cid) }
    }
}

/// Represents a text segment (decoded as a UTF-8 String) or a spacing adjustment
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TextItem {
    /// A segment of text
    Text(String),
    /// A spacing adjustment, in thousandths of an em
    Offset(f32),
    /// Positioned glyph IDs with horizontal offsets and optional CID mapping
    /// This avoids the need to convert GIDs to strings during parsing
    GlyphIds(Vec<Codepoint>),
}

impl From<String> for TextItem {
    fn from(s: String) -> Self {
        TextItem::Text(s)
    }
}

impl From<&str> for TextItem {
    fn from(s: &str) -> Self {
        TextItem::Text(s.to_string())
    }
}

impl From<f32> for TextItem {
    fn from(n: f32) -> Self {
        TextItem::Offset(n)
    }
}

impl From<f64> for TextItem {
    fn from(n: f64) -> Self {
        TextItem::Offset(n as f32)
    }
}

impl From<i32> for TextItem {
    fn from(n: i32) -> Self {
        TextItem::Offset(n as f32)
    }
}

impl From<i64> for TextItem {
    fn from(n: i64) -> Self {
        TextItem::Offset(n as f32)
    }
}

// Optional: For convenience with small integer literals
impl From<u8> for TextItem {
    fn from(n: u8) -> Self {
        TextItem::Offset(n as f32)
    }
}

/// A trait for mapping raw byte sequences to Unicode using a ToUnicode CMap.
/// (In a full implementation, this would use the actual mapping defined in the PDF.)
pub trait CMap {
    fn map_bytes(&self, bytes: &[u8]) -> String;
}

/// Decode a single WinAnsiEncoding (CP1252) byte to its Unicode character.
///
/// This is the exact inverse of the encoder used for the 14 built-in fonts
/// (`/Encoding /WinAnsiEncoding`, PDF 32000-1 Annex D.2). WinAnsi agrees with
/// ASCII in 0x20..=0x7E and with Latin-1 in 0xA0..=0xFF; 0x80..=0x9F holds
/// typographic punctuation. Unassigned bytes decode to U+FFFD.
pub fn win_ansi_char(b: u8) -> char {
    match b {
        0x20..=0x7E => b as char,
        0xA0..=0xFF => b as char,
        0x80 => '\u{20AC}', // € euro
        0x82 => '\u{201A}', // ‚ single low-9 quote
        0x83 => '\u{0192}', // ƒ florin
        0x84 => '\u{201E}', // „ double low-9 quote
        0x85 => '\u{2026}', // … ellipsis
        0x86 => '\u{2020}', // † dagger
        0x87 => '\u{2021}', // ‡ double dagger
        0x88 => '\u{02C6}', // ˆ circumflex
        0x89 => '\u{2030}', // ‰ per mille
        0x8A => '\u{0160}', // Š S caron
        0x8B => '\u{2039}', // ‹ single left angle quote
        0x8C => '\u{0152}', // Œ OE ligature
        0x8E => '\u{017D}', // Ž Z caron
        0x91 => '\u{2018}', // ' left single quote
        0x92 => '\u{2019}', // ' right single quote
        0x93 => '\u{201C}', // " left double quote
        0x94 => '\u{201D}', // " right double quote
        0x95 => '\u{2022}', // • bullet
        0x96 => '\u{2013}', // – en dash
        0x97 => '\u{2014}', // — em dash
        0x98 => '\u{02DC}', // ˜ small tilde
        0x99 => '\u{2122}', // ™ trademark
        0x9A => '\u{0161}', // š s caron
        0x9B => '\u{203A}', // › single right angle quote
        0x9C => '\u{0153}', // œ oe ligature
        0x9E => '\u{017E}', // ž z caron
        0x9F => '\u{0178}', // Ÿ Y diaeresis
        // 0x00..=0x1F control range and the unassigned 0x81/0x8D/0x8F/0x90/0x9D:
        // tab/newline pass through so text extraction keeps whitespace.
        b'\t' | b'\n' | b'\r' => b as char,
        _ => '\u{FFFD}',
    }
}

/// Decode the bytes of a text-showing operator for a *simple* (one-byte-code)
/// font: each byte is one character code. Codes are looked up in the font's
/// ToUnicode CMap when one exists, falling back to WinAnsiEncoding — the
/// standard encoding of the built-in fonts and the most common simple-font
/// encoding in the wild.
pub fn decode_simple_font_bytes(bytes: &[u8], to_unicode: Option<&crate::cmap::ToUnicodeCMap>) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        if let Some(s) = to_unicode.and_then(|c| c.lookup_string(b as u32)) {
            out.push_str(&s);
        } else {
            out.push(win_ansi_char(b));
        }
    }
    out
}

/// Decode a PDF string (literal or hexadecimal) into a Rust UTF‑8 String.
/// If a ToUnicode CMap is provided, use it to map the raw bytes; otherwise, fallback
/// to assuming the bytes are encoded in WinAnsi (or UTF‑8 when possible).
pub fn decode_pdf_string(obj: &Object, to_unicode: Option<&impl CMap>) -> String {
    if let Object::String(ref bytes, format) = obj {
        match format {
            StringFormat::Literal => {
                // Here you should process escape sequences (\, \(, \), octal codes, etc.).
                // For simplicity, we assume the provided bytes are already unescaped.
                if let Some(cmap) = to_unicode {
                    cmap.map_bytes(bytes)
                } else {
                    String::from_utf8_lossy(bytes).into_owned()
                }
            }
            StringFormat::Hexadecimal => {
                // For hex strings the bytes are the raw binary data.
                if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                    // Contains a BOM – assume UTF-16BE.
                    let utf16_iter = bytes[2..].chunks(2).filter_map(|pair| {
                        if pair.len() == 2 {
                            Some(u16::from_be_bytes([pair[0], pair[1]]))
                        } else {
                            None
                        }
                    });
                    String::from_utf16(&utf16_iter.collect::<Vec<_>>()).unwrap_or_default()
                } else {
                    // Without BOM, use the ToUnicode mapping if available, or fallback.
                    if let Some(cmap) = to_unicode {
                        cmap.map_bytes(bytes)
                    } else {
                        String::from_utf8_lossy(bytes).into_owned()
                    }
                }
            }
        }
    } else {
        String::new()
    }
}

/// Given the operands of a TJ operator (an array of PDF objects),
/// decode them into a Vec<TextItem> where string elements become TextItem::Text
/// (after decoding) and numbers become TextItem::Offset.
pub fn decode_tj_operands(operands: &[Object], to_unicode: Option<&impl CMap>) -> Vec<TextItem> {
    let mut items = Vec::new();
    for obj in operands {
        match obj {
            Object::String(_, _) => {
                let s = decode_pdf_string(obj, to_unicode);
                items.push(TextItem::Text(s));
            }
            Object::Integer(i) => {
                items.push(TextItem::Offset(*i as f32));
            }
            Object::Real(r) => {
                items.push(TextItem::Offset(*r));
            }
            _ => {
                // Ignore unsupported types or log a warning.
            }
        }
    }
    items
}

/// Given the operands of a TJ operator, extract raw glyph IDs without decoding to Unicode.
/// This is more efficient for round-trip PDF editing where text extraction is not needed.
/// Returns TextItem::GlyphIds instead of TextItem::Text.
pub fn decode_tj_operands_as_glyph_ids(operands: &[Object]) -> Vec<TextItem> {
    let mut items = Vec::new();
    let mut current_glyphs = Vec::new();
    
    for obj in operands {
        match obj {
            Object::String(bytes, _) => {
                // Extract glyph IDs from the byte string
                // For CID fonts, each glyph is typically 2 bytes (big-endian u16)
                // For simple fonts, each byte is a glyph ID
                if bytes.len() >= 2 && bytes.len() % 2 == 0 {
                    // Assume CID font (2 bytes per glyph)
                    for chunk in bytes.chunks(2) {
                        if chunk.len() == 2 {
                            let gid = u16::from_be_bytes([chunk[0], chunk[1]]);
                            current_glyphs.push(Codepoint::new(gid, 0.0));
                        }
                    }
                } else {
                    // Simple font (1 byte per glyph)
                    for &byte in bytes {
                        current_glyphs.push(Codepoint::new(byte as u16, 0.0));
                    }
                }
            }
            Object::Integer(i) => {
                // Offset in thousandths of an em
                if !current_glyphs.is_empty() {
                    // Flush current glyphs before adding offset
                    items.push(TextItem::GlyphIds(std::mem::take(&mut current_glyphs)));
                }
                items.push(TextItem::Offset(*i as f32));
            }
            Object::Real(r) => {
                if !current_glyphs.is_empty() {
                    items.push(TextItem::GlyphIds(std::mem::take(&mut current_glyphs)));
                }
                items.push(TextItem::Offset(*r));
            }
            _ => {
                // Ignore unsupported types
            }
        }
    }
    
    // Flush remaining glyphs
    if !current_glyphs.is_empty() {
        items.push(TextItem::GlyphIds(current_glyphs));
    }
    
    items
}

/// Decode Tj operator string as raw glyph IDs
pub fn decode_tj_string_as_glyph_ids(bytes: &[u8]) -> Vec<TextItem> {
    let mut glyphs = Vec::new();
    
    // Extract glyph IDs from the byte string
    if bytes.len() >= 2 && bytes.len().is_multiple_of(2) {
        // Assume CID font (2 bytes per glyph)
        for chunk in bytes.chunks(2) {
            if chunk.len() == 2 {
                let gid = u16::from_be_bytes([chunk[0], chunk[1]]);
                glyphs.push(Codepoint::new(gid, 0.0));
            }
        }
    } else {
        // Simple font (1 byte per glyph)
        for &byte in bytes {
            glyphs.push(Codepoint::new(byte as u16, 0.0));
        }
    }
    
    if glyphs.is_empty() {
        vec![]
    } else {
        vec![TextItem::GlyphIds(glyphs)]
    }
}

/// Encode a Rust string as a PDF literal string.
/// It surrounds the string with parentheses and escapes special characters.
pub fn encode_pdf_string_literal(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 2);
    result.push('(');
    for c in s.chars() {
        match c {
            '(' => result.push_str("\\("),
            ')' => result.push_str("\\)"),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\x08' => result.push_str("\\b"),
            '\x0C' => result.push_str("\\f"),
            _ => result.push(c),
        }
    }
    result.push(')');
    result
}

/// Encode a Rust string as a PDF hex string.
/// The string is encoded as UTF-16BE with a BOM (0xFEFF) and then output as hex.
pub fn encode_pdf_string_hex(s: &str) -> String {
    // Encode as UTF-16BE with BOM.
    let mut utf16: Vec<u16> = Vec::new();
    utf16.push(0xFEFF); // BOM
    utf16.extend(s.encode_utf16());
    let mut bytes = Vec::with_capacity(utf16.len() * 2);
    for code in utf16 {
        bytes.extend_from_slice(&code.to_be_bytes());
    }
    let hex: String = bytes.iter().map(|b| format!("{:02X}", b)).collect();
    format!("<{}>", hex)
}

/// Given a Rust string, decide whether a literal or hex encoding yields a smaller output.
/// Returns the PDF string representation.
pub fn encode_pdf_string_minimal(s: &str) -> String {
    let literal = encode_pdf_string_literal(s);
    let hex = encode_pdf_string_hex(s);
    if literal.len() <= hex.len() {
        literal
    } else {
        hex
    }
}

/// Encodes a vector of TextItem into a vector of lopdf::Object suitable for a TJ operator.
/// Text segments are encoded as PDF strings (choosing the minimal encoding),
/// spacing offsets as numbers, and glyph IDs as hex strings.
pub fn encode_text_items(items: &[TextItem]) -> Vec<Object> {
    let mut objs = Vec::new();
    for item in items {
        match item {
            TextItem::Text(s) => {
                let pdf_str = encode_pdf_string_minimal(s);
                // Check if the encoding is hex or literal based on its delimiters.
                if pdf_str.starts_with('<') {
                    // For hex, remove the delimiters and convert back to bytes.
                    let inner = &pdf_str[1..pdf_str.len() - 1];
                    let mut bytes = Vec::new();
                    for i in (0..inner.len()).step_by(2) {
                        if i + 2 <= inner.len() {
                            if let Ok(byte) = u8::from_str_radix(&inner[i..i + 2], 16) {
                                bytes.push(byte);
                            }
                        }
                    }
                    objs.push(Object::String(bytes, StringFormat::Hexadecimal));
                } else {
                    // For literal strings, we assume a UTF-8 encoding.
                    objs.push(Object::String(s.as_bytes().to_vec(), StringFormat::Literal));
                }
            }
            TextItem::Offset(n) => {
                // Keep fractional kerning: flooring to Integer lost sub-unit
                // offsets (the serializer's own path always wrote Real).
                objs.push(Object::Real(*n));
            }
            TextItem::GlyphIds(glyphs) => {
                // Encode glyph IDs as hex string (2 bytes per glyph ID)
                // Interleaved with offsets
                for codepoint in glyphs {
                    let bytes = codepoint.gid.to_be_bytes().to_vec();
                    objs.push(Object::String(bytes, StringFormat::Hexadecimal));
                    if codepoint.offset != 0.0 {
                        objs.push(Object::Integer(codepoint.offset as i64));
                    }
                }
            }
        }
    }
    objs
}

/// The `BDC` that opens a `/Span` marked-content sequence with `text` as its `/ActualText`
/// (UTF-16BE with its byte-order mark); close it with [`crate::Op::EndMarkedContent`].
///
/// Copying, searching and accessibility read the span's glyphs as `text`, instead of
/// reconstructing it from the glyphs: from their `/ToUnicode` text, which a glyph shared by two
/// letters has only once (Arabic letters drawn as one body and their dots), and from their
/// positions, which a layout that places every glyph turns into spurious spaces. Saving with
/// [`crate::PdfSaveOptions::optimize`] drops the span again where its glyphs read as `text`
/// anyway ([`crate::optimize_text`]).
pub fn actual_text_span(text: &str) -> crate::Op {
    let mut data = vec![0xFE, 0xFF];
    for u in text.encode_utf16() {
        data.extend_from_slice(&u.to_be_bytes());
    }
    let mut map = std::collections::BTreeMap::new();
    map.insert(
        "ActualText".to_string(),
        crate::DictItem::String {
            data,
            literal: false,
        },
    );
    crate::Op::BeginMarkedContentWithProperties {
        tag: "Span".to_string(),
        properties: crate::DictItem::Dict { map },
    }
}

/// A glyph where a text layout put it - azul's text3 (the glyphs of its positioned clusters),
/// or any other shaper - for [`show_placed_glyphs`] to draw it there.
#[cfg(feature = "text_layout")]
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedGlyph {
    /// Its font: an index into the fonts given to [`show_placed_glyphs`]
    pub font: usize,
    /// Its glyph id in that font
    pub gid: u16,
    /// Where it is drawn: its origin from the start of the baseline, in pt of the text space
    /// (the text matrix in force), x along the baseline, y up from it (a mark raised over its
    /// letter)
    pub x: f32,
    pub y: f32,
    /// The text it stands for in the font's `/ToUnicode` map: its cluster's text on one glyph
    /// of the cluster, empty on the others (a mark, a dot drawn apart from its letter)
    pub text: String,
}

/// The operations that draw `glyphs` - one line of them, left to right, each in its font of
/// `fonts` at `size` - exactly where a text layout put them, inside a text section whose text
/// matrix stands at the start of their baseline (after [`crate::Op::StartTextSection`] and the
/// matrix; the character spacing `Tc` at 0 and the horizontal scaling `Tz` at 100, as a PDF
/// starts):
///
/// - one `TJ` per stretch of one font and one rise, the pen moved to each glyph with a kern
///   against the glyph's width in the font's `/W` - the width printpdf writes there
///   ([`glyph_pdf_width`]), which a PDF viewer moves the pen on by - so kerning, mark offsets,
///   a run drawn right to left and letter spacing all land where the layout put them;
/// - a glyph above or below the baseline (a mark) shown with the text rise `Ts`, which is 0
///   again at the end;
/// - with an `actual_text`, all of it in an [`actual_text_span`] of that text, so the PDF's
///   text reads as the layout's text where the glyphs can not say it.
///
/// `fonts` are each font's resource id (as added to the document) with its parsed face, whose
/// glyph widths the kerns are measured against; a glyph's `font` past their end is the first's.
#[cfg(feature = "text_layout")]
pub fn show_placed_glyphs(
    fonts: &[(crate::FontId, &crate::font::AzulParsedFont)],
    size: crate::Pt,
    glyphs: &[PlacedGlyph],
    actual_text: Option<&str>,
) -> Vec<crate::Op> {
    use crate::Op;

    fn flush(ops: &mut Vec<Op>, items: &mut Vec<TextItem>, run: &mut Vec<Codepoint>) {
        if !run.is_empty() {
            items.push(TextItem::GlyphIds(std::mem::take(run)));
        }
        if !items.is_empty() {
            ops.push(Op::ShowText {
                items: std::mem::take(items),
            });
        }
    }

    let mut ops = Vec::new();
    if fonts.is_empty() || size.0 <= 0.0 || glyphs.is_empty() {
        return ops;
    }
    let span = actual_text.filter(|t| !t.is_empty());
    if let Some(text) = span {
        ops.push(actual_text_span(text));
    }
    // where the viewer's pen stands and the rise in force, in pt from the baseline's start
    let mut pen = 0.0f32;
    let mut rise = 0.0f32;
    let mut current: Option<usize> = None;
    let mut items: Vec<TextItem> = Vec::new();
    let mut run: Vec<Codepoint> = Vec::new();
    for g in glyphs {
        let font = if g.font < fonts.len() { g.font } else { 0 };
        if current != Some(font) {
            flush(&mut ops, &mut items, &mut run);
            ops.push(Op::SetFont {
                font: crate::ops::PdfFontHandle::External(fonts[font].0.clone()),
                size,
            });
            current = Some(font);
        }
        if (g.y - rise).abs() > 1e-5 {
            flush(&mut ops, &mut items, &mut run);
            ops.push(Op::SetLineOffset { multiplier: g.y });
            rise = g.y;
        }
        // a TJ number moves the pen back by a thousandth of the font size
        let kern = (g.x - pen) / size.0 * 1000.0;
        if kern.abs() > 1e-3 {
            match run.last_mut() {
                Some(last) => last.offset -= kern,
                None => items.push(TextItem::Offset(-kern)),
            }
        }
        run.push(Codepoint {
            gid: g.gid,
            offset: 0.0,
            cid: Some(g.text.clone()),
        });
        pen = g.x + glyph_pdf_width(fonts[font].1, g.gid) as f32 / 1000.0 * size.0;
    }
    flush(&mut ops, &mut items, &mut run);
    if rise != 0.0 {
        ops.push(Op::SetLineOffset { multiplier: 0.0 });
    }
    if span.is_some() {
        ops.push(Op::EndMarkedContent);
    }
    ops
}

/// A glyph's width in the font's `/W` array as printpdf writes it: its advance in thousandths
/// of an em, cut to a whole number. A PDF viewer moves the pen on by it after showing the glyph.
#[cfg(feature = "text_layout")]
pub fn glyph_pdf_width(font: &crate::font::AzulParsedFont, gid: u16) -> i64 {
    let units_per_em = f32::from(font.pdf_font_metrics.units_per_em);
    if units_per_em <= 0.0 {
        return 0;
    }
    font.get_or_decode_glyph(gid)
        .map(|g| (f32::from(g.horz_advance) * 1000.0 / units_per_em) as i64)
        .unwrap_or(0)
}
