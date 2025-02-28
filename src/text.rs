/// PDF Text decoding / encoding and ToUnicode handling
use lopdf::{Object, StringFormat};
use serde_derive::{Deserialize, Serialize};

/// Represents a text segment (decoded as a UTF-8 String) or a spacing adjustment
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TextItem {
    Text(String), // A segment of text
    Offset(i32),  // A spacing adjustment (in thousandths of an em)
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

impl From<i32> for TextItem {
    fn from(n: i32) -> Self {
        TextItem::Offset(n)
    }
}

impl From<i64> for TextItem {
    fn from(n: i64) -> Self {
        TextItem::Offset(n as i32)
    }
}

impl From<f32> for TextItem {
    fn from(n: f32) -> Self {
        TextItem::Offset(n.round() as i32)
    }
}

impl From<f64> for TextItem {
    fn from(n: f64) -> Self {
        TextItem::Offset(n.round() as i32)
    }
}

// Optional: For convenience with small integer literals
impl From<u8> for TextItem {
    fn from(n: u8) -> Self {
        TextItem::Offset(n as i32)
    }
}

/// A trait for mapping raw byte sequences to Unicode using a ToUnicode CMap.
/// (In a full implementation, this would use the actual mapping defined in the PDF.)
pub trait CMap {
    fn map_bytes(&self, bytes: &[u8]) -> String;
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
                items.push(TextItem::Offset(*i as i32));
            }
            Object::Real(r) => {
                items.push(TextItem::Offset(*r as i32));
            }
            _ => {
                // Ignore unsupported types or log a warning.
            }
        }
    }
    items
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
/// and spacing offsets as numbers.
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
                objs.push(Object::Integer(*n as i64));
            }
        }
    }
    objs
}
