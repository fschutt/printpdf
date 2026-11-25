//! Hostile patterns. Whatever the input, parsing, saving and rendering a document with patterns
//! returns, within a bounded time and memory, and without a panic. A pattern that cannot be read
//! becomes a warning or is kept as it is.
//!
//! Documents are built with `lopdf` so every case is precise. Patterns that are built in code
//! (a `NaN` step, a huge matrix, nesting 40 levels deep) go through the writer.
//!
//! A call that does not return cannot be cancelled, so a timeout ends the whole test process
//! (see `bounded`, the same as in `no_panic_structure.rs`).

use std::io::Write as _;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use printpdf::*;

// A debug build on a CI runner is about ten times slower than a release build.
const TIMEOUT: Duration = Duration::from_secs(if cfg!(debug_assertions) { 60 } else { 5 });

/// The stack of a WebAssembly module is 1 MiB. Parsing has to fit in it.
const STACK_SIZE: usize = 1 << 20;

/// Runs `f` on a thread with the stack of a WebAssembly module and waits at most [`TIMEOUT`]. A
/// panic of `f` is raised again here. A timeout ends the process.
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

// ---------------------------------------------------------------------------------------------
// Building documents

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

fn numbers(values: &[f32]) -> Object {
    Object::Array(values.iter().map(|v| Object::Real(*v)).collect())
}

fn reference(id: ObjectId) -> Object {
    Object::Reference(id)
}

/// A one-page document (200 x 200) with this content stream and these page resources.
fn document(
    content: &[u8],
    build: impl FnOnce(&mut Document) -> Dictionary,
) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages = doc.new_object_id();
    let resources = build(&mut doc);
    let contents = doc.add_object(Stream::new(dictionary! {}, content.to_vec()));
    let page = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => reference(pages),
        "MediaBox" => numbers(&[0.0, 0.0, 200.0, 200.0]),
        "Resources" => resources,
        "Contents" => reference(contents),
    });
    doc.objects.insert(
        pages,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Count" => 1, "Kids" => vec![reference(page)],
        }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => reference(pages) });
    doc.trailer.set("Root", reference(catalog));
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("save");
    out
}

/// A tiling pattern stream with the usual entries; `extra` overrides or adds entries.
fn tiling(doc: &mut Document, extra: Dictionary, content: &[u8]) -> ObjectId {
    let mut dict = dictionary! {
        "Type" => "Pattern", "PatternType" => 1, "PaintType" => 1, "TilingType" => 1,
        "BBox" => numbers(&[0.0, 0.0, 10.0, 10.0]),
        "XStep" => 10, "YStep" => 10,
        "Resources" => dictionary! {},
    };
    for (key, value) in extra.iter() {
        dict.set(key.clone(), value.clone());
    }
    doc.add_object(Stream::new(dict, content.to_vec()))
}

fn one_pattern(content: &[u8], extra: Dictionary) -> Vec<u8> {
    document(content, |doc| {
        let p1 = tiling(doc, extra, b"1 0 0 rg 0 0 5 5 re f");
        dictionary! { "Pattern" => dictionary! { "P1" => reference(p1) } }
    })
}

/// Everything a consumer does with a document: parse, read the model, write it again, parse
/// that, and draw the SVG, extract the text. Returns what the first parse made of the patterns
/// and the warnings.
fn all_the_ways(what: &str, bytes: Vec<u8>) -> (usize, Vec<PdfWarnMsg>) {
    bounded(what, move || {
        let mut warnings = Vec::new();
        let Ok(doc) = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut warnings) else {
            return (0, warnings);
        };
        let patterns = doc.resources.patterns.map.len();
        let _ = doc.extract_text();
        for page in &doc.pages {
            let _ = page.to_svg(&doc.resources, &PdfToSvgOptions::default(), &mut warnings);
        }
        let saved = doc.save(&PdfSaveOptions::default(), &mut warnings);
        let _ = PdfDocument::parse(&saved, &PdfParseOptions::default(), &mut warnings);
        let json = serde_json::to_string(&doc.resources).expect("json");
        // A pattern kept as it is can nest deeper than the JSON reader allows, so this may be
        // an error; it is not a panic.
        let _ = serde_json::from_str::<PdfResources>(&json);
        (patterns, warnings)
    })
}

