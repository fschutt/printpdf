//! Runs the built `ppdf` binary end to end: HTML to PDF, PDF to JSON and back,
//! text, boxes, SVG, fonts and images, and the failure paths.

use printpdf::{OutputImageFormat, RawImage, RawImageData, RawImageFormat};
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

const THREE_PAGES: &str = r#"<html><body style="font-size: 20px">
<h1 style="break-after: page">Page one</h1>
<h1 style="break-after: page">Page two</h1>
<h1>Page three</h1>
</body></html>"#;

struct Out {
    code: i32,
    stdout: Vec<u8>,
    stderr: String,
}

impl Out {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", self.text()))
    }
}

fn run(args: &[&str], stdin: &[u8]) -> Out {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ppdf"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start ppdf");

    // Feed stdin from a thread: ppdf reads it all before it writes, but do not
    // depend on that.
    let mut pipe = child.stdin.take().expect("stdin");
    let data = stdin.to_vec();
    let feeder = std::thread::spawn(move || {
        let _ = pipe.write_all(&data);
    });
    let out = child.wait_with_output().expect("wait for ppdf");
    feeder.join().unwrap();

    Out {
        code: out.status.code().unwrap_or(-1),
        stdout: out.stdout,
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Runs ppdf and requires exit code 0.
fn ok(args: &[&str]) -> Out {
    ok_stdin(args, &[])
}

fn ok_stdin(args: &[&str], stdin: &[u8]) -> Out {
    let out = run(args, stdin);
    assert_eq!(out.code, 0, "ppdf {args:?} failed: {}", out.stderr);
    out
}

/// A fresh directory for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("ppdf-it-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn path(dir: &Path, name: &str) -> String {
    dir.join(name).to_str().unwrap().to_string()
}

/// A three page PDF made by `ppdf html`, shared by the tests that only read it.
fn three_pages() -> String {
    static PDF: OnceLock<String> = OnceLock::new();
    PDF.get_or_init(|| {
        let dir = scratch("fixture");
        let html = path(&dir, "three.html");
        let pdf = path(&dir, "three.pdf");
        std::fs::write(&html, THREE_PAGES).unwrap();
        ok(&["html", &html, "-o", &pdf]);
        pdf
    })
    .clone()
}

#[test]
fn html_to_pdf_then_text() {
    let pdf = three_pages();
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));

    // pages are separated by a form feed
    let text = ok(&["text", &pdf]).text();
    let pages: Vec<&str> = text.split('\x0c').collect();
    assert_eq!(pages.len(), 3, "{text:?}");
    assert!(pages[0].contains("Page one"));
    assert!(pages[1].contains("Page two"));
    assert!(pages[2].contains("Page three"));
}

#[test]
fn page_selection_keeps_the_order_written() {
    let pdf = three_pages();
    assert_eq!(
        ok(&["text", &pdf, "--pages", "2"]).text().trim(),
        "Page two"
    );

    let text = ok(&["text", &pdf, "--pages", "3,1"]).text();
    let pages: Vec<&str> = text.split('\x0c').collect();
    assert!(
        pages[0].contains("Page three") && pages[1].contains("Page one"),
        "{text:?}"
    );

    let doc = ok(&["json", &pdf, "--pages", "3,1"]).json();
    assert_eq!(doc["pages"].as_array().unwrap().len(), 2);
}

#[test]
fn json_has_a_schema_first_and_round_trips_through_stdin() {
    let pdf = three_pages();
    let json = ok(&["json", &pdf]);
    let doc = json.json();
    assert_eq!(doc.as_object().unwrap().keys().next().unwrap(), "$schema");
    assert_eq!(doc["$schema"], "printpdf-json/1");
    assert_eq!(doc["pages"].as_array().unwrap().len(), 3);

    // JSON on stdin, PDF on stdout (the file is never named), then read it back.
    let dir = scratch("roundtrip");
    let again = path(&dir, "again.pdf");
    ok_stdin(&["pdf", "-", "-o", &again], &json.stdout);
    assert_eq!(ok(&["text", &again]).text(), ok(&["text", &pdf]).text());
}

#[test]
fn editing_the_json_edits_the_pdf() {
    let pdf = three_pages();
    let mut doc = ok(&["json", &pdf]).json();
    doc["pages"].as_array_mut().unwrap().remove(1); // drop page two

    let dir = scratch("edit");
    let edited = path(&dir, "edited.pdf");
    ok_stdin(&["pdf", "-", "-o", &edited], doc.to_string().as_bytes());

    let text = ok(&["text", &edited]).text();
    assert!(text.contains("Page one") && text.contains("Page three"));
    assert!(!text.contains("Page two"), "{text:?}");
}

