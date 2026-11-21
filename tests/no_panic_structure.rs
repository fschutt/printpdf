//! Hostile documents against the structural part of the parser: document loading, the page tree,
//! resources (fonts, XObjects, shadings, graphics states), the document information, outlines,
//! link annotations and layers. Whatever the input, parsing returns (an `Err` is fine), within a
//! bounded time and memory, and without a panic.
//!
//! Documents are built with `lopdf` so every case is precise.
//!
//! - `mod panics`: values that panic on an arithmetic overflow or a slip of an index.
//! - `mod resources`: streams and shared resources whose size is the point.
//! - `mod walks`: cycles, deep nesting, shared objects, and documents mutated at random.
//!
//! A call that does not return cannot be cancelled, and the loops tested here grow memory
//! without bound, so a timeout ends the whole test process (see `bounded`). The modules sort in
//! the order above, so with `--test-threads=1` the results of the other tests are printed before
//! a hang ends the process.

use std::io::Write as _;
use std::sync::{mpsc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::Duration;

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream, StringFormat};
use printpdf::{
    DictItem, Op, PdfDocument, PdfParseErrorSeverity, PdfParseOptions, PdfWarnMsg, RawImageData,
    XObject,
};

const TIMEOUT: Duration = Duration::from_secs(5);

/// The stack of a WebAssembly module is 1 MiB. Parsing has to fit in it.
const STACK_SIZE: usize = 1 << 20;

/// More than the 256 MiB a single stream may decode to.
const BOMB_LEN: usize = 300 * 1024 * 1024;

/// The tests that hold hundreds of MiB at once take this lock, so that they do not add up when
/// the tests run in parallel.
static HEAVY: Mutex<()> = Mutex::new(());

fn heavy() -> MutexGuard<'static, ()> {
    HEAVY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Runs `f` on a thread with the stack of a WebAssembly module and waits at most [`TIMEOUT`].
///
/// A panic of `f` is raised again here. A timeout ends the process: the thread cannot be
/// stopped, and the loops this file tests grow memory without bound.
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

fn int(i: i64) -> Object {
    Object::Integer(i)
}

fn string(bytes: &[u8]) -> Object {
    Object::String(bytes.to_vec(), StringFormat::Literal)
}

fn array(items: Vec<Object>) -> Object {
    Object::Array(items)
}

fn numbers(values: &[f32]) -> Object {
    Object::Array(values.iter().map(|v| Object::Real(*v)).collect())
}

fn reference(id: ObjectId) -> Object {
    Object::Reference(id)
}

/// A document with a catalog, a page tree node and one page (200 x 100, no content).
struct Pdf {
    doc: Document,
    catalog: ObjectId,
    pages: ObjectId,
    page: ObjectId,
}

fn pdf() -> Pdf {
    let mut doc = Document::with_version("1.5");
    let pages = doc.new_object_id();
    let page = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => reference(pages),
        "MediaBox" => numbers(&[0.0, 0.0, 200.0, 100.0]),
    });
    doc.objects.insert(
        pages,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => 1,
            "Kids" => array(vec![reference(page)]),
        }),
    );
    let catalog = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => reference(pages),
    });
    doc.trailer.set("Root", reference(catalog));
    Pdf { doc, catalog, pages, page }
}

/// The dictionary of an object that is a dictionary or a stream.
fn dict_mut(doc: &mut Document, id: ObjectId) -> &mut Dictionary {
    match doc.objects.get_mut(&id) {
        Some(Object::Dictionary(d)) => d,
        Some(Object::Stream(s)) => &mut s.dict,
        _ => panic!("object {id:?} is neither a dictionary nor a stream"),
    }
}

impl Pdf {
    fn add(&mut self, object: impl Into<Object>) -> ObjectId {
        self.doc.add_object(object)
    }

    fn set(&mut self, id: ObjectId, key: &str, value: impl Into<Object>) {
        dict_mut(&mut self.doc, id).set(key, value);
    }

    /// Sets `/Resources` of the page to a dictionary with these entries.
    fn page_resources(&mut self, entries: Dictionary) {
        let page = self.page;
        self.set(page, "Resources", entries);
    }

    fn bytes(mut self) -> Vec<u8> {
        document_bytes(&mut self.doc)
    }
}

fn document_bytes(doc: &mut Document) -> Vec<u8> {
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("save");
    out
}

// ---------------------------------------------------------------------------------------------
// Parsing

#[derive(Debug, Clone)]
enum Kind {
    /// A decoded image: width, height and the number of samples.
    Image(usize, usize, usize),
    Form,
    /// A preserved stream: the number of `Leaf` entries and nested streams in its dictionary.
    External { leaves: usize, streams: usize },
}

/// A parsed document reduced to what the tests look at. It is built on the parsing thread, so
/// the result needs no `Send` bound.
#[derive(Debug)]
struct Parsed {
    result: Result<(), String>,
    pages: usize,
    /// `[x, y, width, height]` of the media box of each page.
    media_boxes: Vec<[f32; 4]>,
    links: usize,
    bookmarks: Vec<(String, usize)>,
    xobjects: Vec<(String, Kind)>,
    shadings: usize,
    title: String,
    warnings: Vec<PdfWarnMsg>,
}

fn leaves_and_streams(item: &DictItem) -> (usize, usize) {
    let add = |a: (usize, usize), b: (usize, usize)| (a.0 + b.0, a.1 + b.1);
    match item {
        DictItem::Dict { map } => map.iter().fold((0, 0), |sum, (key, value)| {
            add(add(sum, (usize::from(key == "Leaf"), 0)), leaves_and_streams(value))
        }),
        DictItem::Array(items) => items
            .iter()
            .fold((0, 0), |sum, value| add(sum, leaves_and_streams(value))),
        DictItem::Stream { stream } => {
            let inner = stream
                .dict
                .values()
                .fold((0, 0), |sum, value| add(sum, leaves_and_streams(value)));
            add((0, 1), inner)
        }
        _ => (0, 0),
    }
}

fn parse(bytes: &[u8]) -> Parsed {
    let mut warnings = Vec::new();
    match PdfDocument::parse(bytes, &PdfParseOptions::default(), &mut warnings) {
        Err(e) => Parsed {
            result: Err(e),
            pages: 0,
            media_boxes: Vec::new(),
            links: 0,
            bookmarks: Vec::new(),
            xobjects: Vec::new(),
            shadings: 0,
            title: String::new(),
            warnings,
        },
        Ok(doc) => Parsed {
            result: Ok(()),
            pages: doc.pages.len(),
            media_boxes: doc
                .pages
                .iter()
                .map(|p| {
                    let r = &p.media_box;
                    [r.x.0, r.y.0, r.width.0, r.height.0]
                })
                .collect(),
            links: doc
                .pages
                .iter()
                .flat_map(|p| p.ops.iter())
                .filter(|op| matches!(op, Op::LinkAnnotation { .. }))
                .count(),
            bookmarks: doc
                .bookmarks
                .map
                .values()
                .map(|b| (b.name.clone(), b.page))
                .collect(),
            xobjects: doc
                .resources
                .xobjects
                .map
                .iter()
                .map(|(id, x)| {
                    let kind = match x {
                        XObject::Image(img) => {
                            let samples = match &img.pixels {
                                RawImageData::U8(v) => v.len(),
                                RawImageData::U16(v) => v.len(),
                                RawImageData::F32(v) => v.len(),
                            };
                            Kind::Image(img.width, img.height, samples)
                        }
                        XObject::Form(_) => Kind::Form,
                        XObject::External(ext) => {
                            let (leaves, streams) = ext
                                .stream
                                .dict
                                .values()
                                .fold((0, 0), |sum, v| {
                                    let (l, s) = leaves_and_streams(v);
                                    (sum.0 + l, sum.1 + s)
                                });
                            Kind::External { leaves, streams }
                        }
                    };
                    (id.0.clone(), kind)
                })
                .collect(),
            shadings: doc.resources.shadings.map.len(),
            title: doc.metadata.info.document_title.clone(),
            warnings,
        },
    }
}