// ---------------------------------------------------------------------------------------------
// Documents from files

#[test]
fn a_pattern_that_names_itself() {
    // P1 has P1 in its own resources.
    let bytes = document(b"/Pattern cs /P1 scn 0 0 100 100 re f", |doc| {
        let p1 = doc.new_object_id();
        let stream = Stream::new(
            dictionary! {
                "Type" => "Pattern", "PatternType" => 1, "PaintType" => 1, "TilingType" => 1,
                "BBox" => numbers(&[0.0, 0.0, 10.0, 10.0]), "XStep" => 10, "YStep" => 10,
                "Resources" => dictionary! { "Pattern" => dictionary! { "P1" => reference(p1) } },
            },
            b"/Pattern cs /P1 scn 0 0 10 10 re f".to_vec(),
        );
        doc.objects.insert(p1, Object::Stream(stream));
        dictionary! { "Pattern" => dictionary! { "P1" => reference(p1) } }
    });
    let (patterns, warnings) = all_the_ways("self-referencing pattern", bytes);
    assert_eq!(patterns, 1, "{warnings:?}");
}

#[test]
fn two_patterns_that_name_each_other() {
    let bytes = document(b"/Pattern cs /A scn 0 0 100 100 re f", |doc| {
        let a = doc.new_object_id();
        let b = doc.new_object_id();
        for (this, other, other_name) in [(a, b, "B"), (b, a, "A")] {
            let stream = Stream::new(
                dictionary! {
                    "Type" => "Pattern", "PatternType" => 1, "PaintType" => 1, "TilingType" => 1,
                    "BBox" => numbers(&[0.0, 0.0, 10.0, 10.0]), "XStep" => 10, "YStep" => 10,
                    "Resources" => dictionary! {
                        "Pattern" => dictionary! { other_name => reference(other) }
                    },
                },
                b"0 0 10 10 re f".to_vec(),
            );
            doc.objects.insert(this, Object::Stream(stream));
        }
        dictionary! { "Pattern" => dictionary! { "A" => reference(a), "B" => reference(b) } }
    });
    let (patterns, warnings) = all_the_ways("patterns that name each other", bytes);
    assert_eq!(patterns, 2, "{warnings:?}");
}

#[test]
fn a_chain_of_nested_patterns() {
    // Every pattern has the next one in its resources: 2000 levels. The nesting is kept as it
    // is, down to a fixed depth.
    let bytes = document(b"/Pattern cs /P0 scn 0 0 100 100 re f", |doc| {
        let ids: Vec<ObjectId> = (0..2000).map(|_| doc.new_object_id()).collect();
        for (i, id) in ids.iter().enumerate() {
            let mut resources = Dictionary::new();
            if let Some(next) = ids.get(i + 1) {
                resources.set("Pattern", dictionary! { "Next" => reference(*next) });
            }
            let stream = Stream::new(
                dictionary! {
                    "Type" => "Pattern", "PatternType" => 1, "PaintType" => 1, "TilingType" => 1,
                    "BBox" => numbers(&[0.0, 0.0, 10.0, 10.0]), "XStep" => 10, "YStep" => 10,
                    "Resources" => resources,
                },
                b"0 0 10 10 re f".to_vec(),
            );
            doc.objects.insert(*id, Object::Stream(stream));
        }
        dictionary! { "Pattern" => dictionary! { "P0" => reference(ids[0]) } }
    });
    let (patterns, _) = all_the_ways("2000 nested patterns", bytes);
    assert_eq!(patterns, 1);
}

#[test]
fn a_pattern_object_under_many_names() {
    // 50 000 names for one pattern object: the object is read once and copied.
    let bytes = document(b"/Pattern cs /N0 scn 0 0 100 100 re f", |doc| {
        let p = tiling(doc, Dictionary::new(), b"0 0 10 10 re f");
        let mut names = Dictionary::new();
        for i in 0..50_000 {
            names.set(format!("N{i}"), reference(p));
        }
        dictionary! { "Pattern" => names }
    });
    let (patterns, warnings) = all_the_ways("50000 names for one pattern", bytes);
    assert_eq!(patterns, 50_000, "{warnings:?}");
}