#[test]
fn stripped_binaries_come_back_losslessly() {
    let pdf = three_pages();
    let dir = scratch("strip");
    let parts = path(&dir, "parts");

    let full = ok(&["json", &pdf]);
    let stripped = ok(&["json", &pdf, "--strip-binaries", &parts]);
    assert!(
        stripped.stdout.len() * 4 < full.stdout.len(),
        "the fonts should leave the JSON"
    );
    let files = std::fs::read_dir(&parts).unwrap().count();
    assert!(files >= 1);
    assert!(stripped.text().contains("\"$file\""));

    let a = path(&dir, "a.pdf");
    let b = path(&dir, "b.pdf");
    ok_stdin(&["pdf", "-", "-o", &a], &full.stdout);
    ok_stdin(
        &["pdf", "-", "--binaries", &parts, "-o", &b],
        &stripped.stdout,
    );
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());

    // without --binaries the references cannot be read
    let out = run(&["pdf", "-", "-o", &path(&dir, "c.pdf")], &stripped.stdout);
    assert_eq!(out.code, 1, "{}", out.stderr);
}

#[test]
fn boxes_have_words_and_glyphs_only_on_request() {
    let pdf = three_pages();
    let plain = ok(&["boxes", &pdf, "--pages", "1"]);
    let pages = plain.json();
    let first = &pages[0];
    assert_eq!(first["page"], 1);
    assert!(first["width"].as_f64().unwrap() > 500.0);
    let words: Vec<&str> = first["lines"][0]["words"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["text"].as_str().unwrap())
        .collect();
    assert_eq!(words, ["Page", "one"]);
    assert!(!plain.text().contains("\"glyphs\""));

    assert!(ok(&["boxes", &pdf, "--pages", "1", "--glyphs"])
        .text()
        .contains("\"glyphs\""));
}

#[test]
fn svg_per_page_or_one_on_stdout() {
    let pdf = three_pages();

    let dir = scratch("svg");
    ok(&["svg", &pdf, "-o", &path(&dir, "out")]);
    for n in 1..=3 {
        let svg = std::fs::read_to_string(dir.join("out").join(format!("page-{n}.svg"))).unwrap();
        assert!(svg.contains("<svg"), "page {n}");
    }

    assert!(ok(&["svg", &pdf, "--pages", "2"]).text().contains("<svg"));
    // several pages cannot share stdout
    let out = run(&["svg", &pdf], &[]);
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("-o DIR"), "{}", out.stderr);
}

#[test]
fn info_lists_pages_and_fonts() {
    let info = ok(&["info", &three_pages()]).json();
    assert_eq!(info["page_count"], 3);
    assert_eq!(info["pages"].as_array().unwrap().len(), 3);
    assert!(!info["fonts"].as_array().unwrap().is_empty());
    assert_eq!(info["warnings"]["error"], 0);
}

#[test]
fn fonts_are_written_as_font_programs() {
    let dir = scratch("fonts");
    let out = path(&dir, "out");
    let list = ok(&["fonts", &three_pages(), "-o", &out]).json();
    let fonts = list.as_array().unwrap();
    assert!(!fonts.is_empty());
    for font in fonts {
        let file = dir.join("out").join(font["file"].as_str().unwrap());
        let bytes = std::fs::read(&file).unwrap();
        assert_eq!(bytes.len() as u64, font["bytes"].as_u64().unwrap());
        assert!(
            bytes.starts_with(&[0, 1, 0, 0])
                || bytes.starts_with(b"OTTO")
                || bytes.starts_with(b"true"),
            "{file:?} is not a font program"
        );
    }
}

#[test]
fn images_from_html_are_extracted_as_png() {
    let dir = scratch("images");

    // a 2x2 picture, encoded as PNG with printpdf itself
    let picture = RawImage {
        pixels: RawImageData::U8(vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]),
        width: 2,
        height: 2,
        data_format: RawImageFormat::RGB8,
        tag: Vec::new(),
    };
    let (png, _) = picture.encode_to_bytes(&[OutputImageFormat::Png]).unwrap();
    std::fs::create_dir_all(dir.join("pics")).unwrap();
    std::fs::write(dir.join("pics/dot.png"), png).unwrap();

    // layout does not read the picture's size: give both width and height
    let html = path(&dir, "page.html");
    std::fs::write(
        &html,
        r#"<html><body><img src="dot.png" style="width:40px;height:40px"></body></html>"#,
    )
    .unwrap();
    let pdf = path(&dir, "page.pdf");
    ok(&["html", &html, "--images", &path(&dir, "pics"), "-o", &pdf]);

    let list = ok(&["images", &pdf, "-o", &path(&dir, "out")]).json();
    let images = list.as_array().unwrap();
    assert_eq!(images.len(), 1, "{list}");
    assert_eq!(
        (images[0]["width"].as_u64(), images[0]["height"].as_u64()),
        (Some(2), Some(2))
    );
    let written = std::fs::read(dir.join("out").join(images[0]["file"].as_str().unwrap())).unwrap();
    assert!(written.starts_with(&[0x89, b'P', b'N', b'G']));
}

