//! Hostile files against the preflight (`preflight_bytes`). Whatever the input, it returns a
//! report, within a bounded time and memory, and without a panic.
//!
//! There are no PDF files in the repository, so the inputs are made here: documents written by
//! printpdf (text, an embedded font, PDF/X and PDF/A conformance, links, bookmarks), a rich
//! document built with `lopdf`, and then truncated, bit-flipped and structurally mutated copies
//! of them, and files that are hostile by construction (cycles, deep nesting, decompression
//! bombs, many pages).
//!
//! A call that does not return cannot be cancelled, so a timeout ends the whole test process.

use std::io::Write as _;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream, StringFormat};
use printpdf::preflight::{PreflightProfile, PreflightReport, Severity};
use printpdf::{
    preflight_bytes, Actions, BuiltinFont, LinkAnnotation, Mm, Op, PdfConformance, PdfDocument,
    PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt, Rect, TextItem,
};

// A debug build on a CI runner is about ten times slower than a release build.
const TIMEOUT: Duration = Duration::from_secs(if cfg!(debug_assertions) { 60 } else { 5 });

/// The stack of a WebAssembly module is 1 MiB. The check has to fit in it.
const STACK_SIZE: usize = 1 << 20;

const PROFILES: [PreflightProfile; 5] = PreflightProfile::ALL;

/// Runs `f` on a thread with the stack of a WebAssembly module and waits at most [`TIMEOUT`].
fn bounded<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    let handle = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || {
            let _ = tx.send(f());
        })
        .expect("spawn");
    match rx.recv_timeout(TIMEOUT) {
        Ok(value) => {
            let _ = handle.join();
            value
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // The output of `eprintln!` is captured by the test harness and would be lost.
            let _ = writeln!(
                std::io::stderr(),
                "\nHANG: {what} did not return within {TIMEOUT:?}; ending the test process"
            );
            std::process::exit(101);
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => match handle.join() {
            Err(payload) => std::panic::resume_unwind(payload),
            Ok(()) => panic!("{what}: the thread ended without a result"),
        },
    }
}

/// Checks `bytes` with every profile and asserts that the reports are consistent.
fn check_all(what: &str, bytes: Vec<u8>) -> Vec<PreflightReport> {
    let name = what.to_string();
    bounded(what, move || {
        PROFILES
            .iter()
            .map(|&p| {
                let report = preflight_bytes(&bytes, p);
                let count =
                    |s: Severity| report.findings.iter().filter(|f| f.severity == s).count();
                assert_eq!(report.summary.errors, count(Severity::Error), "{name}");
                assert_eq!(report.summary.warnings, count(Severity::Warning), "{name}");
                assert_eq!(report.summary.infos, count(Severity::Info), "{name}");
                assert!(!report.not_checked.is_empty());
                report
            })
            .collect()
    })
}

// ---------------------------------------------------------------------------------------------
// Documents written by printpdf

fn text_ops(font: PdfFontHandle, text: &str) -> Vec<Op> {
    vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(10.0), Mm(280.0)),
        },
        Op::SetFont {
            font,
            size: Pt(14.0),
        },
        Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        },
        Op::EndTextSection,
    ]
}