#[test]
fn steps_and_matrices_with_extreme_numbers() {
    // A step of 1e9 is a valid step: the pattern is modelled.
    let bytes = one_pattern(
        b"/Pattern cs /P1 scn 0 0 100 100 re f",
        dictionary! { "XStep" => 1_000_000_000, "YStep" => 1_000_000_000 },
    );
    let (patterns, warnings) = all_the_ways("xstep 1e9", bytes);
    assert_eq!(patterns, 1, "{warnings:?}");

    // Zero and negative steps, and a step that is not a number.
    for (what, extra) in [
        ("zero xstep", dictionary! { "XStep" => 0 }),
        ("zero ystep", dictionary! { "YStep" => 0.0 }),
        ("negative xstep", dictionary! { "XStep" => -10 }),
        ("name as xstep", dictionary! { "XStep" => name("Wide") }),
        ("missing bbox", dictionary! { "BBox" => name("None") }),
        ("short bbox", dictionary! { "BBox" => numbers(&[1.0, 2.0]) }),
        ("huge bbox", dictionary! { "BBox" => numbers(&[-1e30, -1e30, 1e30, 1e30]) }),
        ("huge matrix", dictionary! { "Matrix" => numbers(&[1e30, 1e30, -1e30, 1e30, 1e38, -1e38]) }),
        ("tiny matrix", dictionary! { "Matrix" => numbers(&[1e-30, 0.0, 0.0, 1e-30, 0.0, 0.0]) }),
        ("singular matrix", dictionary! { "Matrix" => numbers(&[0.0; 6]) }),
        ("short matrix", dictionary! { "Matrix" => numbers(&[1.0, 0.0]) }),
        ("matrix of names", dictionary! { "Matrix" => vec![name("a"); 6] }),
        ("paint type 7", dictionary! { "PaintType" => 7 }),
        ("pattern type 3", dictionary! { "PatternType" => 3 }),
        ("tiling type 9", dictionary! { "TilingType" => 9 }),
        ("resources not a dictionary", dictionary! { "Resources" => 5 }),
        ("unknown entry", dictionary! { "Mystery" => 1 }),
    ] {
        let bytes = one_pattern(b"/Pattern cs /P1 scn 0 0 100 100 re f", extra);
        let (_, warnings) = all_the_ways(what, bytes);
        // The result is not the point; it returned. The warnings are not errors of the test.
        let _ = warnings;
    }
}

#[test]
fn missing_and_broken_resources() {
    // No /Resources at all, and content that names a font, a graphics state and a shading it
    // does not have.
    let content = b"BT /F1 12 Tf (x) Tj ET /G1 gs /S1 sh 0 0 10 10 re f";
    let bytes = document(b"/Pattern cs /P1 scn 0 0 100 100 re f", |doc| {
        let mut dict = dictionary! {
            "Type" => "Pattern", "PatternType" => 1, "PaintType" => 1, "TilingType" => 1,
            "BBox" => numbers(&[0.0, 0.0, 10.0, 10.0]), "XStep" => 10, "YStep" => 10,
        };
        dict.remove(b"Resources");
        let p1 = doc.add_object(Stream::new(dict, content.to_vec()));
        dictionary! { "Pattern" => dictionary! { "P1" => reference(p1) } }
    });
    let (patterns, warnings) = all_the_ways("pattern without resources", bytes);
    assert_eq!(patterns, 1, "{warnings:?}");

    // Resources that point at objects that do not exist.
    let bytes = document(b"/Pattern cs /P1 scn 0 0 100 100 re f", |doc| {
        let p1 = tiling(
            doc,
            dictionary! {
                "Resources" => dictionary! {
                    "Font" => reference((9999, 0)),
                    "ExtGState" => dictionary! { "G1" => reference((9998, 0)) },
                    "Shading" => dictionary! { "S1" => reference((9997, 0)) },
                },
            },
            content,
        );
        dictionary! { "Pattern" => dictionary! { "P1" => reference(p1) } }
    });
    let (patterns, warnings) = all_the_ways("dangling resources", bytes);
    assert_eq!(patterns, 1, "{warnings:?}");

    // A /Pattern resource that is not a dictionary, and entries that are not patterns.
    for category in [
        Object::Integer(7),
        reference((9999, 0)),
        Object::Array(vec![]),
        Object::Dictionary(dictionary! {
            "A" => 1, "B" => name("x"), "C" => reference((9999, 0)), "D" => Object::Null,
            "E" => dictionary! {}, "F" => dictionary! { "PatternType" => 2 },
            "G" => dictionary! { "PatternType" => 2, "Shading" => 5 },
            "H" => dictionary! { "PatternType" => 2, "Shading" => reference((9999, 0)) },
        }),
    ] {
        let bytes = document(b"/Pattern cs /A scn 0 0 100 100 re f", |_| {
            dictionary! { "Pattern" => category }
        });
        all_the_ways("pattern category", bytes);
    }
}