/// Parses `bytes` on a bounded thread.
fn parse_bounded(what: &str, bytes: Vec<u8>) -> Parsed {
    bounded(what, move || parse(&bytes))
}

fn has_warning(parsed: &Parsed, needle: &str) -> bool {
    parsed.warnings.iter().any(|w| w.msg.contains(needle))
}

fn messages(parsed: &Parsed) -> Vec<&str> {
    parsed
        .warnings
        .iter()
        .filter(|w| w.severity != PdfParseErrorSeverity::Info)
        .map(|w| w.msg.as_str())
        .collect()
}

/// Every decoded image holds at least one sample for each of its pixels.
fn assert_images_consistent(what: &str, parsed: &Parsed) {
    for (id, kind) in &parsed.xobjects {
        if let Kind::Image(width, height, samples) = kind {
            assert!(
                *samples as u128 >= (*width as u128) * (*height as u128),
                "{what}: image {id} declares {width} x {height} pixels but holds {samples} samples"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Documents that share a few shapes

const A4: [f32; 4] = [0.0, 0.0, 595.28, 841.89];

fn same(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 0.01)
}

/// A document whose page draws `/Im0`, an image with this dictionary and these bytes. With
/// `smask` the image has a soft mask of that width and height.
fn image_pdf(
    width: i64,
    height: i64,
    bits: i64,
    space: &str,
    data: &[u8],
    smask: Option<(i64, i64)>,
) -> Vec<u8> {
    let mut p = pdf();
    let mut image = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => width,
        "Height" => height,
        "ColorSpace" => space,
        "BitsPerComponent" => bits,
    };
    if let Some((w, h)) = smask {
        let mask = p.add(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => w,
                "Height" => h,
                "ColorSpace" => "DeviceGray",
                "BitsPerComponent" => 8,
            },
            vec![255; 4],
        ));
        image.set("SMask", reference(mask));
    }
    let image = p.add(Stream::new(image, data.to_vec()));
    p.page_resources(dictionary! {
        "XObject" => dictionary! { "Im0" => reference(image) },
    });
    p.bytes()
}

/// Gives the page an outline with one entry for each title, chained with `/Next`. The last
/// entry continues with entry `cycle_to` when that is set.
fn add_outline(p: &mut Pdf, titles: &[&[u8]], cycle_to: Option<usize>) {
    let ids: Vec<ObjectId> = titles.iter().map(|_| p.doc.new_object_id()).collect();
    let outlines = p.doc.new_object_id();
    for (i, (id, title)) in ids.iter().zip(titles).enumerate() {
        let mut item = dictionary! {
            "Title" => string(title),
            "Parent" => reference(outlines),
            "Dest" => array(vec![reference(p.page), name("Fit")]),
        };
        let next = match ids.get(i + 1) {
            Some(next) => Some(*next),
            None => cycle_to.and_then(|c| ids.get(c).copied()),
        };
        if let Some(next) = next {
            item.set("Next", reference(next));
        }
        p.doc.objects.insert(*id, Object::Dictionary(item));
    }
    let mut outlines_dict = dictionary! {
        "Type" => "Outlines",
        "Count" => titles.len() as i64,
    };
    if let Some(first) = ids.first() {
        outlines_dict.set("First", reference(*first));
    }
    p.doc.objects.insert(outlines, Object::Dictionary(outlines_dict));
    let catalog = p.catalog;
    p.set(catalog, "Outlines", reference(outlines));
}

/// A zlib stream of `len` spaces, the smallest stream that decodes to more than a stream may.
fn bomb() -> &'static [u8] {
    static BOMB: OnceLock<Vec<u8>> = OnceLock::new();
    BOMB.get_or_init(|| {
        use flate2::{write::ZlibEncoder, Compression};
        let chunk = vec![b' '; 1 << 20];
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        for _ in 0..BOMB_LEN / chunk.len() {
            encoder.write_all(&chunk).expect("compress");
        }
        encoder.finish().expect("compress")
    })
}

// ---------------------------------------------------------------------------------------------

/// Values that panic on an arithmetic overflow or a slip of an index.
mod panics {
    use super::*;

    /// `/Width` and `/Height` come straight from the file. Products of them wrap around in
    /// release builds and panic with overflow checks; negative numbers are huge as `usize`.
    #[test]
    fn image_sizes_that_overflow() {
        let cases: [(i64, i64, i64, &str); 10] = [
            // 3 * width wraps around to 2
            (6_148_914_691_236_517_206, 1, 8, "DeviceRGB"),
            (-1, 1, 8, "DeviceRGB"),
            (3, -1, 8, "DeviceGray"),
            // 2 * 3 * width wraps around
            (1 << 62, 1, 16, "DeviceRGB"),
            (1 << 62, 1, 16, "DeviceGray"),
            (i64::MAX, i64::MAX, 8, "DeviceGray"),
            (1 << 40, 1 << 40, 8, "DeviceRGB"),
            (0, 0, 8, "DeviceRGB"),
            (1, 1, -8, "DeviceRGB"),
            (i64::MIN, i64::MIN, 8, "DeviceRGB"),
        ];
        for (width, height, bits, space) in cases {
            let what = format!("image {width} x {height}, {bits} bits, {space}");
            let bytes = image_pdf(width, height, bits, space, &[1, 2, 3, 4, 5, 6], None);
            let parsed = parse_bounded(&what, bytes);
            assert!(parsed.result.is_ok(), "{what}: {:?}", parsed.result);
            assert_eq!(parsed.pages, 1, "{what}");
            assert_images_consistent(&what, &parsed);
        }
    }

    /// The soft mask of an image has dimensions of its own.
    #[test]
    fn soft_mask_sizes_that_overflow() {
        let masks: [(i64, i64); 6] = [
            (1 << 32, 1 << 32),
            (-1, 2),
            (2, -1),
            (6_148_914_691_236_517_206, 3),
            (i64::MAX, i64::MAX),
            (i64::MIN, 1),
        ];
        for (w, h) in masks {
            let what = format!("soft mask {w} x {h}");
            let bytes = image_pdf(1, 1, 8, "DeviceRGB", &[10, 20, 30], Some((w, h)));
            let parsed = parse_bounded(&what, bytes);
            assert!(parsed.result.is_ok(), "{what}: {:?}", parsed.result);
            assert_images_consistent(&what, &parsed);
        }
    }

    /// A matching soft mask still becomes the alpha channel.
    #[test]
    fn matching_soft_mask_is_merged() {
        let bytes = image_pdf(1, 1, 8, "DeviceRGB", &[10, 20, 30], Some((1, 1)));
        let parsed = parse_bounded("matching soft mask", bytes);
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert_eq!(parsed.xobjects.len(), 1);
        match &parsed.xobjects[0].1 {
            Kind::Image(1, 1, samples) => {
                assert!(cfg!(feature = "images"));
                assert_eq!(*samples, 4, "RGB and alpha");
            }
            Kind::External { .. } => assert!(!cfg!(feature = "images")),
            other => panic!("unexpected {other:?}"),
        }
    }

