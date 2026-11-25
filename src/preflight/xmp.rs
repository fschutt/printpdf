//! XMP metadata: a check that the packet is well-formed XML, and the PDF/A identification.

use super::file::find;

/// The deepest element nesting that is accepted.
const MAX_DEPTH: usize = 256;
/// The namespace of `pdfaid:part` and `pdfaid:conformance`.
const PDFAID_NS: &str = "http://www.aiim.org/pdfa/ns/id/";

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b':' | b'-' | b'.') || b >= 0x80
}

fn is_xml_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

/// The position after the first `needle` at or after `from`.
fn skip_past(b: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    let rest = b.get(from..)?;
    find(rest, needle).map(|p| from.saturating_add(p).saturating_add(needle.len()))
}

/// Checks that the references in `text` are `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;` or a
/// character number.
fn check_entities(text: &[u8], at: usize) -> Result<(), String> {
    let mut i = 0usize;
    while let Some(&byte) = text.get(i) {
        if byte == b'&' {
            let rest = text.get(i.saturating_add(1)..).unwrap_or(&[]);
            let end = rest.iter().take(12).position(|&c| c == b';');
            let name = end.and_then(|e| rest.get(..e));
            let ok = match name {
                Some(b"amp" | b"lt" | b"gt" | b"quot" | b"apos") => true,
                Some(n) => match n.strip_prefix(b"#") {
                    Some(hex) if hex.first() == Some(&b'x') => {
                        hex.len() > 1 && hex.iter().skip(1).all(u8::is_ascii_hexdigit)
                    }
                    Some(dec) => !dec.is_empty() && dec.iter().all(u8::is_ascii_digit),
                    None => false,
                },
                None => false,
            };
            if !ok {
                return Err(format!(
                    "a & at byte {} does not start a valid reference; write &amp;",
                    at.saturating_add(i)
                ));
            }
        }
        i = i.saturating_add(1);
    }
    Ok(())
}

/// Checks that `b` is well-formed XML: tags nest, attributes are quoted, `&` starts a valid
/// reference and there is one root element. It does not read a DTD and does not check names
/// beyond their characters.
pub(super) fn check_well_formed(b: &[u8]) -> Result<(), String> {
    let mut i = 0usize;
    let mut stack: Vec<&[u8]> = Vec::new();
    let mut roots = 0usize;

    // A byte order mark may start the packet.
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        i = 3;
    }

    while let Some(&byte) = b.get(i) {
        if byte != b'<' {
            // Text up to the next tag.
            let end = b
                .get(i..)
                .and_then(|r| r.iter().position(|&c| c == b'<'))
                .map_or(b.len(), |p| i.saturating_add(p));
            let text = b.get(i..end).unwrap_or(&[]);
            if stack.is_empty() && !text.iter().all(|&c| is_xml_space(c)) {
                return Err(format!("text outside of the root element at byte {i}"));
            }
            check_entities(text, i)?;
            i = end;
            continue;
        }

        let rest = b.get(i..).unwrap_or(&[]);
        if rest.starts_with(b"<!--") {
            i = skip_past(b, i.saturating_add(4), b"-->")
                .ok_or_else(|| format!("the comment at byte {i} is not closed"))?;
        } else if rest.starts_with(b"<![CDATA[") {
            i = skip_past(b, i.saturating_add(9), b"]]>")
                .ok_or_else(|| format!("the CDATA section at byte {i} is not closed"))?;
        } else if rest.starts_with(b"<?") {
            i = skip_past(b, i.saturating_add(2), b"?>")
                .ok_or_else(|| format!("the processing instruction at byte {i} is not closed"))?;
        } else if rest.starts_with(b"<!") {
            i = skip_past(b, i.saturating_add(2), b">")
                .ok_or_else(|| format!("the declaration at byte {i} is not closed"))?;
        } else if rest.starts_with(b"</") {
            let name_start = i.saturating_add(2);
            let close = skip_past(b, name_start, b">")
                .ok_or_else(|| format!("the closing tag at byte {i} is not closed"))?;
            let name = b.get(name_start..close.saturating_sub(1)).unwrap_or(&[]);
            let name = name.split(|&c| is_xml_space(c)).next().unwrap_or(&[]);
            match stack.pop() {
                Some(open) if open == name => {}
                Some(open) => {
                    return Err(format!(
                        "the closing tag </{}> at byte {i} does not match <{}>",
                        String::from_utf8_lossy(name),
                        String::from_utf8_lossy(open)
                    ))
                }
                None => return Err(format!("the closing tag at byte {i} has no opening tag")),
            }
            i = close;
        } else {
            // An opening tag: name, attributes, then `>` or `/>`.
            let name_start = i.saturating_add(1);
            let name_len = b
                .get(name_start..)
                .map_or(0, |r| r.iter().take_while(|&&c| is_name_byte(c)).count());
            if name_len == 0 {
                return Err(format!(
                    "a < at byte {i} does not start a tag; write &lt; for the character"
                ));
            }
            let name_end = name_start.saturating_add(name_len);
            let name = b.get(name_start..name_end).unwrap_or(&[]);
            let mut j = name_end;
            let self_closing;
            loop {
                while b.get(j).is_some_and(|&c| is_xml_space(c)) {
                    j = j.saturating_add(1);
                }
                match b.get(j) {
                    None => return Err(format!("the tag at byte {i} is not closed")),
                    Some(b'>') => {
                        self_closing = false;
                        j = j.saturating_add(1);
                        break;
                    }
                    Some(b'/') if b.get(j.saturating_add(1)) == Some(&b'>') => {
                        self_closing = true;
                        j = j.saturating_add(2);
                        break;
                    }
                    Some(_) => {
                        let attr_len = b
                            .get(j..)
                            .map_or(0, |r| r.iter().take_while(|&&c| is_name_byte(c)).count());
                        if attr_len == 0 {
                            return Err(format!("unexpected character in the tag at byte {j}"));
                        }
                        j = j.saturating_add(attr_len);
                        while b.get(j).is_some_and(|&c| is_xml_space(c)) {
                            j = j.saturating_add(1);
                        }
                        if b.get(j) != Some(&b'=') {
                            return Err(format!("an attribute at byte {j} has no value"));
                        }
                        j = j.saturating_add(1);
                        while b.get(j).is_some_and(|&c| is_xml_space(c)) {
                            j = j.saturating_add(1);
                        }
                        let quote = match b.get(j) {
                            Some(&q @ (b'"' | b'\'')) => q,
                            _ => {
                                return Err(format!(
                                    "the attribute value at byte {j} is not quoted"
                                ))
                            }
                        };
                        let value_start = j.saturating_add(1);
                        let len = b
                            .get(value_start..)
                            .and_then(|r| r.iter().position(|&c| c == quote))
                            .ok_or_else(|| {
                                format!("the attribute value at byte {j} is not closed")
                            })?;
                        let value = b
                            .get(value_start..value_start.saturating_add(len))
                            .unwrap_or(&[]);
                        if value.contains(&b'<') {
                            return Err(format!("the attribute value at byte {j} contains a <"));
                        }
                        check_entities(value, value_start)?;
                        j = value_start.saturating_add(len).saturating_add(1);
                    }
                }
            }
            if stack.is_empty() {
                roots = roots.saturating_add(1);
                if roots > 1 {
                    return Err(format!("a second root element starts at byte {i}"));
                }
            }
            if !self_closing {
                if stack.len() >= MAX_DEPTH {
                    return Err("the elements are nested more than 256 levels deep".to_string());
                }
                stack.push(name);
            }
            i = j;
        }
    }

    if let Some(open) = stack.last() {
        return Err(format!(
            "the element <{}> is not closed",
            String::from_utf8_lossy(open)
        ));
    }
    if roots == 0 {
        return Err("there is no root element".to_string());
    }
    Ok(())
}