#[test]
fn shading_patterns_with_odd_shadings() {
    let function = |extra: Dictionary| {
        let mut f = dictionary! {
            "FunctionType" => 2, "Domain" => numbers(&[0.0, 1.0]),
            "C0" => numbers(&[1.0, 0.0, 0.0]), "C1" => numbers(&[0.0, 0.0, 1.0]), "N" => 1,
        };
        for (key, value) in extra.iter() {
            f.set(key.clone(), value.clone());
        }
        f
    };
    let shading = |function: Dictionary, extra: Dictionary| {
        let mut s = dictionary! {
            "ShadingType" => 2, "ColorSpace" => "DeviceRGB",
            "Coords" => numbers(&[0.0, 0.0, 100.0, 0.0]),
            "Function" => function, "Extend" => vec![Object::Boolean(true), Object::Boolean(true)],
        };
        for (key, value) in extra.iter() {
            s.set(key.clone(), value.clone());
        }
        s
    };
    let cases = [
        ("plain", shading(function(Dictionary::new()), Dictionary::new())),
        ("exponent 2", shading(function(dictionary! { "N" => 2 }), Dictionary::new())),
        ("empty C0", shading(function(dictionary! { "C0" => numbers(&[]) }), Dictionary::new())),
        ("cmyk", shading(
            function(dictionary! { "C0" => numbers(&[0.0; 4]), "C1" => numbers(&[1.0; 4]) }),
            dictionary! { "ColorSpace" => "DeviceCMYK" },
        )),
        ("function type 4", shading(dictionary! { "FunctionType" => 4 }, Dictionary::new())),
        ("type 3 function without functions", shading(
            dictionary! { "FunctionType" => 3, "Domain" => numbers(&[0.0, 1.0]) },
            Dictionary::new(),
        )),
        ("short coords", shading(function(Dictionary::new()), dictionary! { "Coords" => numbers(&[1.0]) })),
        ("shading type 99", shading(function(Dictionary::new()), dictionary! { "ShadingType" => 99 })),
        ("coords of names", shading(
            function(Dictionary::new()),
            dictionary! { "Coords" => vec![name("a"); 4] },
        )),
        ("background", shading(function(Dictionary::new()), dictionary! { "Background" => numbers(&[0.0; 3]) })),
        ("huge coords", shading(function(Dictionary::new()), dictionary! { "Coords" => numbers(&[-1e30, 0.0, 1e30, 0.0]) })),
    ];
    for (what, shading) in cases {
        let bytes = document(b"/Pattern cs /P1 scn 0 0 100 100 re f", |doc| {
            let pattern = doc.add_object(dictionary! {
                "Type" => "Pattern", "PatternType" => 2, "Shading" => shading,
                "Matrix" => numbers(&[1e30, 0.0, 0.0, 1e30, 0.0, 0.0]),
                "ExtGState" => dictionary! { "CA" => 0.5, "Mystery" => 1 },
            });
            dictionary! { "Pattern" => dictionary! { "P1" => reference(pattern) } }
        });
        let (patterns, warnings) = all_the_ways(what, bytes);
        assert_eq!(patterns, 1, "{what}: {warnings:?}");
    }
}