#[test]
fn a_json_round_trip_does_not_change_images() {
    let dir = scratch("image_roundtrip");

    // A smooth 128x128 picture. The library default saves it as JPEG, which is lossy, so
    // writing it again with image optimization on would change the pixels it reads back as.
    let mut pixels = Vec::new();
    for y in 0..128u32 {
        for x in 0..128u32 {
            pixels.extend([
                (x * 2 % 256) as u8,
                ((y * 2 + x) % 256) as u8,
                (255 - x - y) as u8,
            ]);
        }
    }
    let picture = RawImage {
        pixels: RawImageData::U8(pixels),
        width: 128,
        height: 128,
        data_format: RawImageFormat::RGB8,
        tag: Vec::new(),
    };
    let (png, _) = picture.encode_to_bytes(&[OutputImageFormat::Png]).unwrap();
    std::fs::create_dir_all(dir.join("pics")).unwrap();
    std::fs::write(dir.join("pics/g.png"), png).unwrap();

    let html = path(&dir, "g.html");
    std::fs::write(
        &html,
        r#"<html><body><img src="g.png" style="width:200px;height:200px"></body></html>"#,
    )
    .unwrap();
    let first = path(&dir, "first.pdf");
    ok(&["html", &html, "--images", &path(&dir, "pics"), "-o", &first]);

    // what the image looks like after each way of writing it again
    let pixels_of = |pdf: &str, name: &str| {
        let out = path(&dir, name);
        let list = ok(&["images", pdf, "-o", &out]).json();
        let file = list[0]["file"].as_str().unwrap().to_string();
        std::fs::read(dir.join(name).join(file)).unwrap()
    };
    let before = pixels_of(&first, "before");

    let json = ok(&["json", &first]);
    let again = path(&dir, "again.pdf");
    ok_stdin(&["pdf", "-", "-o", &again], &json.stdout);
    assert_eq!(
        pixels_of(&again, "after"),
        before,
        "a round trip must not change the pixels"
    );

    let lossy = path(&dir, "lossy.pdf");
    ok_stdin(
        &["pdf", "-", "--optimize-images", "-o", &lossy],
        &json.stdout,
    );
    assert_ne!(
        pixels_of(&lossy, "lossy"),
        before,
        "--optimize-images re-encodes"
    );
}

#[test]
fn css_files_are_added_to_the_html() {
    let dir = scratch("css");
    let html = path(&dir, "p.html");
    std::fs::write(
        &html,
        "<html><head></head><body><p>colour me</p></body></html>",
    )
    .unwrap();
    std::fs::write(dir.join("p.css"), "p { color: #ff0000; }").unwrap();
    let pdf = path(&dir, "p.pdf");
    ok(&["html", &html, "--css", &path(&dir, "p.css"), "-o", &pdf]);

    let boxes = ok(&["boxes", &pdf]).json();
    let color = &boxes[0]["lines"][0]["words"][0]["color"]["data"];
    assert!(
        color["r"].as_f64().unwrap() > 0.9 && color["g"].as_f64().unwrap() < 0.1,
        "{color}"
    );
}

#[test]
fn html_page_size_is_in_millimetres() {
    let dir = scratch("size");
    let pdf = path(&dir, "a5.pdf");
    ok_stdin(
        &["html", "-", "--width", "148", "--height", "210", "-o", &pdf],
        b"<p>x</p>",
    );
    let info = ok(&["info", &pdf]).json();
    let width = info["pages"][0]["width_pt"].as_f64().unwrap();
    assert!((width - 148.0 / 25.4 * 72.0).abs() < 0.5, "{width}");
}

#[test]
fn failures_exit_with_1_and_a_message() {
    let pdf = three_pages();
    let cases: [(&[&str], &[u8], &str); 6] = [
        (&["text"], b"not a pdf", "Failed to load PDF"),
        (&["text", "does-not-exist.pdf"], b"", "does-not-exist.pdf"),
        (
            &["text", &pdf, "--pages", "9"],
            b"",
            "page 9 does not exist",
        ),
        (&["text", &pdf, "--pages", "0"], b"", "start at 1"),
        (
            &["pdf", "-"],
            br#"{"$schema":"printpdf-json/2"}"#,
            "unsupported $schema",
        ),
        (&["pdf", "-"], b"{ not json", "not valid JSON"),
    ];
    for (args, stdin, message) in cases {
        let out = run(args, stdin);
        assert_eq!(out.code, 1, "ppdf {args:?}: {}", out.stderr);
        assert!(out.stderr.starts_with("ppdf: error:"), "{}", out.stderr);
        assert!(
            out.stderr.contains(message),
            "ppdf {args:?}: {}",
            out.stderr
        );
        assert!(out.stdout.is_empty(), "nothing is written on failure");
    }
}