fn printpdf_documents() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let opts = PdfSaveOptions::default();

    let conformances = [
        PdfConformance::default(),
        PdfConformance::X1A_2001_PDF_1_3,
        PdfConformance::X3_2002_PDF_1_3,
        PdfConformance::A1B_2005_PDF_1_4,
        PdfConformance::A2B_2011_PDF_1_7,
    ];
    let font_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/assets/fonts/RobotoMedium.ttf"
    ));

    for conformance in conformances {
        let label = conformance.get_identifier_string();

        let mut doc = PdfDocument::new("Builtin font");
        doc.metadata.info.conformance = conformance.clone();
        let mut ops = text_ops(
            PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            "Hello & <world>",
        );
        let link = LinkAnnotation::new(
            Rect::from_xywh(Pt(50.0), Pt(50.0), Pt(100.0), Pt(20.0)),
            Actions::uri("https://example.com".to_string()),
            None,
            None,
            None,
        );
        ops.push(Op::LinkAnnotation { link });
        doc.pages
            .push(PdfPage::new(Mm(210.0), Mm(297.0), ops.clone()));
        doc.pages.push(PdfPage::new(Mm(100.0), Mm(100.0), ops));
        doc.add_bookmark("Start", 1);
        out.push((
            format!("builtin font, {label}"),
            doc.save(&opts, &mut Vec::new()),
        ));

        let mut doc = PdfDocument::new("Embedded font & more");
        doc.metadata.info.conformance = conformance;
        let font = printpdf::ParsedFont::from_bytes(font_bytes, 0, &mut Vec::new()).expect("font");
        let id = doc.add_font(&font);
        doc.pages.push(PdfPage::new(
            Mm(210.0),
            Mm(297.0),
            text_ops(PdfFontHandle::External(id), "Grüße"),
        ));
        out.push((
            format!("embedded font, {label}"),
            doc.save(&opts, &mut Vec::new()),
        ));
    }
    out
}

#[test]
fn documents_written_by_printpdf() {
    for (what, bytes) in printpdf_documents() {
        let reports = check_all(&what, bytes);
        // The General profile finds nothing that is an error in a file that printpdf wrote.
        let general = reports.first().expect("a report");
        assert!(!general.has_errors(), "{what}: {:#?}", general.findings);
    }
}

// ---------------------------------------------------------------------------------------------
// A rich document built with lopdf

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

fn string(s: &[u8]) -> Object {
    Object::String(s.to_vec(), StringFormat::Literal)
}

fn reference(id: ObjectId) -> Object {
    Object::Reference(id)
}

fn array(v: Vec<Object>) -> Object {
    Object::Array(v)
}

fn numbers(v: &[f32]) -> Object {
    Object::Array(v.iter().map(|x| Object::Real(*x)).collect())
}