    /// A bitmap that declares far more pixels than its data holds is not allocated.
    #[test]
    fn image_that_declares_more_pixels_than_it_holds() {
        for (w, h) in [(20_000, 20_000), (1 << 20, 1 << 20), (65_536, 65_536)] {
            let what = format!("image {w} x {h}");
            let bytes = image_pdf(w, h, 8, "DeviceRGB", &[1, 2, 3], None);
            let parsed = parse_bounded(&what, bytes);
            assert!(parsed.result.is_ok(), "{what}: {:?}", parsed.result);
            assert_images_consistent(&what, &parsed);
            assert!(
                parsed
                    .xobjects
                    .iter()
                    .all(|(_, k)| !matches!(k, Kind::Image(..))),
                "{what}: no pixels, no image"
            );
        }
    }

    /// `/MediaBox` is an array of four numbers. Anything else falls back to A4 with a warning.
    #[test]
    fn rectangles_with_odd_arrays() {
        let invalid: Vec<(&str, Object)> = vec![
            ("empty", array(vec![])),
            ("one number", array(vec![int(0)])),
            ("three numbers", array(vec![int(0), int(0), int(1)])),
            (
                "five numbers",
                array(vec![int(0), int(0), int(1), int(1), int(1)]),
            ),
            (
                "a string",
                array(vec![int(0), int(0), string(b"x"), int(1)]),
            ),
            (
                "a nested array",
                array(vec![int(0), int(0), array(vec![int(1)]), int(1)]),
            ),
            ("null", Object::Null),
            ("an integer", int(5)),
            ("a dangling reference", reference((999, 0))),
            ("a name", name("Letter")),
        ];
        for (what, value) in invalid {
            let mut p = pdf();
            let page = p.page;
            p.set(page, "MediaBox", value);
            let parsed = parse_bounded(what, p.bytes());
            assert_eq!(parsed.pages, 1, "{what}");
            assert!(same(parsed.media_boxes[0], A4), "{what}: {:?}", parsed.media_boxes);
            assert!(has_warning(&parsed, "MediaBox"), "{what}: {:?}", messages(&parsed));
        }

        // Valid boxes are read as they are, and the trim and crop boxes of the page too.
        let mut p = pdf();
        let page = p.page;
        p.set(page, "MediaBox", numbers(&[10.0, 20.0, 110.0, 220.0]));
        p.set(page, "CropBox", array(vec![int(1), int(2)]));
        p.set(page, "TrimBox", array(vec![int(1), int(2), int(3), int(4), int(5)]));
        let parsed = parse_bounded("valid media box", p.bytes());
        assert!(same(parsed.media_boxes[0], [10.0, 20.0, 100.0, 200.0]));
    }

    /// `/Rect`, `/Border`, `/C` and the action of a link have arrays of any length.
    #[test]
    fn link_annotations_with_odd_values() {
        let uri = || dictionary! { "S" => "URI", "URI" => string(b"http://example.com") };
        let rect = || numbers(&[0.0, 0.0, 10.0, 10.0]);
        let link = |rect: Object, action: Object| {
            dictionary! {
                "Type" => "Annot",
                "Subtype" => "Link",
                "Rect" => rect,
                "A" => action,
            }
        };
        let mut p = pdf();
        let page_id = p.page;
        let goto = |dest: Option<Object>| {
            let mut action = dictionary! { "S" => "GoTo" };
            if let Some(dest) = dest {
                action.set("D", dest);
            }
            Object::Dictionary(action)
        };
        let mut annots: Vec<(Dictionary, bool)> = vec![
            (link(rect(), Object::Dictionary(uri())), true),
            (
                link(
                    rect(),
                    goto(Some(array(vec![
                        reference(page_id),
                        name("XYZ"),
                        int(1),
                        int(2),
                        int(3),
                    ]))),
                ),
                true,
            ),
            (link(rect(), goto(Some(array(vec![])))), true),
            (link(rect(), goto(Some(array(vec![reference(page_id), name("XYZ")])))), true),
            (link(rect(), goto(None)), false),
            (link(rect(), Object::Dictionary(dictionary! { "S" => "URI" })), false),
            (link(rect(), Object::Dictionary(dictionary! { "S" => "Launch" })), false),
            (link(rect(), Object::Dictionary(dictionary! {})), false),
            (link(rect(), reference((999, 0))), false),
            (link(array(vec![]), Object::Dictionary(uri())), false),
            (link(numbers(&[0.0, 0.0, 10.0]), Object::Dictionary(uri())), false),
            (link(numbers(&[0.0, 0.0, 10.0, 10.0, 10.0]), Object::Dictionary(uri())), false),
            (
                link(
                    array(vec![int(0), int(0), name("x"), int(10)]),
                    Object::Dictionary(uri()),
                ),
                false,
            ),
        ];
        // Odd borders and colors keep the link and use the defaults.
        for border in [vec![], vec![1.0], vec![1.0, 2.0, 3.0], vec![1.0; 4], vec![1.0; 5]] {
            let mut annot = link(rect(), Object::Dictionary(uri()));
            annot.set("Border", numbers(&border));
            annots.push((annot, true));
        }
        for color in [vec![], vec![1.0], vec![1.0, 2.0], vec![1.0; 3], vec![1.0; 4], vec![1.0; 5]] {
            let mut annot = link(rect(), Object::Dictionary(uri()));
            annot.set("C", numbers(&color));
            annot.set("H", name("Z"));
            annots.push((annot, true));
        }
        // No subtype, or another one.
        let mut no_subtype = link(rect(), Object::Dictionary(uri()));
        no_subtype.remove(b"Subtype");
        annots.push((no_subtype, false));
        annots.push((
            dictionary! { "Type" => "Annot", "Subtype" => "Text", "Rect" => rect() },
            false,
        ));

        let expected = annots.iter().filter(|(_, keep)| *keep).count();
        let ids: Vec<Object> = annots
            .into_iter()
            .map(|(annot, _)| reference(p.add(annot)))
            .collect();
        p.set(page_id, "Annots", array(ids));
        let parsed = parse_bounded("link annotations", p.bytes());
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert_eq!(parsed.links, expected, "{:?}", messages(&parsed));
    }

    /// Outline titles are text strings: UTF-16BE with a byte order mark, or UTF-8.
    #[test]
    fn bookmark_titles_in_odd_encodings() {
        let titles: [(&[u8], &str); 9] = [
            (b"", ""),
            (b"Hello", "Hello"),
            (b"\xFE", "\u{FFFD}"),
            (b"\xFE\xFF", ""),
            (b"\xFE\xFF\x00\x48\x00\x69", "Hi"),
            // An odd last byte is the high byte of a code unit.
            (b"\xFE\xFF\x00\x41\x42", "A\u{4200}"),
            (b"\xFE\xFF\xD8\x00", "\u{FFFD}"),
            (b"\xFF\xFE\x41\x00", "\u{FFFD}\u{FFFD}A\0"),
            (b"\xFE\xFF\xDC\x00\x00\x41", "\u{FFFD}A"),
        ];
        let mut p = pdf();
        let raw: Vec<&[u8]> = titles.iter().map(|(t, _)| *t).collect();
        add_outline(&mut p, &raw, None);
        let parsed = parse_bounded("bookmark titles", p.bytes());
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        let names: Vec<&str> = parsed.bookmarks.iter().map(|(n, _)| n.as_str()).collect();
        let expected: Vec<&str> = titles.iter().map(|(_, e)| *e).collect();
        assert_eq!(names, expected);
        assert!(parsed.bookmarks.iter().all(|(_, page)| *page == 1));
    }