#[test]
fn pattern_operators_with_odd_operands() {
    let contents: &[&[u8]] = &[
        b"/Pattern cs scn",
        b"/Pattern cs /scn",
        b"/Pattern cs 1 2 3 4 5 6 7 8 9 /P1 scn 0 0 5 5 re f",
        b"/Pattern cs 1 2 /P1 scn 0 0 5 5 re f",
        b"/Pattern cs /P1 /P2 scn 0 0 5 5 re f",
        b"/Pattern cs (str) /P1 scn 0 0 5 5 re f",
        b"/Pattern cs [1 2] /P1 scn 0 0 5 5 re f",
        b"/Pattern cs /Missing scn 0 0 5 5 re f",
        b"/Pattern cs /P1 SC 0 0 5 5 re S",
        b"/Cs1 cs 1 0 0 /P1 scn /Cs2 CS 0 /P1 SCN 0 0 5 5 re B",
        b"/Pattern cs cs cs /Pattern /Pattern cs",
        b"q q q /Pattern cs /P1 scn Q Q Q Q Q 0 0 5 5 re f",
        b"1 0 0 rg /Pattern cs 0 0 5 5 re f /P1 scn 0 0 5 5 re f 0 g 0 0 5 5 re f",
    ];
    for content in contents {
        let bytes = document(content, |doc| {
            let p1 = tiling(doc, Dictionary::new(), b"0 0 5 5 re f");
            dictionary! {
                "Pattern" => dictionary! { "P1" => reference(p1) },
                // One that is a pattern space, one that is not, one that is broken.
                "ColorSpace" => dictionary! {
                    "Cs1" => vec![name("Pattern"), name("DeviceRGB")],
                    "Cs2" => vec![name("Pattern")],
                    "Cs3" => vec![reference((9999, 0)), name("Pattern")],
                    "Cs4" => name("Pattern"),
                    "Cs5" => Object::Array(vec![]),
                },
            }
        });
        all_the_ways(&String::from_utf8_lossy(content), bytes);
    }

    // A thousand operands before the name.
    let mut many = Vec::new();
    for _ in 0..1000 {
        many.extend_from_slice(b"1 ");
    }
    many.extend_from_slice(b"/P1 scn 0 0 5 5 re f");
    let bytes = document(&many, |doc| {
        let p1 = tiling(doc, Dictionary::new(), b"0 0 5 5 re f");
        dictionary! { "Pattern" => dictionary! { "P1" => reference(p1) } }
    });
    all_the_ways("a thousand color components", bytes);

    // Many pattern color spaces.
    let bytes = document(b"/Cs0 cs /P1 scn 0 0 5 5 re f", |doc| {
        let p1 = tiling(doc, Dictionary::new(), b"0 0 5 5 re f");
        let mut spaces = Dictionary::new();
        for i in 0..20_000 {
            spaces.set(format!("Cs{i}"), vec![name("Pattern"), name("DeviceGray")]);
        }
        dictionary! { "Pattern" => dictionary! { "P1" => reference(p1) }, "ColorSpace" => spaces }
    });
    all_the_ways("20000 pattern color spaces", bytes);
}