fn icc(n: i64) -> Vec<u8> {
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

/// A document that has most of the things the preflight looks at.
fn rich_document() -> Document {
    let mut doc = Document::with_version("1.4");
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    let pages = doc.new_object_id();

    let profile = doc.add_object(Stream::new(dictionary! { "N" => 4 }, icc(4)));
    let rgb_profile = doc.add_object(Stream::new(dictionary! { "N" => 3 }, icc(3)));
    let intent = doc.add_object(dictionary! {
        "Type" => "OutputIntent", "S" => "GTS_PDFX", "OutputConditionIdentifier" => string(b"FOGRA39"),
        "DestOutputProfile" => reference(profile),
    });
    let tint = doc.add_object(
        dictionary! { "FunctionType" => 2, "Domain" => numbers(&[0.0, 1.0]), "N" => 1 },
    );
    let font_file = doc.add_object(Stream::new(Dictionary::new(), vec![0, 1, 0, 0, 0, 0, 0, 0]));
    let descriptor = doc.add_object(dictionary! {
        "Type" => "FontDescriptor", "FontName" => "AAAAAA+Test", "Flags" => 32,
        "FontFile2" => reference(font_file),
    });
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "AAAAAA+Test",
        "FontDescriptor" => reference(descriptor),
    });
    let helvetica = doc.add_object(
        dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
    );
    let image = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => 2, "Height" => 1,
            "BitsPerComponent" => 8,
            "ColorSpace" => array(vec![name("ICCBased"), reference(rgb_profile)]),
        },
        vec![1, 2, 3, 4, 5, 6],
    ));
    let form = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form", "BBox" => numbers(&[0.0, 0.0, 50.0, 50.0]),
            "Resources" => Object::Dictionary(dictionary! {
                "Font" => Object::Dictionary(dictionary! { "F2" => reference(helvetica) }),
            }),
        },
        b"BT /F2 10 Tf (form) Tj ET 0 0 0 1 k 0 0 5 5 re f".to_vec(),
    ));
    let contents = doc.add_object(Stream::new(
        Dictionary::new(),
        b"q 1 0 0 1 10 10 cm /Fm1 Do Q BT /F1 12 Tf 10 50 Td (Hello) Tj ET \
          /Im1 Do /Cs1 cs 0.5 sc 0 0 10 10 re f /GS1 gs 1 0 0 rg 0 0 5 5 re S"
            .to_vec(),
    ));
    let action =
        doc.add_object(dictionary! { "S" => "URI", "URI" => string(b"https://example.com") });
    let annot = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Link", "Rect" => numbers(&[0.0, 0.0, 10.0, 10.0]),
        "A" => reference(action),
    });
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => reference(pages),
        "MediaBox" => numbers(&[0.0, 0.0, 200.0, 100.0]),
        "TrimBox" => numbers(&[5.0, 5.0, 195.0, 95.0]),
        "Contents" => reference(contents),
        "Annots" => array(vec![reference(annot)]),
        "Resources" => Object::Dictionary(dictionary! {
            "Font" => Object::Dictionary(dictionary! { "F1" => reference(font) }),
            "XObject" => Object::Dictionary(dictionary! { "Fm1" => reference(form), "Im1" => reference(image) }),
            "ExtGState" => Object::Dictionary(dictionary! {
                "GS1" => Object::Dictionary(dictionary! { "ca" => 0.5, "BM" => "Multiply" }),
            }),
            "ColorSpace" => Object::Dictionary(dictionary! {
                "Cs1" => array(vec![name("Separation"), name("Spot"), name("DeviceCMYK"), reference(tint)]),
                "Cs2" => array(vec![name("Indexed"), name("DeviceRGB"), Object::Integer(1), string(b"abcdef")]),
            }),
        }),
    });
    doc.objects.insert(
        pages,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Count" => 1, "Kids" => array(vec![reference(page)]),
        }),
    );
    let xmp = doc.add_object(Stream::new(
        dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
        b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"r\"><rdf:Description \
          xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\" pdfaid:part=\"1\" pdfaid:conformance=\"B\"/>\
          </rdf:RDF></x:xmpmeta>"
            .to_vec(),
    ));
    let catalog = doc.add_object(dictionary! {
        "Type" => "Catalog", "Pages" => reference(pages), "OutputIntents" => array(vec![reference(intent)]),
        "Metadata" => reference(xmp),
    });
    let info = doc.add_object(dictionary! {
        "Title" => string(b"T"), "CreationDate" => string(b"D:20240101120000Z"),
        "ModDate" => string(b"D:20240102120000Z"), "Trapped" => "False",
        "GTS_PDFXVersion" => string(b"PDF/X-3:2002"),
    });
    doc.trailer.set("Root", reference(catalog));
    doc.trailer.set("Info", reference(info));
    doc.trailer.set(
        "ID",
        array(vec![
            Object::String(vec![1; 16], StringFormat::Hexadecimal),
            Object::String(vec![2; 16], StringFormat::Hexadecimal),
        ]),
    );
    doc
}

fn document_bytes(doc: &mut Document) -> Vec<u8> {
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("save");
    out
}

fn modern_bytes(doc: &mut Document) -> Vec<u8> {
    let mut out = Vec::new();
    doc.save_modern(&mut out).expect("save");
    out
}

#[test]
fn the_rich_document_has_the_findings_that_it_was_built_with() {
    let reports = check_all("rich document", document_bytes(&mut rich_document()));
    let find = |p: PreflightProfile, rule: &str| {
        reports[PROFILES.iter().position(|&q| q == p).expect("profile")]
            .findings
            .iter()
            .any(|f| f.rule == rule)
    };
    // The transparency in the page is an error for PDF/X and fine for PDF/A-2.
    assert!(find(PreflightProfile::PdfX3_2002, "transparency-alpha"));
    assert!(find(
        PreflightProfile::PdfX3_2002,
        "transparency-blend-mode"
    ));
    assert!(!find(PreflightProfile::PdfA2b, "transparency-alpha"));
    // Helvetica in the form is not embedded.
    assert!(find(PreflightProfile::PdfX3_2002, "font-not-embedded"));
    // The page draws in RGB with an ICC image, which PDF/X-1a does not allow.
    assert!(find(
        PreflightProfile::PdfX1a_2001,
        "colour-space-not-allowed"
    ));
    assert!(find(PreflightProfile::PdfX1a_2001, "rgb-colour-in-pdfx1a"));
    assert!(!find(PreflightProfile::General, "colour-space-malformed"));
}