    /// The information dictionary: text strings and dates of any shape.
    #[test]
    fn info_dictionary_with_odd_values() {
        let long_date = vec![b'9'; 1 << 20];
        let multibyte = "D:2024\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}".as_bytes().to_vec();
        let dates: Vec<&[u8]> = vec![
            b"",
            b"D:",
            b"D:2024",
            b"D:20240101000000",
            b"D:20240230000000",
            b"D:20241345250000",
            b"D:20240101000000+99'99'99'",
            b"D:20240101000000-",
            b"D:20240101000000+5",
            b"D:20240101000000Z00'00'",
            b"99999999999999999999999999999",
            b"-2024010100000",
            &multibyte,
            &long_date,
        ];
        for date in dates {
            for key in ["CreationDate", "ModDate"] {
                let mut p = pdf();
                let info = p.add(dictionary! {
                    key => string(date),
                    "Title" => string(b"\xFE\xFF\x00A"),
                });
                p.doc.trailer.set("Info", reference(info));
                let what = format!("{key} {:?}", String::from_utf8_lossy(&date[..date.len().min(24)]));
                let parsed = parse_bounded(&what, p.bytes());
                assert!(parsed.result.is_ok(), "{what}: {:?}", parsed.result);
                assert_eq!(parsed.title, "A", "{what}");
            }
        }

        // Entries of the wrong type, odd text, and an /Info that is not a dictionary.
        let odd_values: Vec<(&str, Object)> = vec![
            ("Title", int(5)),
            ("Title", array(vec![string(b"a")])),
            ("Title", string(b"\xFE")),
            ("Title", string(b"\xFE\xFF\x00")),
            ("Author", string(b"\xFE\xFF\xD8\x00\x00")),
            ("Keywords", string(b",,, , a,\xFE\xFF\x00,\x00,")),
            ("Keywords", string(&vec![b','; 100_000])),
            ("Trapped", int(1)),
            ("Trapped", string(b"True")),
            ("GTS_PDFXVersion", string(b"")),
            ("GTS_PDFXVersion", name("x")),
            ("CreationDate", int(0)),
            ("ModDate", array(vec![])),
        ];
        for (key, value) in odd_values {
            let mut p = pdf();
            let info = p.add(dictionary! { key => value });
            p.doc.trailer.set("Info", reference(info));
            let parsed = parse_bounded(key, p.bytes());
            assert!(parsed.result.is_ok(), "{key}: {:?}", parsed.result);
        }
        for info in [int(5), array(vec![]), reference((999, 0)), Object::Null, string(b"x")] {
            let mut p = pdf();
            p.doc.trailer.set("Info", info);
            let parsed = parse_bounded("odd /Info", p.bytes());
            assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        }
    }

    /// Composite fonts with descendants, descriptors and encodings that are missing or of the
    /// wrong type.
    #[test]
    fn fonts_with_odd_descendants_and_encodings() {
        const CMAP: &[u8] = b"begincmap\n1 begincodespacerange <0000> <FFFF> endcodespacerange\n\
                              1 begincidrange <0000> <00FF> 0 endcidrange\nendcmap";
        const TO_UNICODE: &[u8] = b"begincmap\n1 beginbfchar <0001> <0041> endbfchar\nendcmap";
        for case in 0..20 {
            let mut p = pdf();
            let cmap = p.add(Stream::new(dictionary! {}, CMAP.to_vec()));
            let to_unicode = p.add(Stream::new(dictionary! {}, TO_UNICODE.to_vec()));
            let descriptor = p.add(dictionary! {
                "Type" => "FontDescriptor", "FontName" => "Foo", "Flags" => 4,
            });
            let descendant = p.add(dictionary! {
                "Type" => "Font", "Subtype" => "CIDFontType2", "BaseFont" => "Foo",
                "FontDescriptor" => reference(descriptor),
            });
            let array_of_descendant = p.add(array(vec![reference(descendant)]));
            let not_a_stream = p.add(dictionary! {});
            let mut font = dictionary! {
                "Type" => "Font", "Subtype" => "Type0", "BaseFont" => "Foo",
            };
            match case {
                0 => {}
                1 => font.set("DescendantFonts", array(vec![])),
                2 => font.set("DescendantFonts", array(vec![Object::Null])),
                3 => font.set("DescendantFonts", array(vec![reference((999, 0))])),
                4 => font.set("DescendantFonts", array(vec![Object::Dictionary(dictionary! {})])),
                5 => font.set("DescendantFonts", reference(array_of_descendant)),
                6 => font.set("DescendantFonts", reference((999, 0))),
                7 => font.set("DescendantFonts", reference(descendant)),
                8 => font.set("DescendantFonts", array(vec![int(5), reference(descendant)])),
                9 => font.set("DescendantFonts", array(vec![reference(descendant)])),
                10 => {
                    font.set("DescendantFonts", array(vec![reference(descendant)]));
                    font.set("Encoding", reference(cmap));
                }
                11 => font.set("Encoding", reference(not_a_stream)),
                12 => font.set("Encoding", name("Identity-V")),
                13 => font.set("Encoding", name("90ms-RKSJ-H")),
                14 => font.set("Encoding", array(vec![])),
                15 => font.set("Encoding", reference((999, 0))),
                16 => font.set("ToUnicode", reference(to_unicode)),
                17 => font.set("ToUnicode", reference(not_a_stream)),
                18 => font.set("ToUnicode", int(5)),
                _ => {
                    font.set("DescendantFonts", array(vec![reference(descendant)]));
                    font.set("Encoding", reference(cmap));
                    font.set("ToUnicode", reference(to_unicode));
                }
            }
            let font = p.add(font);
            p.page_resources(dictionary! {
                "Font" => dictionary! { "F1" => reference(font) },
            });
            let what = format!("font case {case}");
            let parsed = parse_bounded(&what, p.bytes());
            assert!(parsed.result.is_ok(), "{what}: {:?}", parsed.result);
            assert_eq!(parsed.pages, 1, "{what}");
        }
    }

