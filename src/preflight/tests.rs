//! Tests of the preflight rules.
//!
//! Every rule has a test with a small PDF that triggers it and one that does not. The PDFs are
//! built with `lopdf` directly, so the writer of printpdf does not grade itself.

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream, StringFormat};

use super::*;

// ---------------------------------------------------------------------------------------------
// A builder for small PDFs

struct B {
    doc: Document,
    catalog: ObjectId,
    pages: ObjectId,
    page: ObjectId,
}

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

fn string(s: &str) -> Object {
    Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
}

fn int(i: i64) -> Object {
    Object::Integer(i)
}

fn r(id: ObjectId) -> Object {
    Object::Reference(id)
}

fn nums(v: &[f32]) -> Object {
    Object::Array(v.iter().map(|x| Object::Real(*x)).collect())
}

fn arr(v: Vec<Object>) -> Object {
    Object::Array(v)
}

/// A document with a catalog, a page tree, one page of 200 x 100 units and a file ID.
fn base() -> B {
    let mut doc = Document::with_version("1.4");
    // lopdf writes a cross-reference stream (PDF 1.5) unless it is told to write a table.
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    let pages = doc.new_object_id();
    let page = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => r(pages),
        "MediaBox" => nums(&[0.0, 0.0, 200.0, 100.0]),
    });
    doc.objects.insert(
        pages,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => 1,
            "Kids" => arr(vec![r(page)]),
        }),
    );
    let catalog = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => r(pages),
    });
    doc.trailer.set("Root", r(catalog));
    doc.trailer.set(
        "ID",
        arr(vec![
            Object::String(vec![7; 16], StringFormat::Hexadecimal),
            Object::String(vec![7; 16], StringFormat::Hexadecimal),
        ]),
    );
    B {
        doc,
        catalog,
        pages,
        page,
    }
}

fn dict_mut(doc: &mut Document, id: ObjectId) -> &mut Dictionary {
    match doc.objects.get_mut(&id) {
        Some(Object::Dictionary(d)) => d,
        Some(Object::Stream(s)) => &mut s.dict,
        _ => panic!("object {id:?} has no dictionary"),
    }
}

impl B {
    fn add(&mut self, o: impl Into<Object>) -> ObjectId {
        self.doc.add_object(o)
    }

    fn page(&mut self) -> &mut Dictionary {
        let id = self.page;
        dict_mut(&mut self.doc, id)
    }

    fn catalog(&mut self) -> &mut Dictionary {
        let id = self.catalog;
        dict_mut(&mut self.doc, id)
    }

    fn pages(&mut self) -> &mut Dictionary {
        let id = self.pages;
        dict_mut(&mut self.doc, id)
    }

    /// An uncompressed stream.
    fn stream(&mut self, dict: Dictionary, content: &[u8]) -> ObjectId {
        self.add(Stream::new(dict, content.to_vec()))
    }

    /// Sets the content stream of the page.
    fn content(&mut self, content: &str) {
        let s = self.stream(Dictionary::new(), content.as_bytes());
        self.page().set("Contents", r(s));
    }

    /// Sets an entry of a category of the page resources, for example `("Font", "F1", obj)`.
    fn resource(&mut self, category: &str, key: &str, value: Object) {
        let page = self.page();
        if !matches!(page.get(b"Resources"), Ok(Object::Dictionary(_))) {
            page.set("Resources", Object::Dictionary(Dictionary::new()));
        }
        let Ok(Object::Dictionary(res)) = page.get_mut(b"Resources") else {
            unreachable!()
        };
        if !matches!(res.get(category.as_bytes()), Ok(Object::Dictionary(_))) {
            res.set(category, Object::Dictionary(Dictionary::new()));
        }
        let Ok(Object::Dictionary(cat)) = res.get_mut(category.as_bytes()) else {
            unreachable!()
        };
        cat.set(key, value);
    }

    /// A TrueType font with an embedded font file, as /F1 of the page.
    fn embedded_font(&mut self) {
        let file = self.stream(Dictionary::new(), &[0, 1, 0, 0, 0, 0, 0, 0]);
        let desc = self.add(dictionary! {
            "Type" => "FontDescriptor", "FontName" => "AAAAAA+Test",
            "Flags" => 32, "FontFile2" => r(file),
        });
        let font = self.add(dictionary! {
            "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "AAAAAA+Test",
            "FontDescriptor" => r(desc),
        });
        self.resource("Font", "F1", r(font));
    }

