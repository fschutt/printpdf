//! Round-trip audit harness: parse -> save -> parse for every corpus PDF.
//!
//! Corpus = the `*_example.pdf` files that `bash gen.sh` writes into the repo
//! root (skipped silently when absent, so CI stays green without the corpus)
//! plus documents that other tests in this file build through the public API.
//!
//! For every corpus file this checks:
//!   1. `PdfDocument::parse` succeeds,
//!   2. re-saving produces bytes that `PdfDocument::parse` accepts again
//!      (second round trip must not error),
//!   3. the re-saved file still contains the extractable text of the original
//!      (compared via `PdfDocument::extract_text`).
//!
//! The re-saved files are written to `target/roundtrip/` so external tools
//! (pdftotext, pdftoppm) can compare them against the originals.

use printpdf::{PdfDocument, PdfParseOptions, PdfSaveOptions, PdfWarnMsg};

fn roundtrip_dir() -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("roundtrip");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn parse(bytes: &[u8], ctx: &str) -> (PdfDocument, Vec<PdfWarnMsg>) {
    let mut warnings = Vec::new();
    let doc = PdfDocument::parse(bytes, &PdfParseOptions::default(), &mut warnings)
        .unwrap_or_else(|e| panic!("{ctx}: parse failed: {e}"));
    (doc, warnings)
}

fn errors(warnings: &[PdfWarnMsg]) -> Vec<String> {
    warnings
        .iter()
        .filter(|w| w.severity == printpdf::PdfParseErrorSeverity::Error)
        .map(|w| format!("p{} op{}: {}", w.page, w.op_id, w.msg))
        .collect()
}

/// Flatten extract_text output to one normalized string (whitespace-insensitive).
fn flat_text(doc: &PdfDocument) -> String {
    doc.extract_text()
        .iter()
        .flatten()
        .flat_map(|chunk| chunk.split_whitespace())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Full audit of one PDF byte buffer. Returns (resaved bytes, report lines).
fn audit_roundtrip(name: &str, bytes: &[u8]) -> (Vec<u8>, Vec<String>) {
    let mut report = Vec::new();

    let (doc, warnings) = parse(bytes, name);
    let text_orig = flat_text(&doc);
    let errs = errors(&warnings);
    report.push(format!(
        "{name}: parse ok, {} pages, {} fonts, {} xobjects, {} layers, {} bookmarks, {} parse \
         errors",
        doc.pages.len(),
        doc.resources.fonts.map.len(),
        doc.resources.xobjects.map.len(),
        doc.resources.layers.map.len(),
        doc.bookmarks.map.len(),
        errs.len()
    ));
    for e in errs.iter().take(8) {
        report.push(format!("  parse-error: {e}"));
    }

    let mut save_warnings = Vec::new();
    let resaved = doc.save(&PdfSaveOptions::default(), &mut save_warnings);

    let (doc2, warnings2) = parse(&resaved, &format!("{name} (resaved)"));
    let errs2 = errors(&warnings2);
    report.push(format!(
        "{name}: re-parse ok, {} pages, {} fonts, {} xobjects, {} layers, {} bookmarks, {} parse \
         errors",
        doc2.pages.len(),
        doc2.resources.fonts.map.len(),
        doc2.resources.xobjects.map.len(),
        doc2.resources.layers.map.len(),
        doc2.bookmarks.map.len(),
        errs2.len()
    ));
    for e in errs2.iter().take(8) {
        report.push(format!("  reparse-error: {e}"));
    }

    let text_re = flat_text(&doc2);
    if text_orig != text_re {
        report.push(format!(
            "  TEXT DIVERGED:\n    orig: {:?}\n    re:   {:?}",
            &text_orig.chars().take(160).collect::<String>(),
            &text_re.chars().take(160).collect::<String>()
        ));
    } else {
        report.push(format!(
            "  text stable ({} chars): {:?}",
            text_orig.len(),
            text_orig.chars().take(60).collect::<String>()
        ));
    }

    assert_eq!(
        doc.pages.len(),
        doc2.pages.len(),
        "{name}: page count changed across round trip"
    );
    assert_eq!(
        text_orig, text_re,
        "{name}: extracted text changed across round trip"
    );
    assert!(
        doc2.resources.xobjects.map.len() >= doc.resources.xobjects.map.len(),
        "{name}: xobjects lost across round trip ({} -> {})",
        doc.resources.xobjects.map.len(),
        doc2.resources.xobjects.map.len()
    );
    assert!(
        doc2.bookmarks.map.len() >= doc.bookmarks.map.len(),
        "{name}: bookmarks lost across round trip ({} -> {})",
        doc.bookmarks.map.len(),
        doc2.bookmarks.map.len()
    );
    assert!(
        doc2.resources.layers.map.len() >= doc.resources.layers.map.len(),
        "{name}: layers lost across round trip ({} -> {})",
        doc.resources.layers.map.len(),
        doc2.resources.layers.map.len()
    );

    (resaved, report)
}

/// Audit every generated example PDF found in the repository root.
/// Prints a report; run with `--nocapture` to see it.
#[test]
fn corpus_roundtrip_files() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = roundtrip_dir();
    let mut all = Vec::new();

    let mut names: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map(|e| e == "pdf").unwrap_or(false)
                && p.file_name()
                    .and_then(|f| f.to_str())
                    .map(|f| f.ends_with("_example.pdf") || f == "htmltest.pdf")
                    .unwrap_or(false)
        })
        .collect();
    names.sort();

    if names.is_empty() {
        eprintln!("corpus_roundtrip_files: no corpus PDFs in repo root, skipping");
        return;
    }

    for path in names {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let bytes = std::fs::read(&path).unwrap();
        let (resaved, report) = audit_roundtrip(&name, &bytes);
        std::fs::write(out.join(format!("{name}.roundtrip.pdf")), &resaved).unwrap();

        // Second round trip: parse the re-saved bytes again and save once more.
        let (doc2, _) = parse(&resaved, &format!("{name} roundtrip2"));
        let resaved2 = doc2.save(&PdfSaveOptions::default(), &mut Vec::new());
        let (_, _) = parse(&resaved2, &format!("{name} roundtrip3"));

        all.extend(report);
    }

    eprintln!("== corpus round-trip report ==");
    for line in &all {
        eprintln!("{line}");
    }
}