    /// Shadings with arrays of any length, and functions of any shape.
    #[test]
    fn shadings_with_odd_arrays() {
        let function = |c0: Object, c1: Object| {
            Object::Dictionary(dictionary! {
                "FunctionType" => 2, "Domain" => numbers(&[0.0, 1.0]), "N" => 1,
                "C0" => c0, "C1" => c1,
            })
        };
        let good = || function(numbers(&[1.0, 0.0, 0.0]), numbers(&[0.0, 0.0, 1.0]));
        let shading = |kind: i64, coords: Object, function: Object| {
            dictionary! {
                "ShadingType" => kind, "ColorSpace" => "DeviceRGB",
                "Coords" => coords, "Function" => function,
            }
        };
        let axial = || numbers(&[0.0, 0.0, 100.0, 0.0]);
        let radial = || numbers(&[0.0, 0.0, 0.0, 10.0, 10.0, 50.0]);
        let stitching = |functions: Object, bounds: Object| {
            Object::Dictionary(dictionary! {
                "FunctionType" => 3, "Domain" => numbers(&[0.0, 1.0]),
                "Functions" => functions, "Bounds" => bounds,
            })
        };
        let cases: Vec<(&str, Dictionary, bool)> = vec![
            ("axial", shading(2, axial(), good()), true),
            ("radial", shading(3, radial(), good()), true),
            ("axial with 3 coords", shading(2, numbers(&[0.0, 0.0, 1.0]), good()), false),
            ("axial with 5 coords", shading(2, numbers(&[0.0; 5]), good()), false),
            ("axial with no coords", shading(2, array(vec![]), good()), false),
            ("radial with 4 coords", shading(3, axial(), good()), false),
            ("type 7", shading(7, axial(), good()), false),
            ("type -1", shading(-1, axial(), good()), false),
            ("type 2^62", shading(1 << 62, axial(), good()), false),
            ("no function", dictionary! { "ShadingType" => 2, "Coords" => axial() }, false),
            ("function is an integer", shading(2, axial(), int(5)), false),
            ("function is dangling", shading(2, axial(), reference((999, 0))), false),
            ("empty colors", shading(2, axial(), function(array(vec![]), array(vec![]))), true),
            (
                "2 colors",
                shading(2, axial(), function(numbers(&[1.0, 2.0]), numbers(&[1.0, 2.0]))),
                true,
            ),
            (
                "100 colors",
                shading(2, axial(), function(numbers(&[0.5; 100]), numbers(&[0.5; 100]))),
                true,
            ),
            (
                "strings as colors",
                shading(
                    2,
                    axial(),
                    function(array(vec![string(b"a")]), array(vec![name("b")])),
                ),
                true,
            ),
            ("no functions", shading(2, axial(), stitching(array(vec![]), array(vec![]))), false),
            (
                "more bounds than functions",
                shading(
                    2,
                    axial(),
                    stitching(array(vec![good()]), numbers(&[0.2, 0.4, 0.6, 0.8])),
                ),
                true,
            ),
            (
                "fewer bounds than functions",
                shading(2, axial(), stitching(array(vec![good(), good(), good()]), numbers(&[0.5]))),
                true,
            ),
            (
                "bounds are not numbers",
                shading(
                    2,
                    axial(),
                    stitching(array(vec![good(), good()]), array(vec![name("a")])),
                ),
                true,
            ),
            (
                "functions are not dictionaries",
                shading(2, axial(), stitching(array(vec![int(1), Object::Null]), numbers(&[0.5]))),
                false,
            ),
            (
                "functions are dangling",
                shading(
                    2,
                    axial(),
                    stitching(array(vec![reference((999, 0))]), numbers(&[])),
                ),
                false,
            ),
        ];
        for (what, dict, expected) in cases {
            let mut p = pdf();
            let sh = p.add(dict);
            p.page_resources(dictionary! {
                "Shading" => dictionary! { "Sh0" => reference(sh) },
            });
            let parsed = parse_bounded(what, p.bytes());
            assert!(parsed.result.is_ok(), "{what}: {:?}", parsed.result);
            assert_eq!(parsed.shadings == 1, expected, "{what}: {:?}", messages(&parsed));
        }
    }

    /// Kids of every type.
    #[test]
    fn page_tree_kids_of_every_type() {
        let mut p = pdf();
        let page = p.page;
        let stream = p.add(Stream::new(dictionary! {}, b"q Q".to_vec()));
        let kids = array(vec![
            reference(page),
            Object::Null,
            int(5),
            string(b"x"),
            name("Page"),
            array(vec![reference(page)]),
            reference(stream),
            reference((999, 0)),
            Object::Dictionary(dictionary! { "Type" => "Page" }),
        ]);
        let pages = p.pages;
        p.set(pages, "Kids", kids);
        let parsed = parse_bounded("kids of every type", p.bytes());
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert_eq!(parsed.pages, 1);
    }

    /// `/Count` of a page tree node says how many pages are below it, as far as the file claims.
    /// Reading the pages of the document must not reserve memory for that number.
    #[test]
    fn page_tree_counts_that_announce_too_many_pages() {
        for count in [i64::MAX, 1 << 62, 1 << 40, 1_000_000_000, -1] {
            let mut p = pdf();
            let (root, page) = (p.pages, p.page);
            let below = p.add(dictionary! {
                "Type" => "Pages", "Parent" => reference(root), "Count" => count,
                "Kids" => array(vec![]),
            });
            let second = p.add(dictionary! {
                "Type" => "Pages", "Parent" => reference(root), "Count" => count,
                "Kids" => array(vec![]),
            });
            p.set(root, "Kids", array(vec![reference(page), reference(below), reference(second)]));
            // Bookmarks, links and layers all read the pages of the document.
            add_outline(&mut p, &[b"One"], None);
            let what = format!("/Count {count}");
            let parsed = parse_bounded(&what, p.bytes());
            assert!(parsed.result.is_ok(), "{what}: {:?}", parsed.result);
            assert_eq!(parsed.pages, 1, "{what}");
            assert_eq!(parsed.bookmarks.len(), 1, "{what}");
        }
    }

    /// A catalog, a page tree and pages that are the wrong kind of object are errors or empty
    /// documents, not panics.
    #[test]
    fn degenerate_documents() {
        let mut variants: Vec<(&str, Pdf)> = Vec::new();
        for (what, value) in [
            ("pages is an integer", int(5)),
            ("pages is null", Object::Null),
            ("pages is dangling", reference((999, 0))),
            ("pages is an array", array(vec![])),
            ("pages is a page", Object::Null),
        ] {
            let mut p = pdf();
            let catalog = p.catalog;
            let value = if what == "pages is a page" { reference(p.page) } else { value };
            p.set(catalog, "Pages", value);
            variants.push((what, p));
        }
        for (what, value) in [
            ("kids is an integer", int(5)),
            ("kids is a dangling reference", reference((999, 0))),
            ("kids is null", Object::Null),
        ] {
            let mut p = pdf();
            let pages = p.pages;
            p.set(pages, "Kids", value);
            variants.push((what, p));
        }
        let mut p = pdf();
        let pages = p.pages;
        dict_mut(&mut p.doc, pages).remove(b"Kids");
        variants.push(("no kids", p));
        let mut p = pdf();
        p.doc.trailer.remove(b"Root");
        variants.push(("no root", p));
        let mut p = pdf();
        p.doc.trailer.set("Root", int(5));
        variants.push(("root is an integer", p));
        let mut p = pdf();
        p.doc.trailer.set("Root", reference((999, 0)));
        variants.push(("root is dangling", p));

        for (what, p) in variants {
            // `Ok` or `Err`, but a result.
            let _ = parse_bounded(what, p.bytes());
        }
    }
}

// ---------------------------------------------------------------------------------------------

/// Cycles, deep nesting, shared objects, and mutated documents.
mod walks {
    use super::*;

    /// Entries of an outline that lead back to an earlier entry through `/Next` are read once.
    #[test]
    fn outline_next_cycles() {
        let titles: [&[u8]; 3] = [b"one", b"two", b"three"];
        // (number of entries, the entry the last one continues with, entries read)
        for (count, cycle_to, read) in [(1, 0, 1), (2, 0, 2), (3, 0, 3), (3, 1, 3), (3, 2, 3)] {
            let mut p = pdf();
            add_outline(&mut p, &titles[..count], Some(cycle_to));
            let what = format!("{count} outline entries, the last continues with {cycle_to}");
            let parsed = parse_bounded(&what, p.bytes());
            assert!(parsed.result.is_ok(), "{what}: {:?}", parsed.result);
            assert_eq!(parsed.bookmarks.len(), read, "{what}");
            assert!(has_warning(&parsed, "/Next"), "{what}: {:?}", messages(&parsed));
        }

        // An outline without a cycle gives no warning about one.
        let mut p = pdf();
        add_outline(&mut p, &titles, None);
        let parsed = parse_bounded("outline without a cycle", p.bytes());
        assert_eq!(parsed.bookmarks.len(), 3);
        assert!(!has_warning(&parsed, "/Next"), "{:?}", messages(&parsed));
    }