    /// The standard font Helvetica, as /F1 of the page.
    fn helvetica(&mut self) {
        let font = self.add(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        self.resource("Font", "F1", r(font));
    }

    /// An ICC profile stream with the given number of components.
    fn icc(&mut self, n: i64) -> ObjectId {
        self.stream(dictionary! { "N" => n }, &icc_bytes(n))
    }

    /// An output intent of the given kind with a destination profile of `n` components.
    fn output_intent(&mut self, subtype: &str, n: i64) {
        let profile = self.icc(n);
        let oi = self.add(dictionary! {
            "Type" => "OutputIntent", "S" => subtype,
            "OutputConditionIdentifier" => string("Test"),
            "DestOutputProfile" => r(profile),
        });
        self.catalog().set("OutputIntents", arr(vec![r(oi)]));
    }

    fn info(&mut self, d: Dictionary) {
        let id = self.add(d);
        self.doc.trailer.set("Info", r(id));
    }

    /// Adds an annotation to the page.
    fn annot(&mut self, d: Dictionary) -> ObjectId {
        let id = self.add(d);
        let page = self.page();
        let mut list = match page.get(b"Annots") {
            Ok(Object::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        list.push(r(id));
        page.set("Annots", arr(list));
        id
    }

    /// An ExtGState, which a content stream selects with /GS1 gs.
    fn gstate(&mut self, d: Dictionary) {
        self.resource("ExtGState", "GS1", Object::Dictionary(d));
        self.content("/GS1 gs");
    }

    /// A page whose standards-relevant boxes are in order, for the PDF/X profiles.
    fn trim_box(&mut self) {
        self.page().set("TrimBox", nums(&[10.0, 10.0, 190.0, 90.0]));
    }

    fn bytes(mut self) -> Vec<u8> {
        let mut out = Vec::new();
        self.doc.save_to(&mut out).expect("save");
        out
    }
}

fn icc_bytes(n: i64) -> Vec<u8> {
    let mut v = vec![0u8; 256];
    let sig: &[u8] = match n {
        1 => b"GRAY",
        3 => b"RGB ",
        _ => b"CMYK",
    };
    v[16..20].copy_from_slice(sig);
    v[36..40].copy_from_slice(b"acsp");
    v
}

fn build(f: impl FnOnce(&mut B)) -> Vec<u8> {
    let mut b = base();
    f(&mut b);
    b.bytes()
}

fn rules_of(bytes: &[u8], profile: PreflightProfile) -> Vec<(&'static str, Severity)> {
    preflight_bytes(bytes, profile)
        .findings
        .iter()
        .map(|f| (f.rule, f.severity))
        .collect()
}

#[track_caller]
fn assert_flags(bytes: &[u8], profile: PreflightProfile, rule: &str) {
    let report = preflight_bytes(bytes, profile);
    assert!(
        report.findings.iter().any(|f| f.rule == rule),
        "{rule} was not reported for {profile:?}; findings: {:#?}",
        report.findings
    );
}

#[track_caller]
fn assert_not_flagged(bytes: &[u8], profile: PreflightProfile, rule: &str) {
    let report = preflight_bytes(bytes, profile);
    assert!(
        !report.findings.iter().any(|f| f.rule == rule),
        "{rule} was reported for {profile:?} but should not be; findings: {:#?}",
        report.findings
    );
}

/// A test with a document that triggers `rule` and one that does not.
macro_rules! rule_test {
    ($test:ident, $rule:literal, $profile:expr, bad: $bad:expr, good: $good:expr) => {
        #[test]
        fn $test() {
            assert_flags(&build($bad), $profile, $rule);
            assert_not_flagged(&build($good), $profile, $rule);
        }
    };
}

/// A test that changes the bytes of a valid file.
macro_rules! bytes_test {
    ($test:ident, $rule:literal, $profile:expr, $mutate:expr) => {
        #[test]
        fn $test() {
            let good = build(|_| {});
            let mutate: fn(Vec<u8>) -> Vec<u8> = $mutate;
            assert_flags(&mutate(good.clone()), $profile, $rule);
            assert_not_flagged(&good, $profile, $rule);
        }
    };
}

fn replace(mut bytes: Vec<u8>, from: &[u8], to: &[u8]) -> Vec<u8> {
    let pos = bytes
        .windows(from.len())
        .position(|w| w == from)
        .unwrap_or_else(|| panic!("{:?} not found", String::from_utf8_lossy(from)));
    bytes.splice(pos..pos + from.len(), to.iter().copied());
    bytes
}

use PreflightProfile::{General, PdfA1b, PdfA2b, PdfX1a_2001, PdfX3_2002};

// ---------------------------------------------------------------------------------------------
// The base file, and the report itself

#[test]
fn base_file_has_no_findings_in_the_general_profile() {
    let report = preflight_bytes(&build(|_| {}), General);
    assert_eq!(report.findings, Vec::new(), "{:#?}", report.findings);
    assert!(!report.has_errors());
    assert_eq!(report.summary.pages_checked, 1);
    assert!(!report.not_checked.is_empty());
}

#[test]
fn rule_ids_are_unique_kebab_case_and_each_has_a_test() {
    let mut ids: Vec<&str> = RULES.iter().map(|r| r.id).collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total, "duplicate rule id");
    for id in &ids {
        assert!(
            id.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "{id} is not a kebab-case id"
        );
        // `writer-warning` only comes from `PdfDocument::preflight` and is tested in
        // tests/preflight.rs.
        if *id != "writer-warning" {
            assert!(
                include_str!("tests.rs").contains(&format!("\"{id}\"")),
                "no test mentions {id}"
            );
        }
    }
}

#[test]
fn findings_serialize_to_json() {
    let report = preflight_bytes(
        &build(|b| b.page().remove(b"MediaBox").map(|_| ()).unwrap_or(())),
        General,
    );
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["profile"], "general");
    assert_eq!(json["findings"][0]["rule"], "mediabox-missing");
    assert_eq!(json["findings"][0]["severity"], "error");
    assert_eq!(json["findings"][0]["page"], 1);
    assert!(json["summary"]["errors"].as_u64().unwrap() >= 1);
}

#[test]
fn garbage_is_unreadable_not_a_panic() {
    for bytes in [&b""[..], b"hello", b"%PDF-1.4\n", &[0u8; 5000][..]] {
        let report = preflight_bytes(bytes, General);
        assert!(report.has_errors());
        assert!(report.findings.iter().any(|f| f.rule == "file-unreadable"));
    }
}

#[test]
fn profile_names_round_trip() {
    for p in PreflightProfile::ALL {
        assert_eq!(PreflightProfile::from_name(p.name()), Some(p));
    }
    assert_eq!(PreflightProfile::from_name("pdfx-9"), None);
}

// ---------------------------------------------------------------------------------------------
// File structure

#[test]
fn file_unreadable() {
    assert_flags(b"not a pdf at all", General, "file-unreadable");
    assert_not_flagged(&build(|_| {}), General, "file-unreadable");
}

bytes_test!(header_missing, "header-missing", General, |v| replace(
    v, b"%PDF-", b"%PDX-"
));

#[test]
fn header_not_at_start() {
    let good = build(|_| {});
    let mut bad = b"junk\n".to_vec();
    bad.extend_from_slice(&good);
    assert_flags(&bad, PdfA1b, "header-not-at-start");
    assert_not_flagged(&good, PdfA1b, "header-not-at-start");
    // Only a warning for the General profile.
    assert!(rules_of(&bad, General).contains(&("header-not-at-start", Severity::Warning)));
}

bytes_test!(
    header_version_invalid,
    "header-version-invalid",
    General,
    |v| replace(v, b"%PDF-1.4", b"%PDF-1.9")
);

#[test]
fn header_binary_comment_missing() {
    let good = build(|_| {});
    assert!(
        good.starts_with(b"%PDF-1.4\n%"),
        "lopdf writes a binary comment"
    );
    let bad = replace(good.clone(), b"%PDF-1.4\n%", b"%PDF-1.4\n#");
    assert_flags(&bad, PdfA1b, "header-binary-comment-missing");
    assert_not_flagged(&good, PdfA1b, "header-binary-comment-missing");
    // A comment with ASCII bytes only is not enough either.
    let ascii = replace(good, b"%PDF-1.4\n%", b"%PDF-1.4\n%");
    let mut ascii = ascii;
    for b in ascii.iter_mut().skip(10).take(4) {
        *b = b'a';
    }
    assert_flags(&ascii, PdfA1b, "header-binary-comment-missing");
}

#[test]
fn pdf_version_too_new() {
    let at = |v: &str| build(|b| b.doc.version = v.to_string());
    // PDF/X is built on PDF 1.3; 1.4 is a warning and 1.5 an error.
    assert!(rules_of(&at("1.4"), PdfX3_2002).contains(&("pdf-version-too-new", Severity::Warning)));
    assert!(rules_of(&at("1.5"), PdfX3_2002).contains(&("pdf-version-too-new", Severity::Error)));
    assert_not_flagged(&at("1.3"), PdfX3_2002, "pdf-version-too-new");
    // PDF/A-1 is built on 1.4, PDF/A-2 on 1.7.
    assert!(rules_of(&at("1.6"), PdfA1b).contains(&("pdf-version-too-new", Severity::Warning)));
    assert_not_flagged(&at("1.4"), PdfA1b, "pdf-version-too-new");
    assert_not_flagged(&at("1.7"), PdfA2b, "pdf-version-too-new");
    // The General profile has no limit.
    assert_not_flagged(&at("1.7"), General, "pdf-version-too-new");
}

bytes_test!(eof_marker_missing, "eof-marker-missing", General, |v| {
    replace(v, b"%%EOF", b"%%EOX")
});

#[test]
fn data_after_eof() {
    let good = build(|_| {});
    let mut bad = good.clone();
    bad.extend_from_slice(b"garbage after the end");
    assert_flags(&bad, PdfA1b, "data-after-eof");
    assert_not_flagged(&good, PdfA1b, "data-after-eof");
    let mut one_eol = good;
    one_eol.extend_from_slice(b"\n");
    assert_not_flagged(&one_eol, PdfA1b, "data-after-eof");
}

bytes_test!(
    startxref_missing,
    "startxref-missing",
    General,
    |v| replace(v, b"startxref", b"startxreX")
);

#[test]
fn xref_reconstructed() {
    let good = build(|_| {});
    let bad = replace(good.clone(), b"\nxref\n", b"\nxreX\n");
    assert_flags(&bad, General, "xref-reconstructed");
    assert_not_flagged(&good, General, "xref-reconstructed");
}

#[test]
fn xref_entry_bad_offset_and_offset_correction() {
    let good = build(|_| {});
    // Insert one byte before the first object: every offset after it is one too small.
    let bad = replace(good.clone(), b"\n1 0 obj", b"\n 1 0 obj");
    let report = preflight_bytes(&bad, General);
    let hit = report.findings.iter().any(|f| {
        matches!(
            f.rule,
            "xref-entry-bad-offset" | "xref-reconstructed" | "xref-offset-corrected"
        )
    });
    assert!(hit, "{:#?}", report.findings);
    assert_not_flagged(&good, General, "xref-entry-bad-offset");
    assert_not_flagged(&good, General, "xref-offset-corrected");
}

#[test]
fn xref_entry_bad_offset_exact() {
    // Change the offset of one entry so that it points at the middle of another object.
    let good = build(|_| {});
    // The entry of object 1 is `0000000015 00000 n`; make it point into the middle of the object.
    let bad = replace(good.clone(), b"0000000015 00000 n", b"0000000020 00000 n");
    let report = preflight_bytes(&bad, PdfA1b);
    assert!(
        report.findings.iter().any(|f| matches!(
            f.rule,
            "xref-entry-bad-offset" | "xref-reconstructed" | "xref-offset-corrected"
        )),
        "{:#?}",
        report.findings
    );
}

rule_test!(trailer_root_missing, "trailer-root-missing", General,
    bad: |b| {
        // A root that is not a dictionary. Without a catalog anywhere, the loader cannot even
        // rebuild the trailer.
        let not_a_catalog = b.add(Object::Array(vec![]));
        b.doc.trailer.set("Root", r(not_a_catalog));
    },
    good: |_| {});

rule_test!(catalog_type_wrong, "catalog-type-wrong", General,
    bad: |b| { b.catalog().remove(b"Type"); },
    good: |_| {});

#[test]
fn trailer_size_wrong() {
    let good = build(|_| {});
    // The base file has three objects, so the trailer says /Size 4.
    let bad = replace(good.clone(), b"/Size 4", b"/Size 1");
    assert_flags(&bad, General, "trailer-size-wrong");
    assert_not_flagged(&good, General, "trailer-size-wrong");
}

#[test]
fn trailer_id_missing() {
    let without = build(|b| {
        b.doc.trailer.remove(b"ID");
    });
    assert!(rules_of(&without, PdfA1b).contains(&("trailer-id-missing", Severity::Error)));
    assert!(rules_of(&without, PdfX3_2002).contains(&("trailer-id-missing", Severity::Warning)));
    assert_not_flagged(&without, General, "trailer-id-missing");
    assert_not_flagged(&build(|_| {}), PdfA1b, "trailer-id-missing");
}

#[test]
fn encrypted() {
    let encrypted = {
        let mut b = base();
        let state = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V1 {
            document: &b.doc,
            owner_password: "owner",
            user_password: "",
            permissions: lopdf::Permissions::all(),
        })
        .unwrap();
        b.doc.encrypt(&state).unwrap();
        b.bytes()
    };
    assert!(rules_of(&encrypted, PdfA1b).contains(&("encrypted", Severity::Error)));
    assert!(rules_of(&encrypted, General).contains(&("encrypted", Severity::Info)));
    assert_not_flagged(&build(|_| {}), PdfA1b, "encrypted");
}