#[test]
fn truncated_at_every_step() {
    let bytes = document_bytes(&mut rich_document());
    let step = (bytes.len() / 60).max(1);
    for cut in (0..bytes.len()).step_by(step) {
        check_all(&format!("truncated at {cut}"), bytes[..cut].to_vec());
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn seeds() -> Vec<(String, Vec<u8>)> {
    let mut seeds = printpdf_documents();
    seeds.push(("rich".to_string(), document_bytes(&mut rich_document())));
    seeds.push((
        "rich, modern".to_string(),
        modern_bytes(&mut rich_document()),
    ));
    seeds
}

#[test]
fn bit_flips_and_deleted_chunks() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for (what, bytes) in seeds() {
        for i in 0..40 {
            let mut m = bytes.clone();
            match rng.below(4) {
                0 | 1 => {
                    for _ in 0..=rng.below(8) {
                        let at = rng.below(m.len());
                        m[at] ^= 1 << rng.below(8);
                    }
                }
                2 => {
                    let start = rng.below(m.len());
                    let len = rng.below(200).min(m.len() - start);
                    m.drain(start..start + len);
                }
                _ => {
                    let start = rng.below(m.len());
                    let len = rng.below(300).min(m.len() - start);
                    let chunk: Vec<u8> = m[start..start + len].to_vec();
                    let at = rng.below(m.len());
                    m.splice(at..at, chunk);
                }
            }
            check_all(&format!("{what}, mutation {i}"), m);
        }
    }
}

fn dict_mut(doc: &mut Document, id: ObjectId) -> &mut Dictionary {
    match doc.objects.get_mut(&id) {
        Some(Object::Dictionary(d)) => d,
        Some(Object::Stream(s)) => &mut s.dict,
        _ => panic!("object {id:?} has no dictionary"),
    }
}

fn hostile_value(rng: &mut Rng, ids: &[ObjectId]) -> Object {
    match rng.below(14) {
        0 => Object::Null,
        1 => Object::Boolean(true),
        2 => {
            let numbers: [i64; 8] = [0, -1, 1, 255, 1 << 31, 1 << 62, i64::MAX, i64::MIN];
            Object::Integer(numbers[rng.below(numbers.len())])
        }
        3 => Object::Real([0.0f32, -1.5, 1e30, 1e-30, 0.5][rng.below(5)]),
        4 => name(
            [
                "Foo",
                "",
                "Pages",
                "Page",
                "Identity-H",
                "DeviceRGB",
                "FlateDecode",
                "Image",
                "Form",
                "Named",
                "JavaScript",
                "Launch",
                "Transparency",
                "Indexed",
                "ICCBased",
            ][rng.below(15)],
        ),
        5 => string(
            &[
                b"".to_vec(),
                vec![0xFE, 0xFF, 0x00],
                b"D:2024".to_vec(),
                vec![b'x'; 5_000],
            ][rng.below(4)],
        ),
        6 => array(vec![]),
        7 => array(vec![
            Object::Integer(1),
            Object::Integer(2),
            Object::Integer(3),
        ]),
        8 => array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(10),
            Object::Integer(10),
        ]),
        9 => array(vec![reference(ids[rng.below(ids.len())])]),
        10 => array((1..=6).map(Object::Integer).collect()),
        11 | 12 => reference(ids[rng.below(ids.len())]),
        _ => reference((9_999, 0)),
    }
}