/// The value after `name` in `text`: an attribute (`name="1"`) or an element (`<name>1</name>`).
fn value_of(text: &str, name: &str) -> Option<String> {
    let b = text.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = find(b.get(from..)?, name.as_bytes()) {
        let start = from.saturating_add(rel);
        let after = start.saturating_add(name.len());
        from = after;
        // `part` must not be a prefix of a longer name.
        if b.get(after).is_some_and(|&c| is_name_byte(c)) {
            continue;
        }
        let mut j = after;
        while b.get(j).is_some_and(|&c| is_xml_space(c)) {
            j = j.saturating_add(1);
        }
        match b.get(j) {
            Some(b'=') => {
                j = j.saturating_add(1);
                while b.get(j).is_some_and(|&c| is_xml_space(c)) {
                    j = j.saturating_add(1);
                }
                let quote = *b.get(j)?;
                if quote != b'"' && quote != b'\'' {
                    continue;
                }
                let vs = j.saturating_add(1);
                let len = b.get(vs..)?.iter().position(|&c| c == quote)?;
                let v = b.get(vs..vs.saturating_add(len))?;
                return Some(String::from_utf8_lossy(v).trim().to_string());
            }
            Some(b'>') => {
                let vs = j.saturating_add(1);
                let len = b.get(vs..)?.iter().position(|&c| c == b'<')?;
                let v = b.get(vs..vs.saturating_add(len))?;
                return Some(String::from_utf8_lossy(v).trim().to_string());
            }
            _ => continue,
        }
    }
    None
}

/// The prefix that the packet binds to the PDF/A identification namespace, `pdfaid` if none.
fn pdfaid_prefix(text: &str) -> String {
    let b = text.as_bytes();
    if let Some(pos) = find(b, PDFAID_NS.as_bytes()) {
        let before = b.get(pos.saturating_sub(48)..pos).unwrap_or(&[]);
        let before = String::from_utf8_lossy(before);
        if let Some(x) = before.rfind("xmlns:") {
            let rest = before.get(x.saturating_add(6)..).unwrap_or("");
            let prefix: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !prefix.is_empty() {
                return prefix;
            }
        }
    }
    "pdfaid".to_string()
}

/// `pdfaid:part` and `pdfaid:conformance` of an XMP packet.
pub(super) fn pdfa_id(text: &str) -> (Option<String>, Option<String>) {
    let prefix = pdfaid_prefix(text);
    (
        value_of(text, &format!("{prefix}:part")),
        value_of(text, &format!("{prefix}:conformance")),
    )
}