#[test]
fn object_and_xref_streams_forbidden() {
    let modern = {
        let mut b = base();
        let mut out = Vec::new();
        b.doc.save_modern(&mut out).unwrap();
        out
    };
    assert_flags(&modern, PdfA1b, "object-stream-forbidden");
    assert_flags(&modern, PdfX3_2002, "xref-stream-forbidden");
    // PDF/A-2 and the General profile allow them.
    assert_not_flagged(&modern, PdfA2b, "object-stream-forbidden");
    assert_not_flagged(&modern, General, "xref-stream-forbidden");
    assert_not_flagged(&build(|_| {}), PdfA1b, "object-stream-forbidden");
    assert_not_flagged(&build(|_| {}), PdfA1b, "xref-stream-forbidden");
}

rule_test!(dangling_reference, "dangling-reference", General,
    bad: |b| { b.catalog().set("Extra", r((999, 0))); },
    good: |b| { b.catalog().set("Extra", int(1)); });

// ---------------------------------------------------------------------------------------------
// Streams

rule_test!(stream_length_missing, "stream-length-missing", General,
    bad: |b| {
        let id = b.stream(Dictionary::new(), b"q Q");
        dict_mut(&mut b.doc, id).remove(b"Length");
        b.page().set("Contents", r(id));
    },
    good: |b| b.content("q Q"));

rule_test!(stream_length_mismatch, "stream-length-mismatch", General,
    bad: |b| {
        let id = b.stream(Dictionary::new(), b"q Q q Q q Q");
        dict_mut(&mut b.doc, id).set("Length", 3);
        b.page().set("Contents", r(id));
    },
    good: |b| b.content("q Q q Q q Q"));

rule_test!(stream_filter_unknown, "stream-filter-unknown", General,
bad: |b| {
    let id = b.stream(dictionary! { "Filter" => "FooDecode" }, b"q Q");
    b.page().set("Contents", r(id));
},
good: |b| {
    let id = b.stream(dictionary! { "Filter" => "ASCIIHexDecode" }, b"7120 51>");
    b.page().set("Contents", r(id));
});

rule_test!(stream_filter_abbreviation, "stream-filter-unknown", General,
    bad: |b| {
        let id = b.stream(dictionary! { "Filter" => "AHx" }, b"7120 51>");
        b.page().set("Contents", r(id));
    },
    good: |b| b.content("q Q"));

rule_test!(stream_decode_failed, "stream-decode-failed", General,
bad: |b| {
    // A `z` in the middle of a group.
    let id = b.stream(dictionary! { "Filter" => "ASCII85Decode" }, b"abc z~>");
    b.page().set("Contents", r(id));
},
good: |b| {
    let id = b.stream(dictionary! { "Filter" => "ASCII85Decode" }, b"~>");
    b.page().set("Contents", r(id));
});