/// Replaces, removes or redirects some entries of a valid document.
#[test]
fn mutated_structure() {
    let base = rich_document();
    let ids: Vec<ObjectId> = base.objects.keys().copied().collect();
    let mut entries: Vec<(ObjectId, Vec<u8>)> = Vec::new();
    for (id, object) in &base.objects {
        let dict = match object {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => continue,
        };
        entries.extend(dict.iter().map(|(key, _)| (*id, key.clone())));
    }
    assert!(entries.len() > 50, "{} entries", entries.len());

    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for iteration in 0..400 {
        let mut doc = base.clone();
        for _ in 0..=rng.below(3) {
            let (id, key) = &entries[rng.below(entries.len())];
            let dict = dict_mut(&mut doc, *id);
            if rng.below(10) == 0 {
                dict.remove(key);
            } else {
                dict.set(key.clone(), hostile_value(&mut rng, &ids));
            }
        }
        check_all(
            &format!("structure mutation {iteration}"),
            document_bytes(&mut doc),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Files that are hostile by construction

/// A document with a catalog, a page tree node and one page, to be changed.
fn skeleton() -> (Document, ObjectId, ObjectId, ObjectId) {
    let mut doc = Document::with_version("1.4");
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    let pages = doc.new_object_id();
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => reference(pages), "MediaBox" => numbers(&[0.0, 0.0, 200.0, 100.0]),
    });
    doc.objects.insert(
        pages,
        Object::Dictionary(
            dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => array(vec![reference(page)]) },
        ),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => reference(pages) });
    doc.trailer.set("Root", reference(catalog));
    (doc, catalog, pages, page)
}

fn zlib_zeros(len: usize) -> Vec<u8> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let block = vec![b' '; 1 << 20];
    let mut left = len;
    while left > 0 {
        let n = left.min(block.len());
        e.write_all(&block[..n]).expect("write");
        left -= n;
    }
    e.finish().expect("finish")
}

