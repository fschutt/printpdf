//! Reading lopdf objects without panics: references that may be dangling, numbers that may be
//! integers or reals, rectangles, text strings.

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use super::Run;

/// The most references that are followed in a row.
const MAX_REF_HOPS: usize = 32;

/// The result of looking up a key or following a reference.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Look<'a> {
    /// No such key, or the value is `null`.
    Absent,
    /// A reference to an object that does not exist (or a chain of references that is too long).
    Dangling(ObjectId),
    /// The object, and the id of the indirect object it was found in.
    Found(Option<ObjectId>, &'a Object),
}

impl<'a> Look<'a> {
    pub fn object(self) -> Option<&'a Object> {
        match self {
            Look::Found(_, o) => Some(o),
            _ => None,
        }
    }

    pub fn id(self) -> Option<ObjectId> {
        match self {
            Look::Found(id, _) => id,
            Look::Dangling(id) => Some(id),
            Look::Absent => None,
        }
    }

    pub fn is_absent(self) -> bool {
        matches!(self, Look::Absent)
    }

    /// The dictionary of a dictionary or a stream.
    pub fn dict(self) -> Option<&'a Dictionary> {
        self.object().and_then(super::dict_of)
    }

    pub fn name(self) -> Option<&'a [u8]> {
        match self.object() {
            Some(Object::Name(n)) => Some(n),
            _ => None,
        }
    }

    pub fn array(self) -> Option<&'a Vec<Object>> {
        match self.object() {
            Some(Object::Array(a)) => Some(a),
            _ => None,
        }
    }

    pub fn number(self) -> Option<f64> {
        self.object().and_then(number)
    }

    pub fn integer(self) -> Option<i64> {
        match self.object() {
            Some(Object::Integer(i)) => Some(*i),
            _ => None,
        }
    }

    pub fn boolean(self) -> Option<bool> {
        match self.object() {
            Some(Object::Boolean(b)) => Some(*b),
            _ => None,
        }
    }

    pub fn string(self) -> Option<&'a [u8]> {
        match self.object() {
            Some(Object::String(s, _)) => Some(s),
            _ => None,
        }
    }
}

/// An integer or real as a float. Not NaN or infinite.
pub(crate) fn number(o: &Object) -> Option<f64> {
    let v = match o {
        Object::Integer(i) => *i as f64,
        Object::Real(r) => f64::from(*r),
        _ => return None,
    };
    v.is_finite().then_some(v)
}

/// Follows the references of `o`. `null` is `Absent`.
pub(crate) fn resolve<'a>(doc: &'a Document, o: &'a Object) -> Look<'a> {
    let mut current = o;
    let mut id = None;
    for _ in 0..MAX_REF_HOPS {
        match current {
            Object::Reference(r) => match doc.objects.get(r) {
                Some(next) => {
                    id = Some(*r);
                    current = next;
                }
                None => return Look::Dangling(*r),
            },
            Object::Null => return Look::Absent,
            other => return Look::Found(id, other),
        }
    }
    match id {
        Some(i) => Look::Dangling(i),
        None => Look::Absent,
    }
}

/// The value of `key` in `d`, with references followed.
pub(crate) fn look<'a>(doc: &'a Document, d: &'a Dictionary, key: &str) -> Look<'a> {
    match d.get(key.as_bytes()) {
        Ok(o) => resolve(doc, o),
        Err(_) => Look::Absent,
    }
}

/// A rectangle `[x0 y0 x1 y1]` as `[left, bottom, right, top]`, normalised.
pub(crate) fn rect(doc: &Document, o: &Object) -> Option<[f64; 4]> {
    let Look::Found(_, Object::Array(items)) = resolve(doc, o) else {
        return None;
    };
    if items.len() != 4 {
        return None;
    }
    let mut v = [0.0f64; 4];
    for (slot, item) in v.iter_mut().zip(items) {
        *slot = resolve(doc, item).number()?;
    }
    let [x0, y0, x1, y1] = v;
    Some([x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)])
}