fn zlib(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

rule_test!(stream_decode_failed_flate, "stream-decode-failed", General,
bad: |b| {
    let mut data = zlib(b"q 1 0 0 1 0 0 cm Q q 1 0 0 1 0 0 cm Q q 1 0 0 1 0 0 cm Q");
    data.truncate(data.len() - 6);
    let id = b.stream(dictionary! { "Filter" => "FlateDecode" }, &data);
    b.page().set("Contents", r(id));
},
good: |b| {
    let data = zlib(b"q 1 0 0 1 0 0 cm Q q 1 0 0 1 0 0 cm Q q 1 0 0 1 0 0 cm Q");
    let id = b.stream(dictionary! { "Filter" => "FlateDecode" }, &data);
    b.page().set("Contents", r(id));
});

rule_test!(stream_decode_failed_flate_checksum, "stream-decode-failed", General,
bad: |b| {
    let mut data = zlib(b"q 1 0 0 1 0 0 cm Q");
    let n = data.len();
    data[n - 1] ^= 0xFF;
    let id = b.stream(dictionary! { "Filter" => "FlateDecode" }, &data);
    b.page().set("Contents", r(id));
},
good: |b| {
    let data = zlib(b"q 1 0 0 1 0 0 cm Q");
    let id = b.stream(dictionary! { "Filter" => "FlateDecode" }, &data);
    b.page().set("Contents", r(id));
});

rule_test!(stream_external_file, "stream-external-file", PdfA1b,
    bad: |b| {
        let id = b.stream(dictionary! { "F" => string("other.bin") }, b"q Q");
        b.page().set("Contents", r(id));
    },
    good: |b| b.content("q Q"));

rule_test!(lzw_filter, "lzw-filter", PdfA1b,
    bad: |b| {
        let id = b.stream(dictionary! { "Filter" => "LZWDecode" }, b"\x80\x0b\x60\x50\x22\x0c\x0c\x85\x01");
        b.page().set("Contents", r(id));
    },
    good: |b| b.content("q Q"));

rule_test!(filter_not_in_base_version, "filter-not-in-base-version", PdfX3_2002,
bad: |b| {
    let id = b.stream(dictionary! { "Filter" => "JPXDecode" }, b"junk");
    b.add(Object::Stream(Stream::new(dictionary! { "Filter" => "JBIG2Decode" }, b"junk".to_vec())));
    b.catalog().set("Extra", r(id));
},
good: |b| {
    let id = b.stream(dictionary! { "Filter" => "DCTDecode" }, b"junk");
    b.catalog().set("Extra", r(id));
});

// ---------------------------------------------------------------------------------------------
// The page tree and the pages

rule_test!(pages_tree_missing, "pages-tree-missing", General,
    bad: |b| { b.catalog().remove(b"Pages"); },
    good: |_| {});

rule_test!(page_tree_cycle, "page-tree-cycle", General,
    bad: |b| {
        let (pages, page) = (b.pages, b.page);
        b.pages().set("Kids", arr(vec![r(pages), r(page)]));
    },
    good: |_| {});

rule_test!(page_tree_node_invalid, "page-tree-node-invalid", General,
    bad: |b| {
        let page = b.page;
        b.pages().set("Kids", arr(vec![r(page), r((998, 0)), int(5)]));
        b.pages().set("Count", 1);
    },
    good: |_| {});

rule_test!(page_count_mismatch, "page-count-mismatch", General,
    bad: |b| { b.pages().set("Count", 7); },
    good: |_| {});

rule_test!(no_pages, "no-pages", General,
    bad: |b| {
        b.pages().set("Kids", arr(vec![]));
        b.pages().set("Count", 0);
    },
    good: |_| {});

rule_test!(mediabox_missing, "mediabox-missing", General,
bad: |b| { b.page().remove(b"MediaBox"); },
good: |b| {
    // A MediaBox that the page inherits from the page tree is fine.
    b.page().remove(b"MediaBox");
    b.pages().set("MediaBox", nums(&[0.0, 0.0, 100.0, 100.0]));
});

rule_test!(mediabox_invalid, "mediabox-invalid", General,
    bad: |b| { b.page().set("MediaBox", nums(&[0.0, 0.0, 100.0])); },
    good: |_| {});

rule_test!(mediabox_without_area, "mediabox-invalid", General,
    bad: |b| { b.page().set("MediaBox", nums(&[0.0, 0.0, 0.0, 100.0])); },
    good: |_| {});

rule_test!(page_box_invalid, "page-box-invalid", General,
    bad: |b| { b.page().set("CropBox", nums(&[1.0, 2.0, 3.0])); },
    good: |b| { b.page().set("CropBox", nums(&[1.0, 2.0, 30.0, 40.0])); });

rule_test!(page_size_huge, "page-size-huge", General,
    bad: |b| { b.page().set("MediaBox", nums(&[0.0, 0.0, 20000.0, 100.0])); },
    good: |b| { b.page().set("MediaBox", nums(&[0.0, 0.0, 14400.0, 100.0])); });

rule_test!(page_size_tiny, "page-size-tiny", General,
    bad: |b| { b.page().set("MediaBox", nums(&[0.0, 0.0, 2.0, 2.0])); },
    good: |_| {});

#[test]
fn page_box_outside_mediabox() {
    let outside = build(|b| b.page().set("TrimBox", nums(&[10.0, 10.0, 300.0, 90.0])));
    assert!(
        rules_of(&outside, PdfX3_2002).contains(&("page-box-outside-mediabox", Severity::Error))
    );
    assert!(rules_of(&outside, General).contains(&("page-box-outside-mediabox", Severity::Warning)));
    let inside = build(|b| b.trim_box());
    assert_not_flagged(&inside, PdfX3_2002, "page-box-outside-mediabox");
}

rule_test!(page_rotate_invalid, "page-rotate-invalid", General,
    bad: |b| { b.page().set("Rotate", 45); },
    good: |b| { b.page().set("Rotate", 90); });

rule_test!(trimbox_missing, "trimbox-missing", PdfX1a_2001,
    bad: |_| {},
    good: |b| b.trim_box());

rule_test!(trimbox_and_artbox, "trimbox-and-artbox", PdfX1a_2001,
bad: |b| {
    b.trim_box();
    b.page().set("ArtBox", nums(&[20.0, 20.0, 180.0, 80.0]));
},
good: |b| {
    b.page().set("ArtBox", nums(&[20.0, 20.0, 180.0, 80.0]));
});

rule_test!(contents_invalid, "contents-invalid", General,
    bad: |b| { b.page().set("Contents", int(5)); },
    good: |b| b.content("q Q"));

rule_test!(contents_dangling, "contents-invalid", General,
    bad: |b| { b.page().set("Contents", r((997, 0))); },
    good: |b| b.content("q Q"));

rule_test!(content_stream_unreadable, "content-stream-unreadable", General,
    bad: |b| b.content("BI /W 5 /H 5 /CS /G /BPC 8 ID"),
    good: |b| b.content("q 1 0 0 1 0 0 cm Q"));

rule_test!(resource_not_found, "resource-not-found", General,
bad: |b| b.content("BT /F9 12 Tf (x) Tj ET"),
good: |b| {
    b.embedded_font();
    b.resource("Font", "F9", r(b.page));
    b.content("BT /F1 12 Tf (x) Tj ET");
});

rule_test!(resource_not_found_by_xobject, "resource-not-found", General,
    bad: |b| b.content("/Nope Do"),
    good: |b| b.content("q Q"));

rule_test!(resource_dangling, "resource-dangling", General,
    bad: |b| b.resource("Font", "F1", r((996, 0))),
    good: |b| b.embedded_font());

rule_test!(resources_malformed, "resources-malformed", General,
    bad: |b| {
        b.page().set("Resources", Object::Dictionary(dictionary! { "Font" => 5 }));
    },
    good: |b| b.embedded_font());

rule_test!(graphics_state_unbalanced_extra_q, "graphics-state-unbalanced", General,
    bad: |b| b.content("q 1 0 0 1 0 0 cm"),
    good: |b| b.content("q 1 0 0 1 0 0 cm Q"));

rule_test!(graphics_state_unbalanced_extra_end, "graphics-state-unbalanced", General,
    bad: |b| b.content("Q"),
    good: |b| b.content("q Q"));

rule_test!(text_object_unbalanced, "text-object-unbalanced", General,
    bad: |b| b.content("BT BT ET"),
    good: |b| b.content("BT ET"));

rule_test!(text_object_unterminated, "text-object-unbalanced", General,
    bad: |b| b.content("BT 1 0 0 1 0 0 Tm"),
    good: |b| b.content("BT 1 0 0 1 0 0 Tm ET"));

rule_test!(text_outside_text_object, "text-outside-text-object", General,
bad: |b| {
    b.embedded_font();
    b.content("/F1 12 Tf (x) Tj");
},
good: |b| {
    b.embedded_font();
    b.content("BT /F1 12 Tf (x) Tj ET");
});

fn form(b: &mut B, dict: Dictionary, content: &str) {
    let mut d = dictionary! { "Type" => "XObject", "Subtype" => "Form" };
    d.extend(&dict);
    let f = b.stream(d, content.as_bytes());
    b.resource("XObject", "Fm1", r(f));
    b.content("/Fm1 Do");
}

rule_test!(form_bbox_missing, "form-bbox-missing", General,
    bad: |b| form(b, Dictionary::new(), "q Q"),
    good: |b| form(b, dictionary! { "BBox" => nums(&[0.0, 0.0, 10.0, 10.0]) }, "q Q"));

rule_test!(xobject_subtype_invalid, "xobject-subtype-invalid", General,
    bad: |b| {
        let s = b.stream(Dictionary::new(), b"x");
        b.resource("XObject", "X1", r(s));
        b.content("/X1 Do");
    },
    good: |b| form(b, dictionary! { "BBox" => nums(&[0.0, 0.0, 10.0, 10.0]) }, "q Q"));

rule_test!(postscript_xobject, "postscript-xobject", PdfA1b,
    bad: |b| {
        let s = b.stream(dictionary! { "Type" => "XObject", "Subtype" => "PS" }, b"x");
        b.resource("XObject", "X1", r(s));
        b.content("/X1 Do");
    },
    good: |b| form(b, dictionary! { "BBox" => nums(&[0.0, 0.0, 10.0, 10.0]) }, "q Q"));

fn image(b: &mut B, extra: Dictionary) {
    let mut d = dictionary! { "Type" => "XObject", "Subtype" => "Image", "BitsPerComponent" => 8 };
    d.extend(&extra);
    let s = b.stream(d, &[0, 0, 0]);
    b.resource("XObject", "Im1", r(s));
    b.content("/Im1 Do");
}

rule_test!(image_dict_incomplete, "image-dict-incomplete", General,
    bad: |b| image(b, dictionary! { "Height" => 1, "ColorSpace" => "DeviceGray" }),
    good: |b| image(b, dictionary! { "Width" => 3, "Height" => 1, "ColorSpace" => "DeviceGray" }));

rule_test!(image_no_colour_space, "image-no-colour-space", General,
    bad: |b| image(b, dictionary! { "Width" => 3, "Height" => 1 }),
    good: |b| image(b, dictionary! { "Width" => 3, "Height" => 1, "ImageMask" => true }));

// ---------------------------------------------------------------------------------------------
// Colour

fn colour_space(b: &mut B, cs: Object) {
    b.resource("ColorSpace", "Cs1", cs);
}

rule_test!(colour_space_malformed_array, "colour-space-malformed", General,
bad: |b| colour_space(b, arr(vec![name("ICCBased")])),
good: |b| {
    let p = b.icc(3);
    colour_space(b, arr(vec![name("ICCBased"), r(p)]));
});

// The failure of #211: a colour space resource that is a dictionary.
rule_test!(colour_space_malformed_dictionary, "colour-space-malformed", General,
    bad: |b| colour_space(b, Object::Dictionary(dictionary! { "N" => 3 })),
    good: |b| colour_space(b, name("DeviceRGB")));

rule_test!(colour_space_malformed_separation, "colour-space-malformed", General,
bad: |b| colour_space(b, arr(vec![name("Separation"), name("Spot")])),
good: |b| {
    let f = b.add(dictionary! { "FunctionType" => 2, "Domain" => nums(&[0.0, 1.0]), "N" => 1 });
    colour_space(b, arr(vec![name("Separation"), name("Spot"), name("DeviceGray"), r(f)]));
});

rule_test!(colour_space_malformed_indexed, "colour-space-malformed", General,
    bad: |b| colour_space(b, arr(vec![name("Indexed"), name("DeviceRGB"), int(1), string("ab")])),
    good: |b| colour_space(b, arr(vec![name("Indexed"), name("DeviceRGB"), int(1), string("abcdef")])));

rule_test!(colour_space_malformed_cal, "colour-space-malformed", General,
bad: |b| colour_space(b, arr(vec![name("CalRGB"), Object::Dictionary(Dictionary::new())])),
good: |b| colour_space(b, arr(vec![
    name("CalRGB"),
    Object::Dictionary(dictionary! { "WhitePoint" => nums(&[0.95, 1.0, 1.09]) }),
])));

rule_test!(colour_space_dangling_profile, "colour-space-malformed", General,
    bad: |b| colour_space(b, arr(vec![name("ICCBased"), r((995, 0))])),
    good: |b| colour_space(b, name("DeviceGray")));

rule_test!(colour_space_unknown, "colour-space-unknown", General,
    bad: |b| colour_space(b, arr(vec![name("CalFoo")])),
    good: |b| colour_space(b, name("Pattern")));

rule_test!(icc_profile_invalid_n, "icc-profile-invalid", General,
bad: |b| {
    let p = b.stream(dictionary! { "N" => 5 }, &icc_bytes(3));
    colour_space(b, arr(vec![name("ICCBased"), r(p)]));
},
good: |b| {
    let p = b.icc(3);
    colour_space(b, arr(vec![name("ICCBased"), r(p)]));
});

rule_test!(icc_profile_invalid_signature, "icc-profile-invalid", General,
bad: |b| {
    let p = b.stream(dictionary! { "N" => 3 }, &[0u8; 200]);
    colour_space(b, arr(vec![name("ICCBased"), r(p)]));
},
good: |b| {
    let p = b.icc(3);
    colour_space(b, arr(vec![name("ICCBased"), r(p)]));
});

rule_test!(icc_profile_invalid_mismatch, "icc-profile-invalid", General,
bad: |b| {
    let p = b.stream(dictionary! { "N" => 4 }, &icc_bytes(3));
    colour_space(b, arr(vec![name("ICCBased"), r(p)]));
},
good: |b| {
    let p = b.icc(4);
    colour_space(b, arr(vec![name("ICCBased"), r(p)]));
});

rule_test!(device_colour_without_output_intent, "device-colour-without-output-intent", PdfA1b,
bad: |b| b.content("1 0 0 rg 0 0 10 10 re f"),
good: |b| {
    b.output_intent("GTS_PDFA1", 3);
    b.content("1 0 0 rg 0 0 10 10 re f");
});

rule_test!(device_colour_needs_matching_profile, "device-colour-without-output-intent", PdfA2b,
bad: |b| {
    // A CMYK profile does not cover DeviceRGB.
    b.output_intent("GTS_PDFA1", 4);
    b.content("1 0 0 rg 0 0 10 10 re f");
},
good: |b| {
    b.output_intent("GTS_PDFA1", 4);
    b.content("0 0 0 1 k 0 0 10 10 re f");
});

rule_test!(device_colour_default_space, "device-colour-without-output-intent", PdfA1b,
bad: |b| b.content("0 g 0 0 10 10 re f"),
good: |b| {
    let p = b.icc(1);
    b.resource("ColorSpace", "DefaultGray", arr(vec![name("ICCBased"), r(p)]));
    b.content("0 g 0 0 10 10 re f");
});

rule_test!(rgb_colour_in_pdfx1a, "rgb-colour-in-pdfx1a", PdfX1a_2001,
    bad: |b| b.content("1 0 0 rg 0 0 10 10 re f"),
    good: |b| b.content("0 0 0 1 k 0 0 10 10 re f"));

rule_test!(colour_space_not_allowed, "colour-space-not-allowed", PdfX1a_2001,
bad: |b| {
    let p = b.icc(3);
    colour_space(b, arr(vec![name("ICCBased"), r(p)]));
    b.content("/Cs1 cs 0 0 0 sc 0 0 10 10 re f");
},
good: |b| {
    colour_space(b, name("DeviceCMYK"));
    b.content("/Cs1 cs 0 0 0 1 sc 0 0 10 10 re f");
});

// ---------------------------------------------------------------------------------------------
// Fonts

#[test]
fn font_not_embedded() {
    let uses_helvetica = build(|b| {
        b.helvetica();
        b.content("BT /F1 12 Tf (x) Tj ET");
    });
    assert!(rules_of(&uses_helvetica, PdfX3_2002).contains(&("font-not-embedded", Severity::Error)));
    assert!(rules_of(&uses_helvetica, PdfA1b).contains(&("font-not-embedded", Severity::Error)));
    // The 14 standard fonts are information in the General profile.
    assert!(rules_of(&uses_helvetica, General).contains(&("font-not-embedded", Severity::Info)));

    let embedded = build(|b| {
        b.embedded_font();
        b.content("BT /F1 12 Tf (x) Tj ET");
    });
    assert_not_flagged(&embedded, PdfX3_2002, "font-not-embedded");

    // A font that is declared but never used is not reported.
    let unused = build(|b| {
        b.helvetica();
        b.content("q Q");
    });
    assert_not_flagged(&unused, PdfX3_2002, "font-not-embedded");
}

#[test]
fn the_font_of_a_text_operator_is_the_one_that_q_restores() {
    let page = |content: &'static str| {
        build(move |b| {
            b.helvetica();
            b.content(content);
        })
    };
    // Selected inside q ... Q and used there: reported.
    assert_flags(
        &page("q BT /F1 12 Tf (x) Tj ET Q"),
        PdfX3_2002,
        "font-not-embedded",
    );
    // Selected inside q ... Q, and the text after Q is shown with the font from before q, which
    // is none: the font is not reported as used.
    assert_not_flagged(
        &page("q BT /F1 12 Tf ET Q BT (x) Tj ET"),
        PdfX3_2002,
        "font-not-embedded",
    );
}

