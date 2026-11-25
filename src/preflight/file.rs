//! Checks of the bytes of the file: the header, the end of the file and the cross-reference table.

use lopdf::xref::{XrefEntry, XrefRecovery, XrefType};
use lopdf::Object;

use super::util::look;
use super::{Loc, Run, Severity, Sink};

/// How far into the file the header may be, and how far from its end `startxref` and `%%EOF`
/// are searched. The same 1024 bytes that Acrobat reads.
const WINDOW: usize = 1024;
/// The most cross-reference entries that are compared with the file.
const MAX_XREF_ENTRIES: usize = 500_000;

/// The first position of `needle` in `haystack`.
pub(super) fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// The last position of `needle` in `haystack`.
pub(super) fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(haystack.len());
    }
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

/// Reads `major.minor` from the bytes after `%PDF-`.
fn parse_version(after: &[u8]) -> Option<(u32, u32)> {
    let mut parts = [0u32; 2];
    let mut idx = 0usize;
    let mut digits = 0usize;
    for &b in after.iter().take(12) {
        match b {
            b'0'..=b'9' => {
                let slot = parts.get_mut(idx)?;
                *slot = slot.checked_mul(10)?.checked_add(u32::from(b - b'0'))?;
                digits += 1;
            }
            b'.' if idx == 0 && digits > 0 => {
                idx = 1;
                digits = 0;
            }
            _ => break,
        }
    }
    let [major, minor] = parts;
    (idx == 1 && digits > 0).then_some((major, minor))
}

/// Checks the header, the end of the file and `startxref`. Returns the header version.
pub(super) fn check_header_and_tail(sink: &mut Sink, bytes: &[u8]) -> Option<(u32, u32)> {
    let profile = sink.profile;
    let head = bytes.get(..bytes.len().min(WINDOW)).unwrap_or(bytes);
    let mut version = None;

    match find(head, b"%PDF-") {
        None => sink.error(
            "header-missing",
            Loc::default(),
            "the file does not start with %PDF- (not found in the first 1024 bytes)".to_string(),
        ),
        Some(pos) => {
            if pos > 0 {
                let sev = if profile.is_pdfa() {
                    Severity::Error
                } else {
                    Severity::Warning
                };
                sink.push(
                    sev,
                    "header-not-at-start",
                    Loc::default(),
                    format!("the %PDF- header starts at byte {pos}, not at byte 0"),
                );
            }
            let after = bytes.get(pos.saturating_add(5)..).unwrap_or(&[]);
            version = parse_version(after);
            match version {
                None => sink.warn(
                    "header-version-invalid",
                    Loc::default(),
                    "the header has no readable version number".to_string(),
                ),
                Some(v) => {
                    if !matches!(v, (1, 0..=7) | (2, 0)) {
                        sink.warn(
                            "header-version-invalid",
                            Loc::default(),
                            format!(
                                "the header version {}.{} is not 1.0 to 1.7 or 2.0",
                                v.0, v.1
                            ),
                        );
                    }
                    check_version_against_standard(sink, v);
                }
            }
            if profile.is_pdfa() {
                check_binary_comment(sink, bytes, pos);
            }
        }
    }

    check_tail(sink, bytes);
    version
}

fn check_version_against_standard(sink: &mut Sink, v: (u32, u32)) {
    let profile = sink.profile;
    let (Some(base), true) = (profile.base_version(), profile.is_standard()) else {
        return;
    };
    // The newest header the standard lets through at all, and the version it is built on.
    let hard_max = if profile.is_pdfx() { (1, 4) } else { (1, 7) };
    if v > hard_max {
        sink.error(
            "pdf-version-too-new",
            Loc::default(),
            format!(
                "the header version is {}.{}; {} allows at most {}.{}",
                v.0,
                v.1,
                profile.label(),
                hard_max.0,
                hard_max.1
            ),
        );
    } else if v > base {
        sink.warn(
            "pdf-version-too-new",
            Loc::default(),
            format!(
                "the header version is {}.{}, but {} is built on PDF {}.{}; features of the newer version are not allowed",
                v.0, v.1, profile.label(), base.0, base.1
            ),
        );
    }
}