    /// An outline whose first entry is the outline root, and one that is its own parent.
    #[test]
    fn outline_root_as_entry() {
        let mut p = pdf();
        add_outline(&mut p, &[b"one"], None);
        let outlines = match p.doc.get_dictionary(p.catalog).and_then(|c| c.get(b"Outlines")) {
            Ok(Object::Reference(id)) => *id,
            other => panic!("no outline: {other:?}"),
        };
        p.set(outlines, "First", reference(outlines));
        p.set(outlines, "Next", reference(outlines));
        let parsed = parse_bounded("outline root as entry", p.bytes());
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
    }

    /// Page tree nodes that list each other, a page that is its own parent, and parents that
    /// form a loop.
    #[test]
    fn cyclic_page_tree() {
        // Two nodes list each other, and the second one has the page.
        let mut p = pdf();
        let (root, page) = (p.pages, p.page);
        let second = p.add(dictionary! {
            "Type" => "Pages",
            "Parent" => reference(root),
            "Kids" => array(vec![reference(root), reference(page)]),
            "Count" => 1,
        });
        p.set(root, "Kids", array(vec![reference(second), reference(second), reference(root)]));
        let parsed = parse_bounded("two nodes that list each other", p.bytes());
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert_eq!(parsed.pages, 1);

        // The page is its own parent, and the page tree node too. No media box anywhere.
        let mut p = pdf();
        let (root, page) = (p.pages, p.page);
        dict_mut(&mut p.doc, page).remove(b"MediaBox");
        p.set(page, "Parent", reference(page));
        p.set(root, "Parent", reference(root));
        let parsed = parse_bounded("parent loop", p.bytes());
        assert_eq!(parsed.pages, 1);
        assert!(same(parsed.media_boxes[0], A4), "{:?}", parsed.media_boxes);
        assert!(has_warning(&parsed, "MediaBox"), "{:?}", messages(&parsed));

        // A longer loop of parents.
        let mut p = pdf();
        let (root, page) = (p.pages, p.page);
        dict_mut(&mut p.doc, page).remove(b"MediaBox");
        let a = p.add(dictionary! { "Type" => "Pages" });
        let b = p.add(dictionary! { "Type" => "Pages", "Parent" => reference(a) });
        p.set(a, "Parent", reference(b));
        p.set(page, "Parent", reference(a));
        p.set(root, "Parent", reference(b));
        let parsed = parse_bounded("long parent loop", p.bytes());
        assert_eq!(parsed.pages, 1);
        assert!(same(parsed.media_boxes[0], A4), "{:?}", parsed.media_boxes);

        // The same page listed many times is one page.
        let mut p = pdf();
        let (root, page) = (p.pages, p.page);
        p.set(root, "Kids", array(vec![reference(page); 20_000]));
        let parsed = parse_bounded("page listed 20000 times", p.bytes());
        assert_eq!(parsed.pages, 1);
    }

    /// A page tree that is thousands of levels deep.
    #[test]
    fn deep_page_tree() {
        let mut p = pdf();
        let (root, page) = (p.pages, p.page);
        let mut parent = root;
        let mut kids = Vec::new();
        for _ in 0..3_000 {
            let node = p.doc.new_object_id();
            kids.push((parent, node));
            parent = node;
        }
        for (parent, node) in &kids {
            p.set(*parent, "Kids", array(vec![reference(*node)]));
            p.doc.objects.insert(
                *node,
                Object::Dictionary(dictionary! {
                    "Type" => "Pages", "Parent" => reference(*parent), "Count" => 1,
                }),
            );
        }
        // The deepest node holds the page.
        p.set(parent, "Kids", array(vec![reference(page)]));
        p.set(page, "Parent", reference(parent));
        let parsed = parse_bounded("page tree 3000 levels deep", p.bytes());
        // Whether the page is found does not matter, a result does.
        let _ = (parsed.result, parsed.pages);
    }