#[test]
fn font_not_embedded_other_font_is_a_warning_in_general() {
    let b = build(|b| {
        let font = b.add(dictionary! {
            "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "Arial",
        });
        b.resource("Font", "F1", r(font));
        b.content("BT /F1 12 Tf (x) Tj ET");
    });
    assert!(rules_of(&b, General).contains(&("font-not-embedded", Severity::Warning)));
    assert!(rules_of(&b, General).contains(&("font-descriptor-missing", Severity::Error)));
}

rule_test!(font_descriptor_missing, "font-descriptor-missing", General,
bad: |b| {
    let font = b.add(dictionary! {
        "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "Foo",
    });
    b.resource("Font", "F1", r(font));
    b.content("BT /F1 12 Tf (x) Tj ET");
},
good: |b| {
    b.helvetica();
    b.content("BT /F1 12 Tf (x) Tj ET");
});

rule_test!(font_program_invalid_dangling, "font-program-invalid", General,
bad: |b| {
    let desc = b.add(dictionary! { "Type" => "FontDescriptor", "FontName" => "Foo", "FontFile2" => r((994, 0)) });
    let font = b.add(dictionary! {
        "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "Foo", "FontDescriptor" => r(desc),
    });
    b.resource("Font", "F1", r(font));
    b.content("BT /F1 12 Tf (x) Tj ET");
},
good: |b| {
    b.embedded_font();
    b.content("BT /F1 12 Tf (x) Tj ET");
});

