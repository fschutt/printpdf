//! Moves the big base64 payloads (fonts, image pixels) out of the JSON document.
//!
//! A document with a 4 MB font is unusable in `jq`. `strip` writes every long
//! base64 string to `DIR/<id>.b64` and replaces it with `{"$file": "<id>.b64"}`;
//! `inline` puts the strings back. The file holds the string exactly as it was in
//! the JSON (including a `data:...;base64,` prefix), so the round trip is lossless.
//! `ppdf fonts` and `ppdf images` write the decoded files.

use serde_json::{Map, Value};
use std::fs;
use std::path::{Component, Path};

/// Strings shorter than this stay in the document.
const MIN_LEN: usize = 512;
const KEY: &str = "$file";

fn is_payload(s: &str) -> bool {
    if s.len() < MIN_LEN {
        return false;
    }
    let body = match s.strip_prefix("data:") {
        Some(rest) => match rest.split_once(";base64,") {
            Some((_, body)) => body,
            None => return false,
        },
        None => s,
    };
    body.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'-' | b'_'))
}

/// FNV-1a, 64 bit: a stable name for a payload, the same on every run.
fn name_for(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}.b64")
}

/// Writes every payload under `value` into `dir` and leaves a `$file` reference
/// behind. Returns the number of payloads moved.
pub fn strip(value: &mut Value, dir: &Path) -> Result<usize, String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut count = 0;
    strip_inner(value, dir, &mut count)?;
    Ok(count)
}

fn strip_inner(value: &mut Value, dir: &Path, count: &mut usize) -> Result<(), String> {
    match value {
        Value::String(s) if is_payload(s) => {
            let name = name_for(s);
            let path = dir.join(&name);
            if !path.exists() {
                fs::write(&path, s.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
            }
            let mut reference = Map::new();
            reference.insert(KEY.to_string(), Value::String(name));
            *value = Value::Object(reference);
            *count += 1;
        }
        Value::Array(items) => {
            for item in items {
                strip_inner(item, dir, count)?;
            }
        }
        Value::Object(map) => {
            for (_, item) in map.iter_mut() {
                strip_inner(item, dir, count)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Replaces every `{"$file": "<name>"}` under `value` with the contents of
/// `dir/<name>`. Returns the number of payloads restored.
pub fn inline(value: &mut Value, dir: &Path) -> Result<usize, String> {
    let mut count = 0;
    inline_inner(value, dir, &mut count)?;
    Ok(count)
}

fn reference(map: &Map<String, Value>) -> Option<&str> {
    if map.len() != 1 {
        return None;
    }
    map.get(KEY)?.as_str()
}

/// The JSON comes from anywhere, so a reference may only name a plain file in `dir`.
fn is_plain_file_name(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(_)), None)
    )
}

fn inline_inner(value: &mut Value, dir: &Path, count: &mut usize) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            if let Some(name) = reference(map) {
                if !is_plain_file_name(name) {
                    return Err(format!("{KEY} '{name}' is not a plain file name"));
                }
                let path = dir.join(name);
                let text = fs::read_to_string(&path)
                    .map_err(|e| format!("{KEY} '{name}': {}: {e}", path.display()))?;
                *value = Value::String(text);
                *count += 1;
            } else {
                for (_, item) in map.iter_mut() {
                    inline_inner(item, dir, count)?;
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                inline_inner(item, dir, count)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ppdf-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn strip_then_inline_is_lossless() {
        let payload = "QUJD".repeat(300);
        let uri = format!("data:font/ttf;base64,{payload}");
        let original = json!({
            "fonts": { "f1": { "data": uri, "name": "short text stays" } },
            "pixels": [{ "u8b64": payload }],
            "n": 3
        });

        let dir = temp_dir("roundtrip");
        let mut doc = original.clone();
        assert_eq!(strip(&mut doc, &dir).unwrap(), 2);
        assert_eq!(doc["fonts"]["f1"]["name"], "short text stays");
        assert!(doc["fonts"]["f1"]["data"]["$file"].is_string());

        assert_eq!(inline(&mut doc, &dir).unwrap(), 2);
        assert_eq!(doc, original);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prose_and_short_strings_stay() {
        let text = "a long sentence with spaces ".repeat(40);
        assert!(!is_payload(&text));
        assert!(!is_payload("QUJD"));
    }

    #[test]
    fn a_reference_cannot_leave_the_directory() {
        let dir = temp_dir("escape");
        let mut doc = json!({ "x": { "$file": "../secret" } });
        assert!(inline(&mut doc, &dir).is_err());
        let mut doc = json!({ "x": { "$file": "/etc/passwd" } });
        assert!(inline(&mut doc, &dir).is_err());
    }
}