/// A comment of at least four bytes above 127 on the line after the header.
fn check_binary_comment(sink: &mut Sink, bytes: &[u8], header_pos: usize) {
    let rest = bytes.get(header_pos..).unwrap_or(&[]);
    let eol = rest.iter().position(|&b| b == b'\n' || b == b'\r');
    let line2 = eol.and_then(|p| {
        let tail = rest.get(p..)?;
        let skip = tail
            .iter()
            .take_while(|&&b| b == b'\n' || b == b'\r')
            .count();
        tail.get(skip..)
    });
    let ok = line2.is_some_and(|l| {
        l.first() == Some(&b'%') && l.iter().skip(1).take(4).filter(|&&b| b >= 128).count() == 4
    });
    if !ok {
        sink.error(
            "header-binary-comment-missing",
            Loc::default(),
            "the line after the header is not a comment of at least four bytes above 127"
                .to_string(),
        );
    }
}

fn check_tail(sink: &mut Sink, bytes: &[u8]) {
    let start = bytes.len().saturating_sub(WINDOW);
    let tail = bytes.get(start..).unwrap_or(&[]);

    match rfind(tail, b"%%EOF") {
        None => {
            let sev = sink
                .forbidden(Some(Severity::Warning))
                .unwrap_or(Severity::Warning);
            sink.push(
                sev,
                "eof-marker-missing",
                Loc::default(),
                "there is no %%EOF marker in the last 1024 bytes".to_string(),
            );
        }
        Some(p) => {
            let after = tail.get(p.saturating_add(5)..).unwrap_or(&[]);
            let extra = after
                .iter()
                .skip_while(|&&b| b == b'\r' || b == b'\n')
                .count();
            // One end-of-line marker may follow. More than that is data after the marker.
            let eols = after.len().saturating_sub(extra);
            if sink.profile.is_pdfa() && (extra > 0 || eols > 2) {
                sink.error(
                    "data-after-eof",
                    Loc::default(),
                    format!("{} bytes follow the last %%EOF marker", after.len()),
                );
            }
        }
    }

    if rfind(tail, b"startxref").is_none() {
        sink.error(
            "startxref-missing",
            Loc::default(),
            "there is no startxref in the last 1024 bytes".to_string(),
        );
    }
}

/// Whether `bytes` at `offset` read `<num> <generation> obj`.
fn object_header_at(bytes: &[u8], offset: usize, num: u32, generation: u16) -> bool {
    let Some(rest) = bytes.get(offset..) else {
        return false;
    };
    let window = rest.get(..rest.len().min(48)).unwrap_or(rest);
    let mut it = window
        .split(|b| b.is_ascii_whitespace())
        .filter(|t| !t.is_empty());
    let number = |t: Option<&[u8]>| -> Option<u64> {
        let t = t?;
        if t.iter().all(u8::is_ascii_digit) {
            std::str::from_utf8(t).ok()?.parse().ok()
        } else {
            None
        }
    };
    number(it.next()) == Some(u64::from(num))
        && number(it.next()) == Some(u64::from(generation))
        && it.next().is_some_and(|t| t.starts_with(b"obj"))
}

/// The result of reading the `/Length` of a stream from the bytes of the file.
enum RawLength {
    Fine,
    Missing,
    /// The declared length, where `endstream` does not follow that many bytes of data.
    Wrong(i64),
    /// The object could not be read this way.
    Unknown,
}

fn is_delimiter_or_space(b: u8) -> bool {
    b.is_ascii_whitespace() || matches!(b, b'/' | b'<' | b'>' | b'[' | b']' | b'(' | b')' | b'%')
}