    /// A form XObject whose `/Resources` is the root of a diamond that doubles at every level.
    /// Inlining it copies every path, so it is a tree of 2^22 leaves.
    fn dag_pdf(depth: usize) -> Vec<u8> {
        let mut p = pdf();
        let mut below = p.add(dictionary! { "Leaf" => 1 });
        for _ in 0..depth {
            below = p.add(dictionary! { "A" => reference(below), "B" => reference(below) });
        }
        let form = p.add(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Form",
                "BBox" => numbers(&[0.0, 0.0, 10.0, 10.0]),
                "Resources" => reference(below),
            },
            b"q Q".to_vec(),
        ));
        p.page_resources(dictionary! {
            "XObject" => dictionary! { "Fm0" => reference(form) },
        });
        p.bytes()
    }

    #[test]
    fn diamond_references_inline_within_a_budget() {
        let _guard = heavy();
        let parsed = parse_bounded("diamond 22 levels deep", dag_pdf(22));
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert_eq!(parsed.pages, 1);
        assert_eq!(parsed.xobjects.len(), 1);
        assert!(
            has_warning(&parsed, "exceed the limit"),
            "no warning about the budget: {:?}",
            messages(&parsed)
        );
        match &parsed.xobjects[0].1 {
            Kind::External { leaves, .. } => assert!(*leaves < 1 << 22, "{leaves} leaves"),
            other => panic!("unexpected {other:?}"),
        }
    }

    /// Objects shared a few times are inlined in full, and give no warning.
    #[test]
    fn diamond_references_inline_in_full_below_the_budget() {
        let parsed = parse_bounded("diamond 6 levels deep", dag_pdf(6));
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert!(!has_warning(&parsed, "exceed the limit"), "{:?}", messages(&parsed));
        match &parsed.xobjects[0].1 {
            Kind::External { leaves, .. } => assert_eq!(*leaves, 64),
            other => panic!("unexpected {other:?}"),
        }
    }

    // -----------------------------------------------------------------------------------------
    // Mutated documents

    const TO_UNICODE: &[u8] = b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
        /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\n\
        endcodespacerange\n2 beginbfchar\n<0001> <0041>\n<0002> <0042>\nendbfchar\nendcmap\nend\nend";

    const CID_CMAP: &[u8] = b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
        /CMapName /Test-H def\n/WMode 0 def\n1 begincodespacerange\n<0000> <FFFF>\n\
        endcodespacerange\n1 begincidrange\n<0000> <00FF> 0\nendcidrange\nendcmap\nend\nend";

    /// A small document with every kind of object the structural part of the parser reads.
    fn rich_document() -> Document {
        let mut p = pdf();
        let (catalog, pages, page1) = (p.catalog, p.pages, p.page);

        let content1 = p.add(Stream::new(
            dictionary! {},
            b"q 0 0 10 10 re f Q /GS0 gs /Fm0 Do /Im0 Do /Sh0 sh".to_vec(),
        ));
        let content2 = p.add(Stream::new(dictionary! {}, b"0 0 5 5 re f".to_vec()));
        let page2 = p.add(dictionary! {
            "Type" => "Page", "Parent" => reference(pages), "Contents" => reference(content2),
            "CropBox" => numbers(&[0.0, 0.0, 100.0, 100.0]),
            "TrimBox" => numbers(&[5.0, 5.0, 95.0, 95.0]),
        });
        p.set(pages, "Kids", array(vec![reference(page1), reference(page2)]));
        p.set(pages, "Count", int(2));

        // Fonts: a standard one, and a composite one with an encoding and a ToUnicode CMap.
        let font1 = p.add(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let to_unicode = p.add(Stream::new(dictionary! {}, TO_UNICODE.to_vec()));
        let encoding = p.add(Stream::new(dictionary! {}, CID_CMAP.to_vec()));
        let descriptor = p.add(dictionary! {
            "Type" => "FontDescriptor", "FontName" => "Foo", "Flags" => 4,
            "FontBBox" => numbers(&[0.0, 0.0, 1000.0, 1000.0]), "ItalicAngle" => 0,
            "Ascent" => 800, "Descent" => -200, "CapHeight" => 700, "StemV" => 80,
        });
        let descendant = p.add(dictionary! {
            "Type" => "Font", "Subtype" => "CIDFontType2", "BaseFont" => "Foo",
            "FontDescriptor" => reference(descriptor), "CIDToGIDMap" => "Identity",
            "DW" => 1000, "W" => array(vec![int(0), array(vec![int(500)])]),
        });
        let font2 = p.add(dictionary! {
            "Type" => "Font", "Subtype" => "Type0", "BaseFont" => "Foo",
            "Encoding" => reference(encoding),
            "DescendantFonts" => array(vec![reference(descendant)]),
            "ToUnicode" => reference(to_unicode),
        });

        // An image with a soft mask, and a form.
        let mask = p.add(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 2, "Height" => 2,
                "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
            },
            vec![255, 128, 64, 0],
        ));
        let image = p.add(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 2, "Height" => 2,
                "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
                "SMask" => reference(mask),
            },
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0],
        ));
        let form = p.add(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Form",
                "BBox" => numbers(&[0.0, 0.0, 10.0, 10.0]),
                "Resources" => dictionary! {
                    "Font" => dictionary! { "F1" => reference(font1) },
                    "XObject" => dictionary! { "Im0" => reference(image) },
                },
            },
            b"q /Im0 Do Q".to_vec(),
        ));

        // A graphics state, two shadings and a layer.
        let gs = p.add(dictionary! {
            "Type" => "ExtGState", "LW" => 2, "CA" => 0.5f32, "ca" => 0.5f32, "BM" => "Multiply",
            "D" => array(vec![array(vec![int(3), int(2)]), int(0)]),
            "Font" => array(vec![reference(font1), int(12)]),
        });
        let function = p.add(dictionary! {
            "FunctionType" => 2, "Domain" => numbers(&[0.0, 1.0]), "N" => 1,
            "C0" => numbers(&[1.0, 0.0, 0.0]), "C1" => numbers(&[0.0, 0.0, 1.0]),
        });
        let stitching = p.add(dictionary! {
            "FunctionType" => 3, "Domain" => numbers(&[0.0, 1.0]),
            "Functions" => array(vec![reference(function), reference(function)]),
            "Bounds" => numbers(&[0.5]), "Encode" => numbers(&[0.0, 1.0, 0.0, 1.0]),
        });
        let axial = p.add(dictionary! {
            "ShadingType" => 2, "ColorSpace" => "DeviceRGB",
            "Coords" => numbers(&[0.0, 0.0, 100.0, 0.0]), "Function" => reference(function),
            "Extend" => array(vec![Object::Boolean(true), Object::Boolean(true)]),
        });
        let radial = p.add(dictionary! {
            "ShadingType" => 3, "ColorSpace" => "DeviceRGB",
            "Coords" => numbers(&[0.0, 0.0, 0.0, 10.0, 10.0, 50.0]),
            "Function" => reference(stitching),
        });
        let ocg = p.add(dictionary! {
            "Type" => "OCG", "Name" => string(b"Layer 1"), "Intent" => "View",
            "Usage" => dictionary! {
                "CreatorInfo" => dictionary! { "Creator" => string(b"x"), "Subtype" => "Artwork" },
            },
        });
        p.set(
            catalog,
            "OCProperties",
            dictionary! {
                "OCGs" => array(vec![reference(ocg)]),
                "D" => dictionary! { "ON" => array(vec![reference(ocg)]) },
            },
        );

        // Links on the first page.
        let uri = p.add(dictionary! {
            "Type" => "Annot", "Subtype" => "Link", "Rect" => numbers(&[10.0, 10.0, 50.0, 20.0]),
            "Border" => numbers(&[0.0, 0.0, 1.0]), "C" => numbers(&[1.0, 0.0, 0.0]), "H" => "I",
            "A" => dictionary! { "S" => "URI", "URI" => string(b"http://example.com") },
        });
        let goto = p.add(dictionary! {
            "Type" => "Annot", "Subtype" => "Link", "Rect" => numbers(&[10.0, 30.0, 50.0, 40.0]),
            "A" => dictionary! {
                "S" => "GoTo",
                "D" => array(vec![reference(page2), name("XYZ"), int(10), int(20), int(1)]),
            },
        });

        p.set(
            page1,
            "Resources",
            dictionary! {
                "Font" => dictionary! { "F1" => reference(font1), "F2" => reference(font2) },
                "XObject" => dictionary! { "Im0" => reference(image), "Fm0" => reference(form) },
                "ExtGState" => dictionary! { "GS0" => reference(gs) },
                "Shading" => dictionary! { "Sh0" => reference(axial), "Sh1" => reference(radial) },
                "Properties" => dictionary! { "OC0" => reference(ocg) },
            },
        );
        p.set(page1, "Contents", array(vec![reference(content1)]));
        p.set(page1, "Annots", array(vec![reference(uri), reference(goto)]));

        // An outline with a child.
        add_outline(&mut p, &[b"One", b"\xFE\xFF\x00T", b"Three"], None);
        let info = p.add(dictionary! {
            "Title" => string(b"T"), "Author" => string(b"A"), "Keywords" => string(b"a, b"),
            "CreationDate" => string(b"D:20240101120000+01'00'"),
            "ModDate" => string(b"D:20240102120000Z"),
            "Trapped" => "False", "GTS_PDFXVersion" => string(b"PDF/X-4"),
        });
        p.doc.trailer.set("Info", reference(info));
        p.doc
    }

    /// The deterministic generator of the mutations.
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

    fn hostile_value(rng: &mut Rng, ids: &[ObjectId]) -> (Object, String) {
        let value = match rng.below(14) {
            0 => Object::Null,
            1 => Object::Boolean(true),
            2 => {
                let numbers: [i64; 12] = [
                    0, -1, 1, 2, 255, 65_536, 1 << 31, 1 << 32, 1 << 62, i64::MAX, i64::MIN,
                    6_148_914_691_236_517_206,
                ];
                int(numbers[rng.below(numbers.len())])
            }
            3 => {
                let reals = [0.0f32, -1.5, 1e30, 1e-30, 0.5];
                Object::Real(reals[rng.below(reals.len())])
            }
            4 => {
                let names = [
                    "Foo", "", "Pages", "Page", "Identity-H", "DeviceRGB", "FlateDecode", "Image",
                    "Form", "GoTo", "URI",
                ];
                name(names[rng.below(names.len())])
            }
            5 => {
                let strings: [Vec<u8>; 7] = [
                    b"".to_vec(),
                    vec![0xFE],
                    vec![0xFE, 0xFF],
                    vec![0xFE, 0xFF, 0x00],
                    b"D:2024".to_vec(),
                    "\u{e9}".repeat(20).into_bytes(),
                    vec![b'x'; 5_000],
                ];
                Object::String(strings[rng.below(strings.len())].clone(), StringFormat::Literal)
            }
            6 => array(vec![]),
            7 => array(vec![int(1), int(2), int(3)]),
            8 => array(vec![int(0), int(0), int(10), int(10)]),
            9 => array(vec![reference(ids[rng.below(ids.len())])]),
            10 => array((1..=6).map(int).collect()),
            11 | 12 => reference(ids[rng.below(ids.len())]),
            _ => reference((9_999, 0)),
        };
        let mut description = format!("{value:?}");
        description.truncate(60);
        (value, description)
    }

    /// Replaces, removes or redirects some entries of a valid document, 300 times. The result of
    /// each parse is an `Ok` or an `Err`, and what matters is that there is one.
    #[test]
    fn mutated_documents() {
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
        assert!(entries.len() > 100, "{} entries", entries.len());

        // The unchanged document parses with its bookmarks, links and resources.
        let parsed = parse_bounded("unchanged document", document_bytes(&mut base.clone()));
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert_eq!(parsed.pages, 2);
        assert_eq!(parsed.bookmarks.len(), 3);
        assert_eq!(parsed.links, 2);
        assert_eq!(parsed.shadings, 2);

        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut failures = Vec::new();
        let (mut ok, mut err) = (0, 0);
        for iteration in 0..300 {
            let mut doc = base.clone();
            let mut changes = Vec::new();
            for _ in 0..=rng.below(3) {
                let (id, key) = &entries[rng.below(entries.len())];
                let dict = dict_mut(&mut doc, *id);
                let key_text = String::from_utf8_lossy(key).to_string();
                match rng.below(10) {
                    0 => {
                        dict.remove(key);
                        changes.push(format!("{id:?} remove /{key_text}"));
                    }
                    _ => {
                        let (value, text) = hostile_value(&mut rng, &ids);
                        dict.set(key.clone(), value);
                        changes.push(format!("{id:?} /{key_text} = {text}"));
                    }
                }
            }
            let what = format!("mutation {iteration}: {}", changes.join("; "));
            let bytes = document_bytes(&mut doc);
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                parse_bounded(&what, bytes)
            }));
            match outcome {
                Ok(parsed) if parsed.result.is_ok() => ok += 1,
                Ok(_) => err += 1,
                Err(_) => failures.push(what),
            }
        }
        eprintln!("300 mutations: {ok} parsed, {err} refused");
        assert!(failures.is_empty(), "parsing panicked:\n{}", failures.join("\n"));
    }
}