/// The bytes of a name as text, for messages.
pub(crate) fn name_text(n: &[u8]) -> String {
    String::from_utf8_lossy(n).into_owned()
}

/// The text of a PDF text string: UTF-16BE with a byte order mark, or one byte per character.
pub(crate) fn text_string(s: &[u8]) -> String {
    match s.strip_prefix(&[0xFE, 0xFF]) {
        Some(rest) => {
            let units: Vec<u16> = rest
                .chunks_exact(2)
                .map(|c| {
                    u16::from_be_bytes([
                        c.first().copied().unwrap_or(0),
                        c.get(1).copied().unwrap_or(0),
                    ])
                })
                .collect();
            String::from_utf16_lossy(&units)
        }
        None => s.iter().map(|&b| char::from(b)).collect(),
    }
}

/// The filter names of a stream, in order. `None` when `/Filter` is not a name or an array of
/// names.
pub(crate) fn filter_names(doc: &Document, s: &Stream) -> Option<Vec<Vec<u8>>> {
    match look(doc, &s.dict, "Filter") {
        Look::Absent => Some(Vec::new()),
        Look::Found(_, Object::Name(n)) => Some(vec![n.clone()]),
        Look::Found(_, Object::Array(items)) => {
            let mut names = Vec::new();
            for item in items {
                match resolve(doc, item) {
                    Look::Found(_, Object::Name(n)) => names.push(n.clone()),
                    _ => return None,
                }
            }
            Some(names)
        }
        _ => None,
    }
}

impl<'a> Run<'a> {
    /// Follows the references of `o`.
    pub fn resolve<'b>(&self, o: &'b Object) -> Look<'b>
    where
        'a: 'b,
    {
        resolve(self.doc, o)
    }

    /// A rectangle.
    pub fn rect<'b>(&self, o: &'b Object) -> Option<[f64; 4]>
    where
        'a: 'b,
    {
        rect(self.doc, o)
    }
}

/// Which allowance a decoded stream is counted against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Budget {
    /// Content streams, which are parsed.
    Content,
    /// Other streams, decoded only to see whether they decode.
    Validation,
}

/// The result of decoding a stream.
pub(crate) enum Decoded {
    Data(Vec<u8>),
    /// The filters are valid but the data is not, or `/Filter` is malformed.
    Failed(String),
    /// Not decoded: a filter for images, or a bound was reached.
    Skipped,
}

/// The filters that can be decoded without an image decoder.
const DECODABLE: [&[u8]; 5] = [
    b"FlateDecode",
    b"LZWDecode",
    b"ASCII85Decode",
    b"ASCIIHexDecode",
    b"RunLengthDecode",
];

/// Decodes a stream within the bounds of the check.
pub(crate) fn decode(run: &mut Run<'_>, s: &Stream, budget: Budget) -> Decoded {
    let Some(names) = filter_names(run.doc, s) else {
        return Decoded::Failed("/Filter is not a name or an array of names".to_string());
    };
    if names.iter().any(|n| !DECODABLE.contains(&n.as_slice())) {
        return Decoded::Skipped;
    }
    let left = match budget {
        Budget::Content => run.content_left,
        Budget::Validation => run.validation_left,
    };
    let cap = left.min(super::MAX_STREAM_BYTES);
    if names.is_empty() {
        if s.content.len() > cap {
            run.sink
                .limit("the limit of decoded stream bytes was reached");
            return Decoded::Skipped;
        }
        deduct(run, budget, s.content.len());
        return Decoded::Data(s.content.clone());
    }
    match s.decompressed_content_with_limit(cap) {
        Ok(data) => {
            deduct(run, budget, data.len());
            Decoded::Data(data)
        }
        Err(lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })) => {
            run.sink
                .limit("the limit of decoded stream bytes was reached");
            Decoded::Skipped
        }
        Err(e) => Decoded::Failed(e.to_string()),
    }
}

fn deduct(run: &mut Run<'_>, budget: Budget, n: usize) {
    let slot = match budget {
        Budget::Content => &mut run.content_left,
        Budget::Validation => &mut run.validation_left,
    };
    *slot = slot.saturating_sub(n);
}