/// The unsigned integer at the start of `s` after white space, and the rest.
fn read_uint(s: &[u8]) -> Option<(u64, &[u8])> {
    let s = s.get(s.iter().take_while(|b| b.is_ascii_whitespace()).count()..)?;
    let n = s.iter().take_while(|b| b.is_ascii_digit()).count();
    if n == 0 || n > 18 {
        return None;
    }
    let v = std::str::from_utf8(s.get(..n)?).ok()?.parse().ok()?;
    Some((v, s.get(n..)?))
}

/// Reads the stream at `offset` the way a reader does: the dictionary, the `stream` keyword,
/// `/Length` bytes and `endstream`.
fn raw_length(doc: &lopdf::Document, bytes: &[u8], offset: usize) -> RawLength {
    let Some(rest) = bytes.get(offset..) else {
        return RawLength::Unknown;
    };
    let window = rest.get(..rest.len().min(1 << 20)).unwrap_or(rest);

    // The `stream` keyword that ends the dictionary, and where the data starts.
    let mut from = 0usize;
    let (dict_end, data_start) = loop {
        let Some(p) = window.get(from..).and_then(|w| find(w, b"stream")) else {
            return RawLength::Unknown;
        };
        let at = from.saturating_add(p);
        from = at.saturating_add(6);
        let before = window.get(..at).unwrap_or(&[]);
        let trimmed = before.len().saturating_sub(
            before
                .iter()
                .rev()
                .take_while(|b| b.is_ascii_whitespace())
                .count(),
        );
        if trimmed == 0 || window.get(trimmed.saturating_sub(1)) != Some(&b'>') {
            continue;
        }
        let eol = match window.get(from..) {
            Some([b'\r', b'\n', ..]) => 2,
            Some([b'\n', ..] | [b'\r', ..]) => 1,
            _ => continue,
        };
        break (at, offset.saturating_add(from).saturating_add(eol));
    };
    let dict = window.get(..dict_end).unwrap_or(&[]);

    // /Length, as a number or as a reference to one.
    let mut search = 0usize;
    let declared = loop {
        let Some(p) = dict.get(search..).and_then(|d| find(d, b"/Length")) else {
            return RawLength::Missing;
        };
        let after = search.saturating_add(p).saturating_add(7);
        search = after;
        let rest = dict.get(after..).unwrap_or(&[]);
        if !rest.first().is_some_and(|&b| is_delimiter_or_space(b)) {
            // `/Length1` and the like.
            continue;
        }
        let Some((n, tail)) = read_uint(rest) else {
            return RawLength::Unknown;
        };
        // `n g R` is a reference.
        if let Some((g, tail)) = read_uint(tail) {
            let tail = tail
                .get(tail.iter().take_while(|b| b.is_ascii_whitespace()).count()..)
                .unwrap_or(&[]);
            if tail.first() == Some(&b'R') {
                let id = (u32::try_from(n).unwrap_or(0), u16::try_from(g).unwrap_or(0));
                match doc.objects.get(&id) {
                    Some(Object::Integer(v)) => break *v,
                    _ => return RawLength::Unknown,
                }
            }
        }
        break i64::try_from(n).unwrap_or(i64::MAX);
    };

    let Ok(len) = usize::try_from(declared) else {
        return RawLength::Wrong(declared);
    };
    let end = data_start.saturating_add(len);
    let after = bytes.get(end..).unwrap_or(&[]);
    let skip = match after {
        [b'\r', b'\n', ..] => 2,
        [b'\n', ..] | [b'\r', ..] => 1,
        _ => 0,
    };
    if after
        .get(skip..)
        .is_some_and(|a| a.starts_with(b"endstream"))
    {
        RawLength::Fine
    } else {
        RawLength::Wrong(declared)
    }
}