rule_test!(font_program_invalid_empty, "font-program-invalid", General,
bad: |b| {
    let file = b.stream(Dictionary::new(), b"");
    let desc = b.add(dictionary! { "Type" => "FontDescriptor", "FontName" => "Foo", "FontFile2" => r(file) });
    let font = b.add(dictionary! {
        "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "Foo", "FontDescriptor" => r(desc),
    });
    b.resource("Font", "F1", r(font));
    b.content("BT /F1 12 Tf (x) Tj ET");
},
good: |b| {
    b.embedded_font();
    b.content("BT /F1 12 Tf (x) Tj ET");
});

rule_test!(font_program_invalid_signature, "font-program-invalid", General,
bad: |b| {
    let file = b.stream(Dictionary::new(), b"this is not a font program");
    let desc = b.add(dictionary! { "Type" => "FontDescriptor", "FontName" => "Foo", "FontFile2" => r(file) });
    let font = b.add(dictionary! {
        "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "Foo", "FontDescriptor" => r(desc),
    });
    b.resource("Font", "F1", r(font));
    b.content("BT /F1 12 Tf (x) Tj ET");
},
good: |b| {
    b.embedded_font();
    b.content("BT /F1 12 Tf (x) Tj ET");
});

rule_test!(font_subtype_invalid, "font-subtype-invalid", General,
bad: |b| {
    let font = b.add(dictionary! { "Type" => "Font", "BaseFont" => "Foo" });
    b.resource("Font", "F1", r(font));
    b.content("BT /F1 12 Tf (x) Tj ET");
},
good: |b| {
    b.embedded_font();
    b.content("BT /F1 12 Tf (x) Tj ET");
});

fn type0_font(b: &mut B, to_unicode: bool) {
    let file = b.stream(Dictionary::new(), &[0, 1, 0, 0, 0, 0, 0, 0]);
    let desc = b.add(dictionary! {
        "Type" => "FontDescriptor", "FontName" => "AAAAAA+Test", "Flags" => 4, "FontFile2" => r(file),
    });
    let cid = b.add(dictionary! {
        "Type" => "Font", "Subtype" => "CIDFontType2", "BaseFont" => "AAAAAA+Test",
        "FontDescriptor" => r(desc),
    });
    let mut font = dictionary! {
        "Type" => "Font", "Subtype" => "Type0", "BaseFont" => "AAAAAA+Test",
        "Encoding" => "Identity-H", "DescendantFonts" => arr(vec![r(cid)]),
    };
    if to_unicode {
        let cmap = b.stream(
            Dictionary::new(),
            b"/CIDInit /ProcSet findresource begin end",
        );
        font.set("ToUnicode", r(cmap));
    }
    let font = b.add(font);
    b.resource("Font", "F1", r(font));
    b.content("BT /F1 12 Tf <0001> Tj ET");
}

rule_test!(font_no_tounicode, "font-no-tounicode", General,
    bad: |b| type0_font(b, false),
    good: |b| type0_font(b, true));

// ---------------------------------------------------------------------------------------------
// Transparency and layers

rule_test!(transparency_smask, "transparency-smask", PdfA1b,
    bad: |b| b.gstate(dictionary! { "SMask" => Object::Dictionary(dictionary! { "S" => "Alpha" }) }),
    good: |b| b.gstate(dictionary! { "SMask" => "None" }));

rule_test!(transparency_alpha, "transparency-alpha", PdfX3_2002,
    bad: |b| b.gstate(dictionary! { "ca" => 0.5 }),
    good: |b| b.gstate(dictionary! { "ca" => 1.0 }));

rule_test!(transparency_blend_mode, "transparency-blend-mode", PdfX1a_2001,
    bad: |b| b.gstate(dictionary! { "BM" => "Multiply" }),
    good: |b| b.gstate(dictionary! { "BM" => "Normal" }));

#[test]
fn transparency_is_allowed_in_pdfa2_and_general() {
    let bytes = build(|b| b.gstate(dictionary! { "ca" => 0.5, "BM" => "Multiply" }));
    assert_not_flagged(&bytes, PdfA2b, "transparency-alpha");
    assert_not_flagged(&bytes, General, "transparency-blend-mode");
}

rule_test!(transparency_group_form, "transparency-group", PdfA1b,
    bad: |b| form(b, dictionary! {
        "BBox" => nums(&[0.0, 0.0, 10.0, 10.0]),
        "Group" => Object::Dictionary(dictionary! { "S" => "Transparency" }),
    }, "q Q"),
    good: |b| form(b, dictionary! { "BBox" => nums(&[0.0, 0.0, 10.0, 10.0]) }, "q Q"));

rule_test!(transparency_group_page, "transparency-group", PdfX3_2002,
    bad: |b| { b.page().set("Group", Object::Dictionary(dictionary! { "S" => "Transparency" })); },
    good: |_| {});