#[test]
fn a_form_that_uses_patterns() {
    // A form whose pattern is in its own resources, one whose pattern is missing, one whose
    // /Pattern resource names the form itself, and a form without resources that sets a pattern.
    let bytes = document(b"/Fm1 Do /Fm2 Do /Fm3 Do /Fm4 Do", |doc| {
        let p1 = tiling(doc, Dictionary::new(), b"0 0 5 5 re f");
        let form = |doc: &mut Document, dict: Dictionary, content: &[u8]| {
            let mut d = dictionary! {
                "Type" => "XObject", "Subtype" => "Form", "BBox" => numbers(&[0.0, 0.0, 200.0, 200.0]),
            };
            for (key, value) in dict.iter() {
                d.set(key.clone(), value.clone());
            }
            doc.add_object(Stream::new(d, content.to_vec()))
        };
        let fm1 = form(
            doc,
            dictionary! { "Resources" => dictionary! {
                "Pattern" => dictionary! { "P1" => reference(p1) } } },
            b"/Pattern cs /P1 scn 0 0 100 100 re f",
        );
        let fm2 = form(
            doc,
            dictionary! { "Resources" => dictionary! {} },
            b"/Pattern cs /P1 scn 0 0 100 100 re f",
        );
        let fm3 = doc.new_object_id();
        let d3 = dictionary! {
            "Type" => "XObject", "Subtype" => "Form", "BBox" => numbers(&[0.0, 0.0, 200.0, 200.0]),
            "Resources" => dictionary! {
                "XObject" => dictionary! { "Fm3" => reference(fm3) },
                "Pattern" => dictionary! { "Fm3" => reference(fm3), "P1" => reference(p1) },
            },
        };
        doc.objects.insert(
            fm3,
            Object::Stream(Stream::new(d3, b"/Fm3 Do /Pattern cs /P1 scn 0 0 9 9 re f".to_vec())),
        );
        let fm4 = form(doc, Dictionary::new(), b"/Pattern cs /P1 scn 0 0 100 100 re f");
        dictionary! {
            "XObject" => dictionary! {
                "Fm1" => reference(fm1), "Fm2" => reference(fm2),
                "Fm3" => reference(fm3), "Fm4" => reference(fm4),
            },
            "Pattern" => dictionary! { "P1" => reference(p1) },
        }
    });
    let (_, warnings) = all_the_ways("forms with patterns", bytes);
    let _ = warnings;
}

// ---------------------------------------------------------------------------------------------
// Documents built in code

fn fill(x: f32, y: f32, w: f32, h: f32) -> Op {
    Op::DrawRectangle {
        rectangle: Rect {
            mode: Some(PaintMode::Fill),
            ..Rect::from_xywh(Pt(x), Pt(y), Pt(w), Pt(h))
        },
    }
}

fn page_with(doc: &mut PdfDocument, ops: Vec<Op>) {
    doc.pages.push(PdfPage::new(Mm(100.0), Mm(100.0), ops));
}

/// Saves `doc`, parses the result, and draws the first page.
fn round_trip(what: &str, doc: PdfDocument) -> (PdfDocument, Vec<PdfWarnMsg>) {
    bounded(what, move || {
        let mut warnings = Vec::new();
        let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
        let parsed = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut warnings)
            .expect("the saved document parses");
        for page in &parsed.pages {
            let _ = page.to_svg(&parsed.resources, &PdfToSvgOptions::default(), &mut warnings);
        }
        let _ = doc.pages.iter().map(|p| p.to_svg(&doc.resources, &PdfToSvgOptions::default(), &mut warnings)).count();
        (parsed, warnings)
    })
}

#[test]
fn numbers_that_are_not_numbers_are_not_written() {
    let mut doc = PdfDocument::new("nan");
    let nan = f32::NAN;
    let inf = f32::INFINITY;
    let cell = Rect::from_wh(Pt(nan), Pt(inf));
    doc.resources.patterns.map.insert(
        PatternId("A".into()),
        Pattern::colored_tiling(cell, nan, inf, vec![fill(nan, inf, nan, 1.0)])
            .with_matrix([nan, inf, -inf, nan, 1e38, -1e38]),
    );
    doc.resources.patterns.map.insert(
        PatternId("B".into()),
        Pattern::colored_tiling(Rect::from_wh(Pt(10.0), Pt(10.0)), 0.0, -0.0, Vec::new()),
    );
    doc.resources.patterns.map.insert(
        PatternId("C".into()),
        Pattern::from_shading(Shading {
            geometry: ShadingGeometry::Radial { coords: [nan, nan, -1.0, inf, inf, nan] },
            stops: vec![GradientStop { offset: nan, color: [nan, 2.0, -1.0] }],
            extend: (false, false),
        })
        .with_matrix([nan; 6]),
    );
    page_with(
        &mut doc,
        vec![
            Op::SetFillPattern { id: PatternId("A".into()), color: Some(rgb(nan, 5.0, -3.0)) },
            fill(0.0, 0.0, 10.0, 10.0),
            Op::SetOutlinePattern { id: PatternId("B".into()), color: Some(rgb(inf, 0.0, 0.0)) },
            Op::SetFillPattern { id: PatternId("C".into()), color: None },
            Op::SetFillPattern { id: PatternId("Missing".into()), color: None },
            fill(0.0, 0.0, 10.0, 10.0),
        ],
    );
    let (parsed, warnings) = round_trip("nan pattern", doc);
    assert!(!parsed.resources.patterns.map.is_empty(), "{warnings:?}");
    // What is written has no number that is not finite: it parses back as a pattern.
    for (id, pattern) in &parsed.resources.patterns.map {
        if let Pattern::Tiling { xstep, ystep, matrix, .. } = pattern {
            assert!(xstep.is_finite() && *xstep != 0.0, "{}: xstep {xstep}", id.0);
            assert!(ystep.is_finite() && *ystep != 0.0, "{}: ystep {ystep}", id.0);
            assert!(matrix.iter().all(|v| v.is_finite()), "{}: {matrix:?}", id.0);
        }
    }
    assert!(warnings.iter().any(|w| w.msg.contains("pattern A")), "{warnings:?}");
}

fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::Rgb(Rgb::new(r, g, b, None))
}

#[test]
fn patterns_nested_deeper_than_the_limit_are_not_written() {
    // Resources inside resources, 40 levels. The writer stops at 16.
    let mut pattern = Pattern::colored_tiling(Rect::from_wh(Pt(1.0), Pt(1.0)), 1.0, 1.0, vec![fill(0.0, 0.0, 1.0, 1.0)]);
    for _ in 0..40 {
        let mut resources = PdfResources::default();
        resources.patterns.map.insert(PatternId("Inner".into()), pattern);
        pattern = Pattern::colored_tiling(
            Rect::from_wh(Pt(1.0), Pt(1.0)),
            1.0,
            1.0,
            vec![Op::SetFillPattern { id: PatternId("Inner".into()), color: None }, fill(0.0, 0.0, 1.0, 1.0)],
        )
        .with_resources(resources);
    }
    let mut doc = PdfDocument::new("deep");
    doc.resources.patterns.map.insert(PatternId("Outer".into()), pattern);
    page_with(
        &mut doc,
        vec![Op::SetFillPattern { id: PatternId("Outer".into()), color: None }, fill(0.0, 0.0, 10.0, 10.0)],
    );
    let (_, warnings) = round_trip("deep patterns", doc);
    assert!(
        warnings.iter().any(|w| w.msg.contains("nested deeper")),
        "{warnings:?}"
    );
}

#[test]
fn many_patterns_in_code() {
    let mut doc = PdfDocument::new("many");
    let mut ops = Vec::new();
    for i in 0..3000 {
        let id = PatternId(format!("P{i}"));
        doc.resources.patterns.map.insert(
            id.clone(),
            Pattern::colored_tiling(Rect::from_wh(Pt(4.0), Pt(4.0)), 4.0, 4.0, vec![fill(0.0, 0.0, 2.0, 2.0)]),
        );
        ops.push(Op::SetFillPattern { id, color: None });
        ops.push(fill(0.0, 0.0, 5.0, 5.0));
    }
    page_with(&mut doc, ops);
    let (parsed, warnings) = round_trip("3000 patterns", doc);
    assert_eq!(parsed.resources.patterns.map.len(), 3000, "{:?}", warnings.len());
}

#[test]
fn an_external_pattern_that_is_not_a_pattern() {
    let mut doc = PdfDocument::new("external");
    doc.resources.patterns.map.insert(PatternId("N".into()), Pattern::External(DictItem::Null));
    doc.resources.patterns.map.insert(PatternId("I".into()), Pattern::External(DictItem::Int(5)));
    doc.resources.patterns.map.insert(
        PatternId("S".into()),
        Pattern::External(DictItem::Stream { stream: ExternalStream::default() }),
    );
    doc.resources.patterns.map.insert(
        PatternId("R".into()),
        Pattern::External(DictItem::Ref { obj: 99999, gen: 0 }),
    );
    page_with(
        &mut doc,
        vec![Op::SetFillPattern { id: PatternId("N".into()), color: None }, fill(0.0, 0.0, 10.0, 10.0)],
    );
    round_trip("external patterns that are not patterns", doc);
}