/// `/Length` of every stream, read from the bytes. The loader repairs a wrong length and
/// writes the repaired one into the dictionary, so the loaded document cannot show it.
fn check_stream_lengths(run: &mut Run<'_>, bytes: &[u8]) {
    let doc = run.doc;
    let mut checked = 0usize;
    for (num, entry) in &doc.reference_table.entries {
        let XrefEntry::Normal { offset, generation } = entry else {
            continue;
        };
        let id = (*num, *generation);
        if !matches!(doc.objects.get(&id), Some(Object::Stream(_))) {
            continue;
        }
        if checked >= MAX_XREF_ENTRIES {
            break;
        }
        checked += 1;
        let Ok(offset) = usize::try_from(*offset) else {
            continue;
        };
        match raw_length(doc, bytes, offset) {
            RawLength::Fine | RawLength::Unknown => {}
            RawLength::Missing => run.sink.error(
                "stream-length-missing",
                Loc::obj(id),
                "a stream has no /Length".to_string(),
            ),
            RawLength::Wrong(declared) => run.sink.error(
                "stream-length-mismatch",
                Loc::obj(id),
                format!(
                    "/Length is {declared}, but endstream does not follow that many bytes of data"
                ),
            ),
        }
    }
}

/// The checks that need the loaded document and the bytes: repair of the cross-reference
/// table, entries that point at the wrong place, PDF 1.5 structures, encryption.
pub(super) fn check_loaded(run: &mut Run<'_>, bytes: &[u8]) {
    let doc = run.doc;
    match doc.xref_recovery {
        Some(XrefRecovery::Reconstructed) => run.sink.error(
            "xref-reconstructed",
            Loc::default(),
            "the cross-reference table could not be read; it was rebuilt by scanning the file, \
             so objects may be missing or come from an older revision"
                .to_string(),
        ),
        Some(XrefRecovery::OffsetCorrected) => run.sink.warn(
            "xref-offset-corrected",
            Loc::default(),
            "a startxref or /Prev offset did not point at a cross-reference section; a section \
             found nearby was used"
                .to_string(),
        ),
        _ => {}
    }

    let mut compressed = 0usize;
    let mut checked = 0usize;
    if doc.xref_recovery.is_none() {
        for (num, entry) in &doc.reference_table.entries {
            match entry {
                XrefEntry::Normal { offset, generation } => {
                    if checked >= MAX_XREF_ENTRIES {
                        run.sink.limit("only the first 500,000 cross-reference entries were compared with the file");
                        break;
                    }
                    checked += 1;
                    let ok = usize::try_from(*offset)
                        .is_ok_and(|o| object_header_at(bytes, o, *num, *generation));
                    if !ok {
                        run.sink.once(
                            Severity::Error,
                            "xref-entry-bad-offset",
                            String::new(),
                            Loc::obj((*num, *generation)),
                            format!(
                                "the cross-reference entry of object {num} {generation} points at \
                                 byte {offset}, where that object does not start"
                            ),
                        );
                    }
                }
                XrefEntry::Compressed { .. } => compressed += 1,
                _ => {}
            }
        }
    }

    check_stream_lengths(run, bytes);

    if run.profile().forbids_compressed_structure() {
        let has_objstm = compressed > 0
            || doc
                .objects
                .values()
                .any(|o| matches!(o, Object::Stream(s) if s.dict.has_type(b"ObjStm")));
        if has_objstm {
            run.sink.error(
                "object-stream-forbidden",
                Loc::default(),
                format!(
                    "the file uses object streams, which {} does not allow",
                    run.profile().label()
                ),
            );
        }
        if matches!(
            doc.reference_table.cross_reference_type,
            XrefType::CrossReferenceStream
        ) {
            run.sink.error(
                "xref-stream-forbidden",
                Loc::default(),
                format!(
                    "the file uses a cross-reference stream, which {} does not allow",
                    run.profile().label()
                ),
            );
        }
    }

    let encrypt = look(doc, &doc.trailer, "Encrypt");
    if !encrypt.is_absent() || doc.encryption_state.is_some() {
        match run.profile().is_standard() {
            true => run.sink.error(
                "encrypted",
                Loc::default(),
                format!(
                    "the file is encrypted, which {} does not allow",
                    run.profile().label()
                ),
            ),
            false => run.sink.info(
                "encrypted",
                Loc::default(),
                "the file is encrypted; it opened with an empty password".to_string(),
            ),
        }
    }
}