#[test]
fn cycles_and_deep_nesting() {
    // The page tree contains itself, through a node and through the root.
    let (mut doc, _, pages, page) = skeleton();
    let inner = doc.add_object(dictionary! {
        "Type" => "Pages", "Parent" => reference(pages), "Count" => 1,
        "Kids" => array(vec![reference(pages), reference(page)]),
    });
    dict_mut(&mut doc, pages).set("Kids", array(vec![reference(inner), reference(inner)]));
    check_all("page tree cycle", document_bytes(&mut doc));

    // A form whose resources contain itself.
    let (mut doc, _, _, page) = skeleton();
    let form = doc.new_object_id();
    doc.objects.insert(
        form,
        Object::Stream(Stream::new(
            dictionary! {
                "Subtype" => "Form", "BBox" => numbers(&[0.0, 0.0, 1.0, 1.0]),
                "Resources" => Object::Dictionary(dictionary! {
                    "XObject" => Object::Dictionary(dictionary! { "Fm1" => reference(form) }),
                }),
            },
            b"/Fm1 Do".to_vec(),
        )),
    );
    let contents = doc.add_object(Stream::new(Dictionary::new(), b"/Fm1 Do".to_vec()));
    dict_mut(&mut doc, page).set("Contents", reference(contents));
    dict_mut(&mut doc, page).set(
        "Resources",
        Object::Dictionary(dictionary! {
            "XObject" => Object::Dictionary(dictionary! { "Fm1" => reference(form) }),
        }),
    );
    check_all("form that draws itself", document_bytes(&mut doc));

    // Forms that nest 200 deep, each in its own object.
    let (mut doc, _, _, page) = skeleton();
    let mut inner: Option<ObjectId> = None;
    for _ in 0..200 {
        let resources = match inner {
            Some(i) => {
                dictionary! { "XObject" => Object::Dictionary(dictionary! { "Fm" => reference(i) }) }
            }
            None => Dictionary::new(),
        };
        inner = Some(doc.add_object(Stream::new(
            dictionary! {
                "Subtype" => "Form", "BBox" => numbers(&[0.0, 0.0, 1.0, 1.0]),
                "Resources" => Object::Dictionary(resources),
            },
            b"/Fm Do".to_vec(),
        )));
    }
    let contents = doc.add_object(Stream::new(Dictionary::new(), b"/Fm Do".to_vec()));
    dict_mut(&mut doc, page).set("Contents", reference(contents));
    dict_mut(&mut doc, page).set(
        "Resources",
        Object::Dictionary(dictionary! {
            "XObject" => Object::Dictionary(dictionary! { "Fm" => reference(inner.expect("form")) }),
        }),
    );
    let reports = check_all("forms nested 200 deep", document_bytes(&mut doc));
    assert!(reports[0]
        .findings
        .iter()
        .any(|f| f.rule == "check-limit-reached"));

    // Colour spaces that nest 200 deep.
    let (mut doc, _, _, page) = skeleton();
    let mut cs = name("DeviceGray");
    for _ in 0..200 {
        cs = array(vec![name("Indexed"), cs, Object::Integer(0), string(b"\0")]);
    }
    dict_mut(&mut doc, page).set(
        "Resources",
        Object::Dictionary(
            dictionary! { "ColorSpace" => Object::Dictionary(dictionary! { "Cs1" => cs }) },
        ),
    );
    check_all("colour spaces nested 200 deep", document_bytes(&mut doc));

    // Colour spaces that contain themselves.
    let (mut doc, _, _, page) = skeleton();
    let cs = doc.new_object_id();
    doc.objects.insert(
        cs,
        array(vec![
            name("Indexed"),
            reference(cs),
            Object::Integer(0),
            string(b"\0"),
        ]),
    );
    dict_mut(&mut doc, page).set(
        "Resources",
        Object::Dictionary(dictionary! { "ColorSpace" => Object::Dictionary(dictionary! { "Cs1" => reference(cs) }) }),
    );
    check_all(
        "colour space that contains itself",
        document_bytes(&mut doc),
    );

    // References to references to references, and to themselves.
    let (mut doc, catalog, _, _) = skeleton();
    let a = doc.new_object_id();
    let b = doc.new_object_id();
    doc.objects.insert(a, reference(b));
    doc.objects.insert(b, reference(a));
    dict_mut(&mut doc, catalog).set("Metadata", reference(a));
    dict_mut(&mut doc, catalog).set("Names", reference(a));
    check_all("reference cycle", document_bytes(&mut doc));
}

#[test]
fn decompression_bombs_are_bounded() {
    // One page whose content stream inflates to 300 MiB.
    let (mut doc, _, _, page) = skeleton();
    let bomb = zlib_zeros(300 * 1024 * 1024);
    let contents = doc.add_object(Stream::new(dictionary! { "Filter" => "FlateDecode" }, bomb));
    dict_mut(&mut doc, page).set("Contents", reference(contents));
    let reports = check_all("content bomb", document_bytes(&mut doc));
    assert!(reports.iter().all(|r| r
        .summary
        .limits_reached
        .iter()
        .any(|l| l.contains("decoded stream bytes"))));

    // Many pages that share one big content stream and one bomb as an image.
    let (mut doc, _, pages, _) = skeleton();
    let contents = doc.add_object(Stream::new(
        dictionary! { "Filter" => "FlateDecode" },
        zlib_zeros(30 * 1024 * 1024),
    ));
    let bomb = doc.add_object(Stream::new(
        dictionary! {
            "Subtype" => "Image", "Width" => 1, "Height" => 1, "BitsPerComponent" => 8,
            "ColorSpace" => "DeviceGray", "Filter" => "FlateDecode",
        },
        zlib_zeros(100 * 1024 * 1024),
    ));
    let mut kids = Vec::new();
    for _ in 0..40 {
        let p = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => reference(pages), "MediaBox" => numbers(&[0.0, 0.0, 10.0, 10.0]),
            "Contents" => reference(contents),
            "Resources" => Object::Dictionary(dictionary! {
                "XObject" => Object::Dictionary(dictionary! { "Im1" => reference(bomb) }),
            }),
        });
        kids.push(reference(p));
    }
    dict_mut(&mut doc, pages).set("Kids", array(kids));
    dict_mut(&mut doc, pages).set("Count", 40);
    check_all("shared bombs on 40 pages", document_bytes(&mut doc));
}