// ---------------------------------------------------------------------------------------------

/// Streams and shared resources whose size is the point: a limit, a budget, or a copy for each use.
mod resources {
    use super::*;

    /// One big stream that a form's resources refer to over and over is copied each time.
    #[test]
    fn inlined_stream_data_has_a_budget() {
        let _guard = heavy();
        let mut p = pdf();
        let big = p.add(Stream::new(dictionary! {}, vec![7u8; 1 << 20]));
        let form = p.add(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Form",
                "BBox" => numbers(&[0.0, 0.0, 10.0, 10.0]),
                "Extra" => array(vec![reference(big); 400]),
            },
            b"q Q".to_vec(),
        ));
        p.page_resources(dictionary! {
            "XObject" => dictionary! { "Fm0" => reference(form) },
        });
        let parsed = parse_bounded("one stream referenced 400 times", p.bytes());
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert!(has_warning(&parsed, "exceed the limit"), "{:?}", messages(&parsed));
        match &parsed.xobjects[0].1 {
            Kind::External { streams, .. } => assert!(*streams < 400, "{streams} copies"),
            other => panic!("unexpected {other:?}"),
        }
    }

    /// A resource that pages list under the same name is read once, however many pages list it.
    #[test]
    fn resources_shared_by_many_pages_are_read_once() {
        let mut p = pdf();
        let image = p.add(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 1, "Height" => 1,
                "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
            },
            vec![1, 2, 3],
        ));
        let shared = p.add(dictionary! {
            "XObject" => dictionary! { "Im0" => reference(image) },
        });
        let (root, first) = (p.pages, p.page);
        let mut kids = vec![reference(first)];
        for _ in 0..500 {
            kids.push(reference(p.add(dictionary! {
                "Type" => "Page", "Parent" => reference(root), "Resources" => reference(shared),
                "MediaBox" => numbers(&[0.0, 0.0, 10.0, 10.0]),
            })));
        }
        p.set(root, "Kids", array(kids));
        p.set(first, "Resources", reference(shared));
        let parsed = parse_bounded("501 pages with one resource dictionary", p.bytes());
        assert_eq!(parsed.pages, 501);
        assert_eq!(parsed.xobjects.len(), 1);
    }

    /// A content stream that decodes to more than a stream may is skipped, with a warning.
    #[test]
    fn content_stream_bomb() {
        let _guard = heavy();
        let mut p = pdf();
        let content = p.add(Stream::new(
            dictionary! { "Filter" => "FlateDecode" },
            bomb().to_vec(),
        ));
        let page = p.page;
        p.set(page, "Contents", reference(content));
        let parsed = parse_bounded("content stream of 300 MiB", p.bytes());
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert_eq!(parsed.pages, 1);
        assert!(has_warning(&parsed, "larger than"), "{:?}", messages(&parsed));
    }

    /// The streams of one page add up to one limit.
    #[test]
    fn content_streams_of_a_page_share_a_limit() {
        let _guard = heavy();
        let mut p = pdf();
        let streams: Vec<Object> = (0..2)
            .map(|_| {
                // Each of them decodes to 150 MiB: below the limit alone, above it together.
                use flate2::{write::ZlibEncoder, Compression};
                let chunk = vec![b' '; 1 << 20];
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
                for _ in 0..150 {
                    encoder.write_all(&chunk).expect("compress");
                }
                let data = encoder.finish().expect("compress");
                reference(p.add(Stream::new(dictionary! { "Filter" => "FlateDecode" }, data)))
            })
            .collect();
        let page = p.page;
        p.set(page, "Contents", array(streams));
        let parsed = parse_bounded("two content streams of 150 MiB", p.bytes());
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert!(has_warning(&parsed, "larger than"), "{:?}", messages(&parsed));
    }

    /// A ToUnicode CMap of 300 MiB.
    #[test]
    fn to_unicode_bomb() {
        let _guard = heavy();
        let mut p = pdf();
        let to_unicode = p.add(Stream::new(
            dictionary! { "Filter" => "FlateDecode" },
            bomb().to_vec(),
        ));
        let font = p.add(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
            "ToUnicode" => reference(to_unicode),
        });
        p.page_resources(dictionary! {
            "Font" => dictionary! { "F1" => reference(font) },
        });
        let parsed = parse_bounded("ToUnicode of 300 MiB", p.bytes());
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert!(has_warning(&parsed, "larger than"), "{:?}", messages(&parsed));
    }

    /// An image whose data decodes to 300 MiB. It is kept as it is.
    #[test]
    fn image_bomb() {
        let _guard = heavy();
        let mut p = pdf();
        let image = p.add(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 1, "Height" => 1,
                "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
                "Filter" => "FlateDecode",
            },
            bomb().to_vec(),
        ));
        p.page_resources(dictionary! {
            "XObject" => dictionary! { "Im0" => reference(image) },
        });
        let parsed = parse_bounded("image of 300 MiB", p.bytes());
        assert!(parsed.result.is_ok(), "{:?}", parsed.result);
        assert!(has_warning(&parsed, "larger than"), "{:?}", messages(&parsed));
        assert!(
            matches!(parsed.xobjects.as_slice(), [(_, Kind::External { .. })]),
            "{:?}",
            parsed.xobjects
        );
    }
}