/// A one page PDF written by hand. It has no cross reference table, so the reader
/// rebuilds it.
fn pdf_with_content(content: &str) -> Vec<u8> {
    format!(
        "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
         2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
         3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources << >> >>\nendobj\n\
         4 0 obj\n<< /Length {} >>\nstream\n{content}\nendstream\nendobj\n\
         trailer\n<< /Root 1 0 R /Size 5 >>\n%%EOF\n",
        content.len()
    )
    .into_bytes()
}

#[test]
fn a_skipped_operation_exits_with_2() {
    let dir = scratch("exit2");
    let pdf = path(&dir, "bad.pdf");
    std::fs::write(&pdf, pdf_with_content("1 0 0 rg 10 10 l BOGUSOP 5 5 m S")).unwrap();

    let out = run(&["text", &pdf], &[]);
    assert_eq!(out.code, 2, "{}", out.stderr);
    assert!(out.stderr.contains("BOGUSOP"), "{}", out.stderr);

    let json = run(&["text", &pdf, "--warnings", "json"], &[]);
    let first = json.stderr.lines().next().expect("a warning line");
    let warning: Value = serde_json::from_str(first).unwrap();
    assert_eq!(warning["severity"], "error");

    // silenced warnings do not change the exit code
    let quiet = run(&["text", &pdf, "--warnings", "off"], &[]);
    assert_eq!(quiet.code, 2);
    assert!(quiet.stderr.is_empty(), "{}", quiet.stderr);

    // a clean file exits with 0
    let clean = path(&dir, "clean.pdf");
    std::fs::write(&clean, pdf_with_content("0 0 m 10 10 l S")).unwrap();
    assert_eq!(run(&["text", &clean], &[]).code, 0);
}

#[test]
fn a_pdf_without_its_trailer_is_still_read() {
    let pdf = three_pages();
    let bytes = std::fs::read(&pdf).unwrap();
    let dir = scratch("damaged");

    // the cross reference table and trailer are gone
    let cut = path(&dir, "cut.pdf");
    std::fs::write(&cut, &bytes[..bytes.len() - 400]).unwrap();
    assert!(ok(&["text", &cut]).text().contains("Page one"));

    // junk in front of the header
    let junk = path(&dir, "junk.pdf");
    let mut with_junk = b"JUNK\n".to_vec();
    with_junk.extend_from_slice(&bytes);
    std::fs::write(&junk, with_junk).unwrap();
    assert!(ok(&["text", &junk]).text().contains("Page one"));
}

#[test]
fn check_exits_with_0_1_or_2() {
    let pdf = three_pages();
    let dir = scratch("check");

    // No errors in the General profile.
    let out = run(&["check", &pdf], &[]);
    assert_eq!(out.code, 0, "{}", out.text());
    assert!(out.text().contains("not a certification"), "{}", out.text());

    // Errors for a standard that the file does not meet.
    let out = run(&["check", &pdf, "--profile", "pdfx-3"], &[]);
    assert_eq!(out.code, 1, "{}", out.text());
    assert!(out.text().contains("output-intent-missing"), "{}", out.text());

    // Not a PDF, and a file that does not exist.
    let junk = path(&dir, "junk.pdf");
    std::fs::write(&junk, b"this is not a PDF").unwrap();
    let out = run(&["check", &junk], &[]);
    assert_eq!(out.code, 2, "{}", out.text());
    assert!(out.text().contains("file-unreadable"), "{}", out.text());
    let out = run(&["check", &path(&dir, "missing.pdf")], &[]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("missing.pdf"), "{}", out.stderr);
}

#[test]
fn check_prints_json_and_reads_stdin() {
    let pdf = three_pages();
    let bytes = std::fs::read(&pdf).unwrap();

    let out = run(&["check", "-", "--profile", "pdfa-1b", "--json"], &bytes);
    assert_eq!(out.code, 1);
    let report = out.json();
    assert_eq!(report["profile"], "pdfa-1b");
    let rules: Vec<&str> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["rule"].as_str().unwrap())
        .collect();
    assert!(rules.contains(&"xmp-missing"), "{rules:?}");
    assert!(report["summary"]["errors"].as_u64().unwrap() >= 1);
    assert!(report["notChecked"].as_array().unwrap().len() > 3);

    // An unknown profile is a usage error.
    let out = run(&["check", &pdf, "--profile", "pdfx-9"], &[]);
    assert_eq!(out.code, 2);
}