#[test]
fn many_pages_and_many_operators() {
    // 20,000 pages.
    let (mut doc, _, pages, _) = skeleton();
    let mut kids = Vec::new();
    for _ in 0..20_000 {
        let p = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => reference(pages), "MediaBox" => numbers(&[0.0, 0.0, 10.0, 10.0]),
        });
        kids.push(reference(p));
    }
    dict_mut(&mut doc, pages).set("Kids", array(kids));
    dict_mut(&mut doc, pages).set("Count", 20_000);
    let reports = check_all("20,000 pages", document_bytes(&mut doc));
    assert_eq!(reports[0].summary.pages_checked, 20_000);

    // One page listed 50,000 times.
    let (mut doc, _, pages, page) = skeleton();
    dict_mut(&mut doc, pages).set("Kids", array(vec![reference(page); 50_000]));
    check_all("one page listed 50,000 times", document_bytes(&mut doc));

    // A content stream of three million operators, and one that never closes q.
    let (mut doc, _, _, page) = skeleton();
    let content = "q 1 0 0 1 0 0 cm ".repeat(150_000);
    let contents = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
    dict_mut(&mut doc, page).set("Contents", reference(contents));
    check_all("300,000 operators", document_bytes(&mut doc));

    // Deeply nested arrays in a content stream and in a dictionary.
    let (mut doc, _, _, page) = skeleton();
    let content = format!("{} 1 Tj", "[".repeat(100_000));
    let contents = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
    dict_mut(&mut doc, page).set("Contents", reference(contents));
    check_all("deeply nested arrays", document_bytes(&mut doc));
}

#[test]
fn xmp_packets_that_are_hostile() {
    for (what, xmp) in [
        ("deep", "<a>".repeat(100_000)),
        (
            "many attributes",
            format!("<a {}/>", "b=\"1\" ".repeat(100_000)),
        ),
        (
            "many entities",
            format!("<a>{}</a>", "&amp;".repeat(500_000)),
        ),
        ("unclosed comment", "<a><!-- ".to_string()),
        (
            "binary",
            String::from_utf8_lossy(&[0xFFu8; 4000]).into_owned(),
        ),
    ] {
        let (mut doc, catalog, _, _) = skeleton();
        let s = doc.add_object(Stream::new(
            dictionary! { "Subtype" => "XML" },
            xmp.into_bytes(),
        ));
        dict_mut(&mut doc, catalog).set("Metadata", reference(s));
        check_all(&format!("xmp: {what}"), document_bytes(&mut doc));
    }
}

#[test]
fn a_report_is_the_same_each_time() {
    let bytes = document_bytes(&mut rich_document());
    let first = bounded("first", {
        let bytes = bytes.clone();
        move || preflight_bytes(&bytes, PreflightProfile::PdfX3_2002)
    });
    let second = bounded("second", move || {
        preflight_bytes(&bytes, PreflightProfile::PdfX3_2002)
    });
    assert_eq!(first, second);
}

#[test]
fn time_is_linear_in_the_size() {
    // A smoke test of the bound: a file of many small objects is checked in a fraction of the
    // time limit.
    let (mut doc, catalog, _, _) = skeleton();
    for i in 0..200_000 {
        doc.add_object(Object::Integer(i));
    }
    dict_mut(&mut doc, catalog).set("Extra", Object::Integer(1));
    let bytes = document_bytes(&mut doc);
    let start = Instant::now();
    check_all("200,000 small objects", bytes);
    assert!(start.elapsed() < TIMEOUT);
}