rule_test!(transparency_image_smask, "transparency-smask", PdfX3_2002,
    bad: |b| {
        let mask = b.stream(dictionary! { "Subtype" => "Image", "Width" => 1, "Height" => 1,
            "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8 }, &[0]);
        image(b, dictionary! { "Width" => 1, "Height" => 1, "ColorSpace" => "DeviceGray", "SMask" => r(mask) });
    },
    good: |b| image(b, dictionary! { "Width" => 1, "Height" => 1, "ColorSpace" => "DeviceGray" }));

rule_test!(optional_content, "optional-content", PdfA1b,
    bad: |b| {
        let ocg = b.add(dictionary! { "Type" => "OCG", "Name" => string("Layer") });
        b.catalog().set("OCProperties", Object::Dictionary(dictionary! { "OCGs" => arr(vec![r(ocg)]) }));
    },
    good: |_| {});

// ---------------------------------------------------------------------------------------------
// Info dictionary, output intent, metadata

fn x_info() -> Dictionary {
    dictionary! {
        "Title" => string("T"),
        "CreationDate" => string("D:20250101000000Z"),
        "ModDate" => string("D:20250101000000Z"),
        "Trapped" => "False",
        "GTS_PDFXVersion" => string("PDF/X-3:2002"),
    }
}

fn x_info_without(key: &str) -> impl FnOnce(&mut B) {
    let key = key.to_string();
    move |b| {
        let mut d = x_info();
        d.remove(key.as_bytes());
        b.info(d);
    }
}

rule_test!(info_title_missing, "info-title-missing", PdfX3_2002,
    bad: x_info_without("Title"),
    good: |b| b.info(x_info()));

rule_test!(info_date_missing, "info-date-missing", PdfX3_2002,
    bad: x_info_without("ModDate"),
    good: |b| b.info(x_info()));

rule_test!(trapped_missing, "trapped-missing", PdfX3_2002,
    bad: x_info_without("Trapped"),
    good: |b| b.info(x_info()));

rule_test!(trapped_unknown, "trapped-missing", PdfX3_2002,
    bad: |b| {
        let mut d = x_info();
        d.set("Trapped", "Unknown");
        b.info(d);
    },
    good: |b| b.info(x_info()));

rule_test!(pdfx_version_missing, "pdfx-version-missing", PdfX3_2002,
    bad: x_info_without("GTS_PDFXVersion"),
    good: |b| b.info(x_info()));

rule_test!(pdfx_version_empty, "pdfx-version-missing", PdfX3_2002,
    bad: |b| {
        let mut d = x_info();
        d.set("GTS_PDFXVersion", string(""));
        b.info(d);
    },
    good: |b| b.info(x_info()));

rule_test!(pdfx_version_mismatch, "pdfx-version-mismatch", PdfX1a_2001,
bad: |b| b.info(x_info()),
good: |b| {
    let mut d = x_info();
    d.set("GTS_PDFXVersion", string("PDF/X-1a:2001"));
    b.info(d);
});

rule_test!(output_intent_missing, "output-intent-missing", PdfX3_2002,
    bad: |_| {},
    good: |b| b.output_intent("GTS_PDFX", 4));

rule_test!(output_intent_missing_wrong_subtype, "output-intent-missing", PdfX1a_2001,
    bad: |b| b.output_intent("GTS_PDFA1", 4),
    good: |b| b.output_intent("GTS_PDFX", 4));

rule_test!(output_intent_invalid, "output-intent-invalid", PdfX3_2002,
    bad: |b| {
        let oi = b.add(dictionary! { "Type" => "OutputIntent", "S" => "GTS_PDFX" });
        b.catalog().set("OutputIntents", arr(vec![r(oi), int(3)]));
    },
    good: |b| b.output_intent("GTS_PDFX", 4));

rule_test!(output_intent_subtype, "output-intent-subtype", PdfA1b,
    bad: |b| b.output_intent("GTS_PDFX", 3),
    good: |b| b.output_intent("GTS_PDFA1", 3));

#[test]
fn output_intent_profile_missing() {
    let without = |sub: &str| {
        build(|b| {
            let oi = b.add(dictionary! {
                "Type" => "OutputIntent", "S" => sub, "OutputConditionIdentifier" => string("FOGRA39"),
            });
            b.catalog().set("OutputIntents", arr(vec![r(oi)]));
        })
    };
    assert!(rules_of(&without("GTS_PDFA1"), PdfA1b)
        .contains(&("output-intent-profile-missing", Severity::Error)));
    assert!(rules_of(&without("GTS_PDFX"), PdfX3_2002)
        .contains(&("output-intent-profile-missing", Severity::Warning)));
    // PDF/X-1a does not need a profile.
    assert_not_flagged(
        &without("GTS_PDFX"),
        PdfX1a_2001,
        "output-intent-profile-missing",
    );
    assert_not_flagged(
        &build(|b| b.output_intent("GTS_PDFA1", 3)),
        PdfA1b,
        "output-intent-profile-missing",
    );
}

#[test]
fn output_intent_profile_is_checked() {
    let bad = build(|b| {
        let profile = b.stream(dictionary! { "N" => 4 }, &[0u8; 300]);
        let oi = b.add(dictionary! {
            "Type" => "OutputIntent", "S" => "GTS_PDFX",
            "OutputConditionIdentifier" => string("X"), "DestOutputProfile" => r(profile),
        });
        b.catalog().set("OutputIntents", arr(vec![r(oi)]));
    });
    assert_flags(&bad, PdfX3_2002, "icc-profile-invalid");
}

fn xmp(part: &str, conformance: &str) -> String {
    format!(
        "<?xpacket begin=\"\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
         <rdf:Description rdf:about=\"\" xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\">\n\
         <pdfaid:part>{part}</pdfaid:part>\n<pdfaid:conformance>{conformance}</pdfaid:conformance>\n\
         </rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>"
    )
}

fn set_xmp(b: &mut B, text: &str) {
    let s = b.stream(
        dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
        text.as_bytes(),
    );
    b.catalog().set("Metadata", r(s));
}

rule_test!(xmp_missing, "xmp-missing", PdfA1b,
    bad: |_| {},
    good: |b| set_xmp(b, &xmp("1", "B")));

rule_test!(xmp_stream_invalid, "xmp-stream-invalid", PdfA1b,
    bad: |b| {
        let s = b.stream(dictionary! { "Type" => "Metadata" }, xmp("1", "B").as_bytes());
        b.catalog().set("Metadata", r(s));
    },
    good: |b| set_xmp(b, &xmp("1", "B")));

rule_test!(xmp_not_well_formed_unescaped_ampersand, "xmp-not-well-formed", PdfA1b,
    bad: |b| set_xmp(b, &xmp("1", "B").replace("<pdfaid:part>", "<dc:title>A & B</dc:title><pdfaid:part>")),
    good: |b| set_xmp(b, &xmp("1", "B").replace("<pdfaid:part>", "<dc:title>A &amp; B</dc:title><pdfaid:part>")));

rule_test!(xmp_not_well_formed_unclosed, "xmp-not-well-formed", General,
    bad: |b| set_xmp(b, "<x:xmpmeta><rdf:RDF></x:xmpmeta>"),
    good: |b| set_xmp(b, "<x:xmpmeta><rdf:RDF/></x:xmpmeta>"));

rule_test!(pdfa_id_missing, "pdfa-id-missing", PdfA2b,
    bad: |b| set_xmp(b, "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF/></x:xmpmeta>"),
    good: |b| set_xmp(b, &xmp("2", "B")));

rule_test!(pdfa_id_mismatch, "pdfa-id-mismatch", PdfA1b,
    bad: |b| set_xmp(b, &xmp("2", "B")),
    good: |b| set_xmp(b, &xmp("1", "A")));

rule_test!(pdfa_id_mismatch_conformance, "pdfa-id-mismatch", PdfA1b,
    bad: |b| set_xmp(b, &xmp("1", "U")),
    good: |b| set_xmp(b, &xmp("1", "B")));

#[test]
fn pdfa_id_as_attributes_and_other_prefix() {
    let attr = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"r\"><rdf:Description \
                xmlns:id=\"http://www.aiim.org/pdfa/ns/id/\" id:part=\"2\" id:conformance=\"U\"/>\
                </rdf:RDF></x:xmpmeta>";
    let bytes = build(|b| set_xmp(b, attr));
    let rules = rules_of(&bytes, PdfA2b);
    assert!(
        !rules
            .iter()
            .any(|(r, _)| r.starts_with("pdfa-id") || r.starts_with("xmp")),
        "{rules:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Active and external content

fn link(action: Dictionary) -> Dictionary {
    dictionary! {
        "Type" => "Annot", "Subtype" => "Link", "Rect" => nums(&[0.0, 0.0, 10.0, 10.0]),
        "A" => Object::Dictionary(action),
    }
}

#[test]
fn javascript() {
    let js = build(|b| {
        b.annot(link(
            dictionary! { "S" => "JavaScript", "JS" => string("app.alert(1)") },
        ));
    });
    assert!(rules_of(&js, PdfA1b).contains(&("javascript", Severity::Error)));
    assert!(rules_of(&js, PdfX3_2002).contains(&("javascript", Severity::Error)));
    assert!(rules_of(&js, General).contains(&("javascript", Severity::Warning)));
    let goto = build(|b| {
        b.annot(link(dictionary! { "S" => "GoTo", "D" => string("x") }));
    });
    assert_not_flagged(&goto, PdfA1b, "javascript");
}

rule_test!(javascript_name_tree, "javascript", PdfA2b,
bad: |b| {
    b.catalog().set("Names", Object::Dictionary(dictionary! { "JavaScript" => Object::Dictionary(Dictionary::new()) }));
},
good: |b| {
    b.catalog().set("Names", Object::Dictionary(Dictionary::new()));
});

rule_test!(launch_action, "launch-action", PdfA1b,
    bad: |b| { b.annot(link(dictionary! { "S" => "Launch", "F" => string("x.exe") })); },
    good: |b| { b.annot(link(dictionary! { "S" => "URI", "URI" => string("https://example.com") })); });

rule_test!(named_action_not_allowed, "named-action-not-allowed", PdfA1b,
    bad: |b| { b.annot(link(dictionary! { "S" => "Named", "N" => "Print" })); },
    good: |b| { b.annot(link(dictionary! { "S" => "Named", "N" => "NextPage" })); });

rule_test!(remote_goto_action, "remote-goto-action", General,
    bad: |b| { b.annot(link(dictionary! { "S" => "GoToR", "F" => string("other.pdf"), "D" => string("x") })); },
    good: |b| { b.annot(link(dictionary! { "S" => "GoTo", "D" => string("x") })); });

rule_test!(uri_javascript, "uri-javascript", General,
    bad: |b| { b.annot(link(dictionary! { "S" => "URI", "URI" => string("javascript:alert(1)") })); },
    good: |b| { b.annot(link(dictionary! { "S" => "URI", "URI" => string("https://example.com") })); });

#[test]
fn embedded_file() {
    let attached = build(|b| {
        let file = b.stream(dictionary! { "Type" => "EmbeddedFile" }, b"data");
        let spec = b.add(dictionary! { "Type" => "Filespec", "F" => string("a.txt"),
        "EF" => Object::Dictionary(dictionary! { "F" => r(file) }) });
        b.annot(dictionary! {
            "Type" => "Annot", "Subtype" => "FileAttachment", "Rect" => nums(&[0.0, 0.0, 10.0, 10.0]),
            "FS" => r(spec),
        });
    });
    assert!(rules_of(&attached, PdfA1b).contains(&("embedded-file", Severity::Error)));
    assert!(rules_of(&attached, PdfA2b).contains(&("embedded-file", Severity::Warning)));
    assert!(rules_of(&attached, PdfX3_2002).contains(&("embedded-file", Severity::Error)));
    assert!(rules_of(&attached, General).contains(&("embedded-file", Severity::Info)));
    assert_not_flagged(&build(|_| {}), PdfA1b, "embedded-file");
}

fn annot(subtype: &str) -> impl FnOnce(&mut B) {
    let subtype = subtype.to_string();
    move |b| {
        b.annot(dictionary! {
            "Type" => "Annot", "Subtype" => subtype.as_str(), "Rect" => nums(&[0.0, 0.0, 10.0, 10.0]),
        });
    }
}

rule_test!(three_d_content, "3d-content", PdfX3_2002,
    bad: annot("3D"),
    good: annot("Text"));

rule_test!(video_content, "video-content", PdfA2b,
    bad: annot("Movie"),
    good: annot("Text"));

rule_test!(audio_content, "audio-content", PdfA1b,
    bad: annot("Sound"),
    good: annot("Text"));

rule_test!(multimedia_content, "multimedia-content", PdfA1b,
    bad: annot("RichMedia"),
    good: annot("Text"));

#[test]
fn media_is_information_in_the_general_profile() {
    let bytes = build(annot("3D"));
    assert!(rules_of(&bytes, General).contains(&("3d-content", Severity::Info)));
    assert!(rules_of(&bytes, PdfA1b).contains(&("3d-content", Severity::Error)));
}

// ---------------------------------------------------------------------------------------------
// Limits

#[test]
fn deep_page_tree_reaches_the_limit_without_overflowing() {
    let bytes = build(|b| {
        let mut parent = b.pages;
        for _ in 0..200 {
            let node = b.add(
                dictionary! { "Type" => "Pages", "Parent" => r(parent), "Count" => 0,
                "Kids" => arr(vec![]) },
            );
            let mut kids = match dict_mut(&mut b.doc, parent).get(b"Kids") {
                Ok(Object::Array(a)) => a.clone(),
                _ => vec![],
            };
            kids.push(r(node));
            dict_mut(&mut b.doc, parent).set("Kids", arr(kids));
            parent = node;
        }
    });
    let report = preflight_bytes(&bytes, General);
    assert!(report
        .findings
        .iter()
        .any(|f| f.rule == "check-limit-reached"));
    assert!(!report.summary.limits_reached.is_empty());
}

#[test]
fn large_content_stream_is_not_parsed() {
    let big = vec![b' '; content::MAX_PARSE_BYTES + 1];
    let bytes = build(|b| {
        let s = b.stream(Dictionary::new(), &big);
        b.page().set("Contents", r(s));
    });
    assert_flags(&bytes, General, "content-not-checked");
    assert_not_flagged(&build(|b| b.content("q Q")), General, "content-not-checked");
}

#[test]
fn findings_are_capped() {
    let bytes = build(|b| {
        for i in 0..1500u32 {
            b.catalog().set(format!("K{i}"), r((10_000 + i, 0)));
        }
    });
    let report = preflight_bytes(&bytes, General);
    assert!(report.findings.len() <= MAX_FINDINGS + 5);
}

#[test]
fn resources_inherited_from_a_node_are_read_once() {
    let bytes = build(|b| {
        let pages = b.pages;
        let mut kids = Vec::new();
        for _ in 0..50 {
            let p = b.add(dictionary! { "Type" => "Page", "Parent" => r(pages),
            "MediaBox" => nums(&[0.0, 0.0, 10.0, 10.0]) });
            kids.push(r(p));
        }
        b.pages().set("Kids", arr(kids));
        b.pages().set("Count", 50);
        b.pages().set(
            "Resources",
            Object::Dictionary(dictionary! {
                "Font" => Object::Dictionary(dictionary! { "F1" => r((996, 0)) }),
            }),
        );
    });
    let report = preflight_bytes(&bytes, General);
    assert_eq!(
        report.findings_for_rule("resource-dangling").count(),
        1,
        "{:#?}",
        report.findings
    );
    assert_eq!(report.summary.pages_checked, 50);
}

#[test]
fn many_pages_are_checked() {
    let bytes = build(|b| {
        let mut kids = Vec::new();
        let pages = b.pages;
        for _ in 0..300 {
            let p = b.add(dictionary! { "Type" => "Page", "Parent" => r(pages),
            "MediaBox" => nums(&[0.0, 0.0, 10.0, 10.0]) });
            kids.push(r(p));
        }
        b.pages().set("Kids", arr(kids));
        b.pages().set("Count", 300);
    });
    let report = preflight_bytes(&bytes, General);
    assert_eq!(report.summary.pages_checked, 300);
    assert!(!report.has_errors(), "{:#?}", report.findings);
}

// ---------------------------------------------------------------------------------------------
// The standard references

#[test]
fn only_pdfa1b_findings_carry_a_clause() {
    let bytes = build(|b| {
        b.helvetica();
        b.content("BT /F1 12 Tf (x) Tj ET");
    });
    let a1 = preflight_bytes(&bytes, PdfA1b);
    let f = a1
        .findings
        .iter()
        .find(|f| f.rule == "font-not-embedded")
        .unwrap();
    assert_eq!(f.standard_reference, Some("ISO 19005-1 6.3.4"));
    for p in [PdfA2b, PdfX1a_2001, PdfX3_2002, General] {
        let r = preflight_bytes(&bytes, p);
        assert!(
            r.findings.iter().all(|f| f.standard_reference.is_none()),
            "{p:?}"
        );
    }
}

#[test]
fn profile_for_conformance() {
    use crate::PdfConformance as C;
    assert_eq!(
        PreflightProfile::for_conformance(&C::A1B_2005_PDF_1_4),
        Some(PdfA1b)
    );
    assert_eq!(
        PreflightProfile::for_conformance(&C::A2U_2011_PDF_1_7),
        Some(PdfA2b)
    );
    assert_eq!(
        PreflightProfile::for_conformance(&C::X1A_2001_PDF_1_3),
        Some(PdfX1a_2001)
    );
    assert_eq!(PreflightProfile::for_conformance(&C::X4_2010_PDF_1_4), None);
    assert_eq!(PreflightProfile::for_conformance(&C::default()), None);
}

// ---------------------------------------------------------------------------------------------
// The XML check of the XMP packet

#[test]
fn xml_well_formedness() {
    use super::xmp::check_well_formed as ok;
    let sample: &[u8] =
        b"<a><b x=\"1\" y='2'><c/><!-- c --><?pi x?><![CDATA[<>&]]>t&amp;&#65;&#x41;</b></a>";
    assert_eq!(ok(sample), Ok(()));
    assert!(ok(b"<a>").is_err());
    assert!(ok(b"<a></b>").is_err());
    assert!(ok(b"<a/><b/>").is_err());
    assert!(ok(b"text").is_err());
    assert!(ok(b"<a>&nbsp;</a>").is_err());
    assert!(ok(b"<a x=1/>").is_err());
    assert!(ok(b"<a x=\"<\"/>").is_err());
    assert!(ok(b"<a> 1 < 2 </a>").is_err());
    assert!(ok(b"").is_err());
    let deep = "<a>".repeat(300);
    assert!(ok(deep.as_bytes()).is_err());
}
